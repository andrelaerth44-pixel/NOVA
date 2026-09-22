pub const BOOTSTRAP_SOURCE:&str=r#"
fn checksum(text) {
    return len(text)
}
fn main() {
    print checksum("NOVA bootstrap")
}
"#;

pub fn bootstrap_report()->String{
    format!("NOVA self-host boundary: compile_source={} bytes; trusted bootstrap=Rust; next replacement units=lexer, parser, semantic, IR",BOOTSTRAP_SOURCE.len())
}
pub fn validate_bootstrap_source()->Result<(),String>{
    crate::compiler::parse_source(BOOTSTRAP_SOURCE).map(|_|()).map_err(|e|format!("bootstrap source invalid: {}",e))
}
