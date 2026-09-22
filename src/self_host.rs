use std::path::Path;

pub const BOOTSTRAP_SOURCE:&str=r#"
fn checksum(text) {
    return len(text)
}
fn main() {
    print checksum("NOVA bootstrap")
}
"#;

pub fn bootstrap_report()->String{
    "NOVA self-host pipeline: Stage 0=Rust bootstrap; Stage 1=NOVA compiler sources validated; Stage 2=self-build is pending; final reproducibility check is pending".into()
}

pub fn validate_bootstrap_source()->Result<(),String>{
    crate::compiler::parse_source(BOOTSTRAP_SOURCE).map_err(|e|format!("bootstrap source invalid: {}",e))?;
    let root = Path::new("selfhost").join("stage1");
    if !root.join("nova.toml").is_file() { return Err("missing selfhost/stage1/nova.toml".into()); }
    let build = crate::project::check(&root).map_err(|e| format!("stage-1 NOVA compiler project invalid: {}",e))?;
    if build.source_files.is_empty() { return Err("stage-1 compiler has no .nova sources".into()); }
    Ok(())
}
