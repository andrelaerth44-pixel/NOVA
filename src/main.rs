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

pub use token::Token;
pub use ast::{Expr, Stmt, Value};
pub use lexer::lex;
pub use parser::Parser;

use std::{collections::HashMap, env, fs};

#[derive(Clone)]
struct Function { args:Vec<String>, body:Vec<Stmt> }
struct Vm { vars:HashMap<String,Value>, fns:HashMap<String,Function>, modules:HashMap<String,bool>, module_stack:Vec<std::path::PathBuf> }
impl Vm {
    fn new()->Self{Self{vars:HashMap::new(),fns:HashMap::new(),modules:HashMap::new(),module_stack:Vec::new()}}
    fn eval(&mut self,e:&Expr)->Result<Value,String>{
        match e {
            Expr::Val(v)=>Ok(v.clone()), Expr::Var(n)=>self.vars.get(n).cloned().ok_or_else(||format!("undefined variable {}",n)),
            Expr::Array(a)=>Ok(Value::Array(a.iter().map(|x|self.eval(x)).collect::<Result<_,_>>()?)),
            Expr::Unary(op,x)=>{let v=self.eval(x)?;match op{Token::Minus=>match v{Value::Num(n)=>Ok(Value::Num(-n)),_=>Err("unary - expects number".into())},Token::Bang=>Ok(Value::Bool(!v.truth())),_=>Err("bad unary".into())}},
            Expr::Binary(a,op,b)=>{let x=self.eval(a)?;if *op==Token::And&&!x.truth(){return Ok(Value::Bool(false))}if *op==Token::Or&&x.truth(){return Ok(Value::Bool(true))}let y=self.eval(b)?;self.bin(x,op,y)},
            Expr::Call(n,a)=>{
                if n=="range" {if a.len()!=2{return Err("range expects 2 arguments".into())}let x=self.eval(&a[0])?;let y=self.eval(&a[1])?;let (x,y)=(num(x)? as i64,num(y)? as i64);return Ok(Value::Array((x..y).map(|n|Value::Num(n as f64)).collect()));}
                if n=="str" {if a.len()!=1{return Err("str expects 1 argument".into())}return Ok(Value::Str(self.eval(&a[0])?.to_string()));}
                if n=="len" {if a.len()!=1{return Err("len expects 1 argument".into())}let v=self.eval(&a[0])?;return Ok(Value::Num(match v{Value::Str(x)=>x.chars().count() as f64,Value::Array(x)=>x.len() as f64,_=>return Err("len expects string or array".into())}));}
                if n=="abs" {if a.len()!=1{return Err("abs expects 1 argument".into())}return Ok(Value::Num(num(self.eval(&a[0])?)?.abs()));}
                if n=="sqrt" {if a.len()!=1{return Err("sqrt expects 1 argument".into())}let v=num(self.eval(&a[0])?)?;if v<0.0{return Err("sqrt expects a non-negative number".into())}return Ok(Value::Num(v.sqrt()));}
                if n=="read_file" {if a.len()!=1{return Err("read_file expects 1 argument".into())}let path=self.eval(&a[0])?;let path=match path{Value::Str(x)=>x,_=>return Err("read_file expects a string path".into())};return Ok(Value::Str(fs::read_to_string(&path).map_err(|e|format!("cannot read {}: {}",path,e))?));}
                if n=="write_file" {if a.len()!=2{return Err("write_file expects 2 arguments".into())}let path=self.eval(&a[0])?;let data=self.eval(&a[1])?;let path=match path{Value::Str(x)=>x,_=>return Err("write_file expects a string path".into())};let data=match data{Value::Str(x)=>x,_=>return Err("write_file expects string data".into())};fs::write(&path,&data).map_err(|e|format!("cannot write {}: {}",path,e))?;return Ok(Value::Null);}
                if n=="exists" {if a.len()!=1{return Err("exists expects 1 argument".into())}let path=self.eval(&a[0])?;let path=match path{Value::Str(x)=>x,_=>return Err("exists expects a string path".into())};return Ok(Value::Bool(std::path::Path::new(&path).exists()));}
                if n=="env" {if a.len()!=1{return Err("env expects 1 argument".into())}let key=self.eval(&a[0])?;let key=match key{Value::Str(x)=>x,_=>return Err("env expects a string key".into())};return Ok(std::env::var(&key).map(Value::Str).unwrap_or(Value::Null));}
                let f=self.fns.get(n).cloned().ok_or_else(||format!("undefined function {}",n))?;
                if f.args.len()!=a.len(){return Err(format!("{} expects {} arguments",n,f.args.len()))}
                let old=self.vars.clone();for(i,k)in f.args.iter().enumerate(){self.vars.insert(k.clone(),self.eval(&a[i])?);}
                let r=self.exec(&f.body)?;self.vars=old;Ok(r.unwrap_or(Value::Null))
            }
        }
    }
    fn bin(&self,a:Value,o:&Token,b:Value)->Result<Value,String>{
        match o {
            Token::Plus=>match(a,b){(Value::Num(x),Value::Num(y))=>Ok(Value::Num(x+y)),(Value::Str(x),Value::Str(y))=>Ok(Value::Str(x+&y)),_=>Err("unsupported +".into())},
            Token::Minus=>num2(a,b,|x,y|x-y),Token::Star=>num2(a,b,|x,y|x*y),Token::Slash=>div2(a,b),Token::Percent=>mod2(a,b),
            Token::EqEq=>Ok(Value::Bool(a.equals(&b))),Token::Ne=>Ok(Value::Bool(!a.equals(&b))),
            Token::Lt=>cmp2(a,b,|x,y|x<y),Token::Le=>cmp2(a,b,|x,y|x<=y),Token::Gt=>cmp2(a,b,|x,y|x>y),Token::Ge=>cmp2(a,b,|x,y|x>=y),
            Token::And=>Ok(Value::Bool(a.truth()&&b.truth())),Token::Or=>Ok(Value::Bool(a.truth()||b.truth())),_=>Err("bad operator".into())
        }
    }
    fn exec(&mut self,s:&[Stmt])->Result<Option<Value>,String>{
        for x in s {match x{
            Stmt::Expr(e)=>{self.eval(e)?;}, Stmt::Let(n,_,e)|Stmt::Assign(n,e)=>{let v=self.eval(e)?;self.vars.insert(n.clone(),v);},
            Stmt::Print(e)=>println!("{}",self.eval(e)?), Stmt::Return(e)=>return Ok(Some(self.eval(e)?)),
            Stmt::If(c,a,b)=>{if self.eval(c)?.truth(){if let Some(v)=self.exec(a)?{return Ok(Some(v))}}else if let Some(v)=self.exec(b)?{return Ok(Some(v))}},
            Stmt::While(c,b)=>{while self.eval(c)?.truth(){if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},
            Stmt::For(n,it,b)=>{let v=self.eval(it)?;match v{Value::Array(xs)=>{for x in xs{self.vars.insert(n.clone(),x);if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},_=>return Err("for expects an array or range".into())}},
            Stmt::Match(value,arms,otherwise)=>{let v=self.eval(value)?;let mut done=false;for (pat,body) in arms{if self.eval(pat)?.to_string()==v.to_string(){if let Some(r)=self.exec(body)?{return Ok(Some(r))}done=true;break}}if !done{if let Some(r)=self.exec(otherwise)?{return Ok(Some(r))}}},
            Stmt::Import(path)=>{ let resolved={let p=std::path::Path::new(path);if p.is_absolute(){p.to_path_buf()}else if let Some(base)=self.module_stack.last(){base.join(p)}else{p.to_path_buf()}}; let key=resolved.to_string_lossy().to_string(); if !self.modules.contains_key(&key){let src=fs::read_to_string(&resolved).map_err(|e|format!("cannot import {}: {}",resolved.display(),e))?;let toks=lex(&src)?;let mut p=Parser::new(toks);let program=p.program()?;self.modules.insert(key.clone(),true);let parent=resolved.parent().map(|x|x.to_path_buf()).unwrap_or_else(||std::path::PathBuf::from("."));self.module_stack.push(parent);let r=self.exec(&program);self.module_stack.pop();r?;}},
            Stmt::Fn(n,a,_,b)=>{self.fns.insert(n.clone(),Function{args:a.iter().map(|x|x.0.clone()).collect(),body:b.clone()});}
        }}Ok(None)
    }
}
fn num(v:Value)->Result<f64,String>{match v{Value::Num(n)=>Ok(n),_=>Err("number expected".into())}}
fn num2(a:Value,b:Value,f:fn(f64,f64)->f64)->Result<Value,String>{Ok(Value::Num(f(num(a)?,num(b)?)))}
fn cmp2(a:Value,b:Value,f:fn(f64,f64)->bool)->Result<Value,String>{Ok(Value::Bool(f(num(a)?,num(b)?)))}
fn div2(a:Value,b:Value)->Result<Value,String>{let x=num(a)?;let y=num(b)?;if y==0.0{return Err("division by zero".into())}Ok(Value::Num(x/y))}
fn mod2(a:Value,b:Value)->Result<Value,String>{let x=num(a)?;let y=num(b)?;if y==0.0{return Err("modulo by zero".into())}Ok(Value::Num(x%y))}

#[derive(Clone, Debug)]
struct StaticFn {
    args: Vec<types::Type>,
    ret: types::Type,
}

struct Checker {
    scopes: Vec<HashMap<String, types::Type>>,
    fns: HashMap<String, StaticFn>,
    errors: Vec<String>,
}

impl Checker {
    fn new() -> Self { Self { scopes: vec![HashMap::new()], fns: HashMap::new(), errors: Vec::new() } }
    fn error(&mut self, msg: impl Into<String>) { self.errors.push(msg.into()); }
    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }
    fn lookup(&self, name: &str) -> Option<types::Type> { self.scopes.iter().rev().find_map(|s| s.get(name).cloned()) }
    fn contains(&self, name: &str) -> bool { self.scopes.iter().rev().any(|s| s.contains_key(name)) }
    fn define(&mut self, name: String, ty: types::Type) { if let Some(s) = self.scopes.last_mut() { s.insert(name, ty); } }

    fn infer(&mut self, e: &Expr) -> types::Type {
        match e {
            Expr::Val(Value::Num(_)) => types::Type::Number,
            Expr::Val(Value::Str(_)) => types::Type::String,
            Expr::Val(Value::Bool(_)) => types::Type::Bool,
            Expr::Val(Value::Null) => types::Type::Null,
            Expr::Val(Value::Array(xs)) => {
                if xs.is_empty() { return types::Type::Array(Box::new(types::Type::Any)); }
                let first = self.infer(&Expr::Val(xs[0].clone()));
                for v in xs.iter().skip(1) {
                    let t = self.infer(&Expr::Val(v.clone()));
                    if !first.compatible(&t) { self.error(format!("array elements have incompatible types: {} and {}", first.name(), t.name())); }
                }
                types::Type::Array(Box::new(first))
            }
            Expr::Var(n) => self.lookup(n).unwrap_or_else(|| {
                self.error(format!("undefined variable {}", n)); types::Type::Unknown
            }),
            Expr::Array(xs) => {
                if xs.is_empty() { return types::Type::Array(Box::new(types::Type::Any)); }
                let first = self.infer(&xs[0]);
                for x in xs.iter().skip(1) {
                    let t = self.infer(x);
                    if !first.compatible(&t) { self.error(format!("array elements have incompatible types: {} and {}", first.name(), t.name())); }
                }
                types::Type::Array(Box::new(first))
            }
            Expr::Unary(op, x) => {
                let t = self.infer(x);
                match op {
                    Token::Minus if !t.compatible(&types::Type::Number) => {
                        self.error(format!("unary - expects number, got {}", t.name())); types::Type::Unknown
                    }
                    Token::Minus => types::Type::Number,
                    Token::Bang => types::Type::Bool,
                    _ => types::Type::Unknown,
                }
            }
            Expr::Binary(a, op, b) => {
                let x = self.infer(a); let y = self.infer(b);
                match op {
                    Token::Plus => {
                        if x.compatible(&types::Type::Number) && y.compatible(&types::Type::Number) { types::Type::Number }
                        else if x.compatible(&types::Type::String) && y.compatible(&types::Type::String) { types::Type::String }
                        else { self.error(format!("operator + cannot combine {} and {}", x.name(), y.name())); types::Type::Unknown }
                    }
                    Token::Minus | Token::Star | Token::Slash | Token::Percent => {
                        if !x.compatible(&types::Type::Number) || !y.compatible(&types::Type::Number) {
                            self.error(format!("arithmetic operator expects numbers, got {} and {}", x.name(), y.name())); types::Type::Unknown
                        } else { types::Type::Number }
                    }
                    Token::Lt | Token::Le | Token::Gt | Token::Ge => {
                        if !x.compatible(&types::Type::Number) || !y.compatible(&types::Type::Number) {
                            self.error(format!("comparison expects numbers, got {} and {}", x.name(), y.name()));
                        }
                        types::Type::Bool
                    }
                    Token::EqEq | Token::Ne | Token::And | Token::Or => types::Type::Bool,
                    _ => types::Type::Unknown,
                }
            }
            Expr::Call(n, args) => {
                if n == "range" {
                    if args.len() != 2 { self.error("range expects 2 arguments"); }
                    for a in args { let t = self.infer(a); if !t.compatible(&types::Type::Number) { self.error("range arguments must be numbers"); } }
                    return types::Type::Array(Box::new(types::Type::Number));
                }
                if n == "str" {
                    if args.len() != 1 { self.error("str expects 1 argument"); }
                    for a in args { self.infer(a); }
                    return types::Type::String;
                }
                if n == "len" {
                    if args.len() != 1 { self.error("len expects 1 argument"); }
                    if let Some(a) = args.first() {
                        let t = self.infer(a);
                        if !matches!(t, types::Type::String | types::Type::Array(_) | types::Type::Any | types::Type::Unknown) {
                            self.error(format!("len expects string or array, got {}", t.name()));
                        }
                    }
                    return types::Type::Number;
                }
                if n == "abs" || n == "sqrt" {
                    if args.len() != 1 { self.error(format!("{} expects 1 argument", n)); }
                    if let Some(a) = args.first() {
                        let t = self.infer(a);
                        if !t.compatible(&types::Type::Number) { self.error(format!("{} expects a number, got {}", n, t.name())); }
                    }
                    return types::Type::Number;
                }
                if n == "read_file" {
                    if args.len() != 1 { self.error("read_file expects 1 argument"); }
                    if let Some(a) = args.first() { let t=self.infer(a); if !t.compatible(&types::Type::String) { self.error(format!("read_file expects a string path, got {}", t.name())); } }
                    return types::Type::String;
                }
                if n == "write_file" {
                    if args.len() != 2 { self.error("write_file expects 2 arguments"); }
                    if let Some(a)=args.first(){let t=self.infer(a);if !t.compatible(&types::Type::String){self.error(format!("write_file expects a string path, got {}",t.name()));}}
                    if let Some(a)=args.get(1){let t=self.infer(a);if !t.compatible(&types::Type::String){self.error(format!("write_file expects string data, got {}",t.name()));}}
                    return types::Type::Null;
                }
                if n == "exists" || n == "env" {
                    if args.len() != 1 { self.error(format!("{} expects 1 argument", n)); }
                    if let Some(a)=args.first(){let t=self.infer(a);if !t.compatible(&types::Type::String){self.error(format!("{} expects a string, got {}",n,t.name()));}}
                    return if n=="exists" { types::Type::Bool } else { types::Type::String };
                }
                let f = match self.fns.get(n).cloned() {
                    Some(f) => f,
                    None => {
                        self.error(format!("undefined function {}", n));
                        for a in args { self.infer(a); }
                        return types::Type::Unknown;
                    }
                };
                if f.args.len() != args.len() { self.error(format!("{} expects {} arguments, got {}", n, f.args.len(), args.len())); }
                for (i, a) in args.iter().enumerate() {
                    let t = self.infer(a);
                    if let Some(expected) = f.args.get(i) {
                        if !expected.compatible(&t) { self.error(format!("argument {} of {} expects {}, got {}", i + 1, n, expected.name(), t.name())); }
                    }
                }
                f.ret
            }
        }
    }

    fn check_block(&mut self, body: &[Stmt], expected_return: Option<types::Type>) {
        for s in body {
            match s {
                Stmt::Let(n, explicit, e) | Stmt::Assign(n, e) => {
                    let t = self.infer(e);
                    if let Some(expected) = explicit {
                        if !expected.compatible(&t) { self.error(format!("type annotation for {} expects {}, got {}", n, expected.name(), t.name())); }
                    }
                    if matches!(s, Stmt::Assign(_, _)) && !self.contains(n) { self.error(format!("assignment to undefined variable {}", n)); }
                    if let Some(old) = self.lookup(n) {
                        if !old.compatible(&t) { self.error(format!("cannot assign {} to {} (expected {})", t.name(), n, old.name())); }
                    }
                    self.define(n.clone(), t);
                }
                Stmt::Print(e) | Stmt::Expr(e) => { self.infer(e); }
                Stmt::Return(e) => {
                    if expected_return.is_none() { self.error("return outside function"); }
                    let t = self.infer(e);
                    if let Some(expected) = &expected_return {
                        if !expected.compatible(&t) { self.error(format!("return type mismatch: expected {}, got {}", expected.name(), t.name())); }
                    }
                }
                Stmt::If(c, a, b) => {
                    let t = self.infer(c);
                    if !t.compatible(&types::Type::Bool) && !t.compatible(&types::Type::Number) { self.error(format!("condition must be bool or number, got {}", t.name())); }
                    self.push_scope();
                    self.check_block(a, expected_return.clone());
                    self.pop_scope();
                    self.push_scope();
                    self.check_block(b, expected_return.clone());
                    self.pop_scope();
                }
                Stmt::While(c, b) => {
                    let t = self.infer(c);
                    if !t.compatible(&types::Type::Bool) && !t.compatible(&types::Type::Number) { self.error(format!("condition must be bool or number, got {}", t.name())); }
                    self.check_block(b, expected_return.clone());
                }
                Stmt::For(n, it, b) => {
                    match self.infer(it) {
                        types::Type::Array(inner) => {
                            self.push_scope();
                            self.define(n.clone(), *inner);
                            self.check_block(b, expected_return.clone());
                            self.pop_scope();
                        }
                        types::Type::Any | types::Type::Unknown => {
                            self.push_scope();
                            self.define(n.clone(), types::Type::Any);
                            self.check_block(b, expected_return.clone());
                            self.pop_scope();
                        }
                        other => self.error(format!("for expects an array, got {}", other.name())),
                    }
                }
                Stmt::Match(value, arms, otherwise) => {
                    let vt = self.infer(value);
                    for (pat, body) in arms {
                        let pt = self.infer(pat);
                        if !vt.compatible(&pt) { self.error(format!("match pattern type {} does not match {}", pt.name(), vt.name())); }
                        self.check_block(body, expected_return.clone());
                    }
                    self.check_block(otherwise, expected_return.clone());
                }
                Stmt::Import(_) => {}
                Stmt::Fn(n, args, ret, body) => {
                    if self.fns.contains_key(n) { self.error(format!("duplicate function {}", n)); continue; }
                    self.fns.insert(n.clone(), StaticFn { args: args.iter().map(|x| x.1.clone()).collect(), ret: ret.clone() });
                    self.push_scope();
                    for (a, t) in args { self.define(a.clone(), t.clone()); }
                    self.check_block(body, Some(ret.clone()));
                    self.pop_scope();
                }
            }
        }
    }

    fn check(&mut self, program: &[Stmt]) -> Result<(), Vec<String>> {
        self.check_block(program, None);
        if self.errors.is_empty() { Ok(()) } else { Err(self.errors.clone()) }
    }
}

fn run_semantic_check(program: &[Stmt]) -> Result<(), String> {
    let mut checker = Checker::new();
    checker.check(program).map_err(|errs| errs.join("\n"))
}

fn main(){
    let a:Vec<String>=env::args().collect();
    if a.len()<2 {eprintln!("NOVA 1.5.0\nusage: nova run <file> | nova check <file> | nova ir <file> | nova build-c <file> [output.c] | nova build-native <file> [output] | nova version");return}
    if a[1]=="version"{println!("NOVA 1.5.0");return}
    if a.len()<3 {eprintln!("missing file");std::process::exit(2)}
    let src=match fs::read_to_string(&a[2]){Ok(x)=>x,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
    let t=match lex(&src){Ok(x)=>x,Err(e)=>{eprintln!("lex error: {}",e);std::process::exit(1)}};
    let mut p=Parser::new(t);
    let program=match p.program(){Ok(x)=>x,Err(e)=>{eprintln!("parse error: {}",e);std::process::exit(1)}};
    if a[1]=="check"{if let Err(e)=run_semantic_check(&program){eprintln!("semantic error:\n{}",e);std::process::exit(1)}let m=optimizer::optimize(lower::lower(&program));if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}println!("ok");return}
    if a[1]=="ir"{let m=optimizer::optimize(lower::lower(&program));if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}print!("{}",ir::format_module(&m));return}
    if a[1]=="build-c"||a[1]=="build-native"{let m=optimizer::optimize(lower::lower(&program));if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}let out=if a.len()>3{&a[3]}else{"a.out"};let c=match backend_c::emit_c(&m){Ok(x)=>x,Err(e)=>{eprintln!("native backend error: {}",e);std::process::exit(1)}};if a[1]=="build-c"{if let Err(e)=fs::write(out,&c){eprintln!("cannot write {}: {}",out,e);std::process::exit(1)}println!("{}",out);return}let status=std::process::Command::new("cc").args(["-O3","-std=c11","-x","c","-","-o",out]).stdin(std::process::Stdio::piped()).spawn().and_then(|mut child|{use std::io::Write;if let Some(mut stdin)=child.stdin.take(){stdin.write_all(c.as_bytes())?;}child.wait()});match status{Ok(s) if s.success()=>println!("{}",out),Ok(s)=>{eprintln!("C compiler exited with {}",s);std::process::exit(1)},Err(e)=>{eprintln!("cannot invoke cc: {}",e);std::process::exit(1)}}return}
    if a[1]!="run"{eprintln!("unknown command {}",a[1]);std::process::exit(2)}
    let mut vm=Vm::new();let file_path=std::path::Path::new(&a[2]);let abs=fs::canonicalize(file_path).unwrap_or_else(|_|file_path.to_path_buf());vm.modules.insert(abs.to_string_lossy().to_string(),true);vm.module_stack.push(abs.parent().map(|x|x.to_path_buf()).unwrap_or_else(||std::path::PathBuf::from(".")));if let Err(e)=vm.exec(&program){eprintln!("runtime error: {}",e);std::process::exit(1)}
}


#[cfg(test)]
mod tests {
    use super::*;
    fn run(src:&str)->Result<Vm,String>{let tokens=lex(src)?;let mut p=Parser::new(tokens);let program=p.program()?;let mut vm=Vm::new();vm.exec(&program)?;Ok(vm)}
    #[test] fn arithmetic(){let vm=run("let x = 2 + 3 * 4").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"14");}
    #[test] fn functions(){let vm=run("fn add(a,b) { return a+b } let x = add(20,22)").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"42");}
    #[test] fn arrays(){let vm=run("let xs = [1,2,3,4]").unwrap();assert_eq!(vm.vars.get("xs").unwrap().to_string(),"[1, 2, 3, 4]");}
    #[test] fn match_exec(){let vm=run("let x = 2 match x { 1 { let y = 10 } 2 { let y = 20 } else { let y = 30 } }").unwrap();assert_eq!(vm.vars.get("y").unwrap().to_string(),"20");}
    #[test] fn divide_zero(){assert!(run("let x = 10 / 0").is_err());}
    #[test] fn builtin_arity(){assert!(run("let x = len([1])").is_ok());assert!(run("let x = len([1],[2])").is_err());}
    #[test] fn short_circuit(){let vm=run("let x = false and (1 / 0)").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"false");}
    #[test] fn typed_equality(){assert_eq!(run("let x = 1 == \"1\"").unwrap().vars.get("x").unwrap().to_string(),"false");assert_eq!(run("let x = [1,2] == [1,2]").unwrap().vars.get("x").unwrap().to_string(),"true");}
    #[test] fn relative_imports(){let dir=std::env::temp_dir().join("nova_import_test");let _=std::fs::create_dir_all(&dir);let child=dir.join("child.nova");let main=dir.join("main.nova");std::fs::write(&child,"let imported = 42").unwrap();std::fs::write(&main,"import \"child.nova\"").unwrap();let src=std::fs::read_to_string(&main).unwrap();let mut vm=Vm::new();vm.modules.insert(main.to_string_lossy().to_string(),true);vm.module_stack.push(dir.clone());let p=Parser::new(lex(&src).unwrap());let mut p=p;let program=p.program().unwrap();vm.exec(&program).unwrap();assert_eq!(vm.vars.get("imported").unwrap().to_string(),"42");let _=std::fs::remove_dir_all(&dir);}\n
    #[test] fn math_builtins(){let vm=run("let x = abs(-4) let y = sqrt(9)").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"4");assert_eq!(vm.vars.get("y").unwrap().to_string(),"3");}
    #[test] fn filesystem_builtins(){let path="/tmp/nova_test_runtime.txt";let src=format!("write_file(\"{}\", \"hello\") let x = read_file(\"{}\")",path,path);let vm=run(&src).unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"hello");assert_eq!(run(&format!("let x = exists(\"{}\")",path)).unwrap().vars.get("x").unwrap().to_string(),"true");let _=std::fs::remove_file(path);}

}
