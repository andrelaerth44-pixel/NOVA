use crate::app_ast::*;

pub fn validate_android_target(app: &AppDeclRoot) -> Result<(), String> {
    if app.name.trim().is_empty() { return Err("Android app name cannot be empty".into()); }
    for decl in &app.declarations {
        match decl {
            AppDecl::Auth(a) if a.providers.is_empty() => return Err("auth block requires a provider".into()),
            AppDecl::Database(d) if d.provider.is_empty() => return Err("database provider cannot be empty".into()),
            AppDecl::Storage(s) if s.buckets.is_empty() => return Err("storage block requires at least one bucket".into()),
            _ => {}
        }
    }
    Ok(())
}

pub fn emit_android_project(app: &AppDeclRoot) -> Result<String, String> {
    validate_android_target(app)?;
    // Android is a native NOVA target. The generated Main.nova is the application
    // source itself; no Kotlin/Java/C++ bridge is generated.
    let mut source = format!("app \"{}\" {{\n", app.name);
    for decl in &app.declarations {
        match decl {
            AppDecl::Auth(x) => {
                source.push_str("    auth {\n");
                for p in &x.providers { source.push_str(&format!("        provider {}\n", p)); }
                source.push_str("    }\n");
            }
            AppDecl::Database(x) => {
                source.push_str(&format!("    database {} {{\n", x.provider));
                for t in &x.tables { source.push_str(&format!("        table {}\n", t)); }
                source.push_str("    }\n");
            }
            AppDecl::Storage(x) => {
                source.push_str("    storage {\n");
                for b in &x.buckets { source.push_str(&format!("        bucket {}\n", b)); }
                source.push_str("    }\n");
            }
            AppDecl::Navigation(routes) => {
                source.push_str("    navigation {\n");
                for r in routes { source.push_str(&format!("        {}\n", r.name)); }
                source.push_str("    }\n");
            }
            AppDecl::Screen(x) => {
                source.push_str(&format!("    screen \"{}\" {{\n", x.name));
                for item in &x.items {
                    match item {
                        ScreenItem::List { resource } => source.push_str(&format!("        list {}\n", resource)),
                        ScreenItem::Button { label, action } => source.push_str(&format!("        button \"{}\" {{ {} }}\n", label, action)),
                    }
                }
                source.push_str("    }\n");
            }
        }
    }
    source.push_str("}\n");
    Ok(source)
}

fn sanitize(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); } else if out.ends_with('_') { continue } else { out.push('_'); }
    }
    if out.is_empty() { "app".into() } else { out }
}


pub fn emit_android_project_files(app: &AppDeclRoot) -> Result<Vec<(String,String)>,String>{
    validate_android_target(app)?;
    let package = format!("com.nova.generated.{}", sanitize(&app.name));
    let source = emit_android_project(app)?;

    // The manifest names Android's platform NativeActivity only as the host
    // entry point. Application logic remains entirely in NOVA and is expected
    // to be compiled to the native Android ABI by the NOVA compiler.
    let manifest = format!(r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application android:hasCode="false" android:extractNativeLibs="true" android:theme="@android:style/Theme.Material.Light.NoActionBar" android:label="{}">
        <activity android:name="android.app.NativeActivity" android:exported="true">
            <meta-data android:name="android.app.lib_name" android:value="nova_main"/>
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
"#, app.name);
    let nova_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
language = "nova"

[target.android]
abi = ["arm64-v8a", "armeabi-v7a"]
entry = "Main.nova"
package = "{}"
"#, sanitize(&app.name), package);

    Ok(vec![
        ("Main.nova".into(), source),
        ("nova.toml".into(), nova_toml),
        ("app/src/main/AndroidManifest.xml".into(), manifest),
        ("README.nova-target".into(), "This Android application contains NOVA source only. The NOVA compiler must produce the native Android ARM64 runtime/library and package it into the APK.".into()),
    ])
}


