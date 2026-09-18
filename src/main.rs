mod token;
mod ast;
mod lexer;
mod parser;
mod span;
mod types;
mod ir;
mod diagnostics;
mod lower;
mod optimizer;
mod backend_c;
mod app_ast;
mod app_parser;
mod app_backend_android;
mod runtime;
mod semantic;
mod ssa_lower;
mod module_loader;
mod package;
mod compiler;

pub use token::Token;
pub use ast::{Expr, Stmt, Value, Pattern, MapKey, EnvFrame, EnvRef, to_map_key};
pub use lexer::lex;
pub use parser::Parser;

use std::{env, fs};
use runtime::Vm;
use module_loader::ModuleLoader;

fn run_semantic_check(program: &[Stmt]) -> Result<(), String> {
    let mut checker = semantic::Checker::new();
    checker.check(program).map_err(|errors| errors.join("\n"))
}

fn load_program(path: &str) -> Result<Vec<Stmt>, String> {
    let mut loader = ModuleLoader::new();
    loader.load_entry(std::path::Path::new(path))
}

fn main(){
    let a:Vec<String>=env::args().collect();
    if a.len()<2 {
        eprintln!("NOVA 1.7.0\nusage: nova run <file> | nova check <file> | nova ir <file> | nova ssa <file> | nova build-c <file> [output.c] | nova build-native <file> [output] | nova app-check <file> | nova build-android <file> [MainActivity.kt] | nova package-check <manifest-or-dir> | nova package-lock <manifest-or-dir> | nova version");
        return
    }
    if a[1]=="version"{println!("NOVA 1.7.0");return}

    if a[1]=="package-check" || a[1]=="package-lock" {
        if a.len()<3 { eprintln!("missing manifest path"); std::process::exit(2); }
        let path = std::path::Path::new(&a[2]);
        let packages = match package::resolve_manifest(path) {
            Ok(x) => x,
            Err(e) => { eprintln!("package error: {}", e); std::process::exit(1); }
        };
        if a[1]=="package-lock" {
            match package::write_lock(path) {
                Ok(lock) => println!("{}", lock.display()),
                Err(e) => { eprintln!("package error: {}", e); std::process::exit(1); }
            }
        } else {
            for p in packages { println!("{} {} {}", p.name, p.version, p.source); }
            println!("ok");
        }
        return
    }

    if a[1]=="app-check" {
        let src=match fs::read_to_string(&a[2]){Ok(x)=>x,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
        let app=match app_parser::AppParser::new(&src).parse(){Ok(x)=>x,Err(e)=>{eprintln!("app parse error: {}",e);std::process::exit(1)}};
        if let Err(e)=app_backend_android::validate_android_target(&app){eprintln!("Android target error: {}",e);std::process::exit(1)}
        println!("ok: Android application target {}", app.name);
        return
    }

    if a[1]=="build-android" {
        let src=match fs::read_to_string(&a[2]){Ok(x)=>x,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
        let app=match app_parser::AppParser::new(&src).parse(){Ok(x)=>x,Err(e)=>{eprintln!("app parse error: {}",e);std::process::exit(1)}};
        let kotlin=match app_backend_android::emit_android_project(&app){Ok(x)=>x,Err(e)=>{eprintln!("Android backend error: {}",e);std::process::exit(1)}};
        let out=if a.len()>3{&a[3]}else{"MainActivity.kt"};
        if let Err(e)=fs::write(out,kotlin){eprintln!("cannot write {}: {}",out,e);std::process::exit(1)}
        println!("{}",out);
        return
    }

    if a.len()<3 {eprintln!("missing file");std::process::exit(2)}
    let entry=&a[2];
    let program=match load_program(entry){
        Ok(x)=>x,
        Err(e)=>{eprintln!("{}",e);std::process::exit(1)}
    };

    if a[1]=="ssa"{
        let src=match fs::read_to_string(&a[2]){Ok(x)=>x,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
        let compiled=match compiler::compile_source(&src){Ok(x)=>x,Err(e)=>{eprintln!("compile error:\\n{}",e);std::process::exit(1)}};
        print!("{}", compiler::format_ssa(&compiled.ssa));
        return
    }

    if a[1]=="check"{
        if let Err(e)=run_semantic_check(&program){eprintln!("semantic error:\n{}",e);std::process::exit(1)}
        let m=optimizer::optimize(lower::lower(&program));
        if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}
        if let Err(e)=ssa_lower::verify_program(&program){eprintln!("SSA error: {}",e);std::process::exit(1)}
        println!("ok");
        return
    }

    if a[1]=="ir"{
        let m=optimizer::optimize(lower::lower(&program));
        if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}
        print!("{}",ir::format_module(&m));
        return
    }

    if a[1]=="build-c"||a[1]=="build-native"{
        let m=optimizer::optimize(lower::lower(&program));
        if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}
        let out=if a.len()>3{&a[3]}else{"a.out"};
        let c=match backend_c::emit_c(&m){Ok(x)=>x,Err(e)=>{eprintln!("native backend error: {}",e);std::process::exit(1)}};
        if a[1]=="build-c"{
            if let Err(e)=fs::write(out,&c){eprintln!("cannot write {}: {}",out,e);std::process::exit(1)}
            println!("{}",out);
            return
        }
        let status=std::process::Command::new("cc")
            .args(["-O3","-std=c11","-x","c","-","-o",out])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child|{
                use std::io::Write;
                if let Some(mut stdin)=child.stdin.take(){stdin.write_all(c.as_bytes())?;}
                child.wait()
            });
        match status{
            Ok(s) if s.success()=>println!("{}",out),
            Ok(s)=>{eprintln!("C compiler exited with {}",s);std::process::exit(1)},
            Err(e)=>{eprintln!("cannot invoke cc: {}",e);std::process::exit(1)}
        }
        return
    }

    if a[1]!="run"{eprintln!("unknown command {}",a[1]);std::process::exit(2)}

    let mut vm=Vm::new();
    if let Err(e)=vm.exec(&program){eprintln!("runtime error: {}",e);std::process::exit(1)}
}
