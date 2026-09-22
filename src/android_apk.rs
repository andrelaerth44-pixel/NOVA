use crate::app_ast::AppDeclRoot;
use crate::app_backend_android::emit_android_project_files;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(program: &Path, args: &[String], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd { cmd.current_dir(dir); }
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot start {}: {}", program.display(), e))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} failed ({}):\n{}{}",
            program.display(),
            output.status,
            stdout,
            stderr
        ));
    }
    Ok(())
}

fn env_path(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|name| std::env::var_os(name).map(PathBuf::from))
}

fn first_existing(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.exists())
}

fn sdk_root() -> Result<PathBuf, String> {
    env_path(&["NOVA_ANDROID_SDK", "ANDROID_HOME", "ANDROID_SDK_ROOT"])
        .filter(|p| p.exists())
        .ok_or_else(|| "Android SDK not found; set NOVA_ANDROID_SDK or ANDROID_HOME".into())
}

fn ndk_root() -> Result<PathBuf, String> {
    if let Some(p) = env_path(&["NOVA_ANDROID_NDK", "ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"]) {
        if p.exists() { return Ok(p); }
    }
    Err("Android NDK not found; set NOVA_ANDROID_NDK".into())
}

fn build_tools_dir(sdk: &Path) -> Result<PathBuf, String> {
    if let Some(explicit) = env_path(&["NOVA_ANDROID_BUILD_TOOLS"]) {
        if explicit.exists() { return Ok(explicit); }
    }
    let root = sdk.join("build-tools");
    let mut versions = fs::read_dir(&root)
        .map_err(|e| format!("cannot read {}: {}", root.display(), e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();
    versions.sort();
    versions.pop().ok_or_else(|| format!("no Android build-tools found under {}", root.display()))
}

fn locate_tool(dir: &Path, name: &str) -> Result<PathBuf, String> {
    first_existing([
        dir.join(name),
        dir.join(format!("{}.exe", name)),
    ]).ok_or_else(|| format!("Android tool {} not found under {}", name, dir.display()))
}

fn ndk_clang(ndk: &Path) -> Result<PathBuf, String> {
    first_existing([
        ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin/clang"),
        ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin/clang.exe"),
    ]).ok_or_else(|| format!("NDK clang not found under {}", ndk.display()))
}

fn ndk_readelf(ndk: &Path) -> Option<PathBuf> {
    first_existing([
        ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"),
        ndk.join("toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf.exe"),
    ])
}

fn shell_tool(name: &str) -> Result<PathBuf, String> {
    let output = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(name)
        .output()
        .map_err(|e| format!("cannot locate {}: {}", name, e))?;
    if !output.status.success() {
        return Err(format!("required tool {} is not available on PATH", name));
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or(name).trim()))
}

fn write_files(root: &Path, files: Vec<(String, String)>) -> Result<(), String> {
    for (relative, contents) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
        }
        fs::write(&path, contents)
            .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    }
    Ok(())
}

fn native_arm64_source(app: &AppDeclRoot) -> String {
    let name = app.name.replace('"', "'");
    format!(r#".text
.p2align 2

.global ANativeActivity_onCreate
.type ANativeActivity_onCreate, %function
ANativeActivity_onCreate:
    mov w0, #4
    adrp x1, nova_log_tag
    add x1, x1, :lo12:nova_log_tag
    adrp x2, nova_log_message
    add x2, x2, :lo12:nova_log_message
    bl __android_log_write
    ret
.size ANativeActivity_onCreate, .-ANativeActivity_onCreate

.global nova_main
.type nova_main, %function
nova_main:
    ret
.size nova_main, .-nova_main

.section .rodata
.p2align 3
nova_app_name:
    .asciz "{}"
nova_log_tag:
    .asciz "NOVA"
nova_log_message:
    .asciz "NOVA native Android runtime loaded"
"#, name)
}

/// Build a signed native Android APK directly from NOVA application source.
///
/// The application source remains Main.nova. No Kotlin, Java, C, C++ or
/// Gradle project is generated. The only platform-facing artifact is an
/// ARM64 ELF shared library exporting Android's NativeActivity entrypoint.
pub fn build_apk(app: &AppDeclRoot, output: &Path) -> Result<PathBuf, String> {
    let sdk = sdk_root()?;
    let ndk = ndk_root()?;
    let bt = build_tools_dir(&sdk)?;
    let aapt2 = locate_tool(&bt, "aapt2")?;
    let apksigner = locate_tool(&bt, "apksigner")?;
    let zipalign = locate_tool(&bt, "zipalign")?;
    let clang = ndk_clang(&ndk)?;
    let readelf = ndk_readelf(&ndk);
    let keytool = shell_tool("keytool")?;

    let work = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".nova-apk-{}", std::process::id()));
    if work.exists() {
        fs::remove_dir_all(&work).map_err(|e| format!("cannot clean {}: {}", work.display(), e))?;
    }
    fs::create_dir_all(work.join("assets"))
        .map_err(|e| format!("cannot create APK staging directory: {}", e))?;

    let files = emit_android_project_files(app)?;
    write_files(&work, files)?;
    fs::write(work.join("assets/Main.nova"), fs::read_to_string(work.join("Main.nova")).unwrap_or_default())
        .map_err(|e| format!("cannot stage Main.nova: {}", e))?;
    fs::write(work.join("assets/nova.toml"), fs::read_to_string(work.join("nova.toml")).unwrap_or_default())
        .map_err(|e| format!("cannot stage nova.toml: {}", e))?;

    let asm = native_arm64_source(app);
    fs::write(work.join("nova_main.S"), asm)
        .map_err(|e| format!("cannot write ARM64 source: {}", e))?;

    let lib = work.join("libnova_main.so");
    run(&clang, &[
        "--target=aarch64-linux-android23".into(),
        "-shared".into(),
        "-fPIC".into(),
        "-nostdlib".into(),
        "-Wl,-soname,libnova_main.so".into(),
        "-llog".into(),
        "-Wl,-z,max-page-size=16384".into(),
        "-o".into(), lib.display().to_string(),
        work.join("nova_main.S").display().to_string(),
    ], Some(&work))?;

    if let Some(readelf) = readelf {
        run(&readelf, &[
            "-Ws".into(),
            lib.display().to_string(),
        ], None)?;
    }

    let platform = sdk.join("platforms/android-35/android.jar");
    if !platform.exists() {
        return Err(format!("Android platform android-35 not installed: {}", platform.display()));
    }

    let unsigned = work.join("unsigned.apk");
    run(&aapt2, &[
        "link".into(),
        "-o".into(), unsigned.display().to_string(),
        "-I".into(), platform.display().to_string(),
        "--manifest".into(), work.join("app/src/main/AndroidManifest.xml").display().to_string(),
        "--auto-add-overlay".into(),
        "--min-sdk-version".into(), "23".into(),
        "--target-sdk-version".into(), "35".into(),
        "-A".into(), work.join("assets").display().to_string(),
    ], Some(&work))?;

    let lib_dir = work.join("lib/arm64-v8a");
    fs::create_dir_all(&lib_dir)
        .map_err(|e| format!("cannot create native library APK directory: {}", e))?;
    fs::copy(&lib, lib_dir.join("libnova_main.so"))
        .map_err(|e| format!("cannot stage ARM64 library: {}", e))?;

    let zip = shell_tool("zip")?;
    run(&zip, &[
        "-0".into(),
        unsigned.display().to_string(),
        "lib/arm64-v8a/libnova_main.so".into(),
    ], Some(&work))?;

    let aligned = work.join("aligned.apk");
    run(&zipalign, &[
        "-p".into(),
        "-f".into(),
        "4".into(),
        unsigned.display().to_string(),
        aligned.display().to_string(),
    ], None)?;

    let keystore = work.join("nova-debug.keystore");
    run(&keytool, &[
        "-genkeypair".into(),
        "-keystore".into(), keystore.display().to_string(),
        "-storepass".into(), "android".into(),
        "-keypass".into(), "android".into(),
        "-alias".into(), "nova".into(),
        "-keyalg".into(), "RSA".into(),
        "-keysize".into(), "2048".into(),
        "-validity".into(), "10000".into(),
        "-dname".into(), "CN=NOVA,O=NOVA,C=AO".into(),
        "-noprompt".into(),
    ], None)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }

    run(&apksigner, &[
        "sign".into(),
        "--ks".into(), keystore.display().to_string(),
        "--ks-pass".into(), "pass:android".into(),
        "--key-pass".into(), "pass:android".into(),
        "--out".into(), output.display().to_string(),
        aligned.display().to_string(),
    ], None)?;

    run(&apksigner, &[
        "verify".into(),
        "--verbose".into(),
        output.display().to_string(),
    ], None)?;

    fs::remove_dir_all(&work).ok();
    Ok(output.to_path_buf())
}
