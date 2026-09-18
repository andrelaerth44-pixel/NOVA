use crate::{Expr, Stmt, Token, Value, Pattern, Parser, lex};
use std::{collections::HashMap, fs};

#[derive(Clone)]
struct Function { pub args: Vec<String>, pub body: Vec<Stmt> }

pub struct Vm {
    vars: HashMap<String, Value>,
    fns: HashMap<String, Function>,
    modules: HashMap<String, bool>,
    module_stack: Vec<std::path::PathBuf>,
}

impl Vm {
    pub fn new() -> Self { Self { vars: HashMap::new(), fns: HashMap::new(), modules: HashMap::new(), module_stack: Vec::new() } }
    pub fn exec_program(&mut self, program: &[Stmt]) -> Result<(), String> { self.exec(program).map(|_| ()) }
    fn eval(&mut self,e:&Expr)->Result<Value,String>{
        match e {
            Expr::Val(v)=>Ok(v.clone()),
            Expr::EnumInit(name, variant, value)=>Ok(Value::Enum{name:name.clone(),variant:variant.clone(),value:match value{Some(v)=>Some(Box::new(self.eval(v)?)),None=>None}}),
            Expr::StructInit(name, fields)=>{
                let mut out=HashMap::new();
                for (field, expr) in fields { out.insert(field.clone(), self.eval(expr)?); }
                Ok(Value::Struct{name:name.clone(),fields:out})
            }
            Expr::Field(base, field)=>{
                match self.eval(base)? {
                    Value::Struct{fields,..} => fields.get(field).cloned().ok_or_else(||format!("unknown field {}",field)),
                    _ => Err("field access requires struct".into())
                }
            }
            Expr::Var(n)=>self.vars.get(n).cloned().ok_or_else(||format!("undefined variable {}",n)),
            Expr::Array(a)=>Ok(Value::Array(a.iter().map(|x|self.eval(x)).collect::<Result<_,_>>()?)),
            Expr::Unary(op,x)=>{let v=self.eval(x)?;match op{Token::Minus=>match v{Value::Num(n)=>Ok(Value::Num(-n)),_=>Err("unary - expects number".into())},Token::Bang=>Ok(Value::Bool(!v.truth())),_=>Err("bad unary".into())}},
            Expr::Binary(a,op,b)=>{let x=self.eval(a)?;if *op==Token::And&&!x.truth(){return Ok(Value::Bool(false))}if *op==Token::Or&&x.truth(){return Ok(Value::Bool(true))}let y=self.eval(b)?;self.bin(x,op,y)},
            Expr::Call(n,a)=>{
                if n=="None" { if !a.is_empty(){return Err("None expects 0 arguments".into())} return Ok(Value::Enum{name:"Option".into(),variant:"None".into(),value:None}); }
                if n=="Some" { if a.len()!=1{return Err("Some expects 1 argument".into())} return Ok(Value::Enum{name:"Option".into(),variant:"Some".into(),value:Some(Box::new(self.eval(&a[0])?))}); }
                if n=="Ok" { if a.len()!=1{return Err("Ok expects 1 argument".into())} return Ok(Value::Enum{name:"Result".into(),variant:"Ok".into(),value:Some(Box::new(self.eval(&a[0])?))}); }
                if n=="Err" { if a.len()!=1{return Err("Err expects 1 argument".into())} return Ok(Value::Enum{name:"Result".into(),variant:"Err".into(),value:Some(Box::new(self.eval(&a[0])?))}); }
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
                let old=self.vars.clone();let vals=a.iter().map(|e|self.eval(e)).collect::<Result<Vec<_>,_>>()?;for(i,k)in f.args.iter().enumerate(){self.vars.insert(k.clone(),vals[i].clone());}
                let r=self.exec(&f.body)?;self.vars=old;Ok(r.unwrap_or(Value::Null))
            }
        }
    }
    fn matches_pattern(pattern:&Pattern,value:&Value,binding:&mut Option<Value>)->bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Literal(expr) => match expr {
                Expr::Val(v) => v.equals(value),
                _ => false,
            },
            Pattern::Enum{variant,..} => match value {
                Value::Enum{variant:actual,value:payload,..} if actual==variant => { *binding=payload.as_deref().cloned(); true },
                _ => false,
            }
        }
    }
    fn bin(&self,a:Value,o:&Token,b:Value)->Result<Value,String>{match o{Token::Plus=>match(a,b){(Value::Num(x),Value::Num(y))=>Ok(Value::Num(x+y)),(Value::Str(x),Value::Str(y))=>Ok(Value::Str(x+&y)),_=>Err("unsupported +".into())},Token::Minus=>num2(a,b,|x,y|x-y),Token::Star=>num2(a,b,|x,y|x*y),Token::Slash=>div2(a,b),Token::Percent=>mod2(a,b),Token::EqEq=>Ok(Value::Bool(a.equals(&b))),Token::Ne=>Ok(Value::Bool(!a.equals(&b))),Token::Lt=>cmp2(a,b,|x,y|x<y),Token::Le=>cmp2(a,b,|x,y|x<=y),Token::Gt=>cmp2(a,b,|x,y|x>y),Token::Ge=>cmp2(a,b,|x,y|x>=y),Token::And=>Ok(Value::Bool(a.truth()&&b.truth())),Token::Or=>Ok(Value::Bool(a.truth()||b.truth())),_=>Err("bad operator".into())}}
    fn exec(&mut self,s:&[Stmt])->Result<Option<Value>,String>{for x in s{match x{Stmt::Expr(e)=>{self.eval(e)?;},Stmt::Let(n,_,e)|Stmt::Assign(n,e)=>{let v=self.eval(e)?;self.vars.insert(n.clone(),v);},Stmt::Print(e)=>println!("{}",self.eval(e)?),Stmt::Return(e)=>return Ok(Some(self.eval(e)?)),Stmt::If(c,a,b)=>{if self.eval(c)?.truth(){if let Some(v)=self.exec(a)?{return Ok(Some(v))}}else if let Some(v)=self.exec(b)?{return Ok(Some(v))}},Stmt::While(c,b)=>{while self.eval(c)?.truth(){if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},Stmt::For(n,it,b)=>{let v=self.eval(it)?;match v{Value::Array(xs)=>{for x in xs{self.vars.insert(n.clone(),x);if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},_=>return Err("for expects an array or range".into())}},Stmt::Match(value,arms,otherwise)=>{let v=self.eval(value)?;let mut done=false;for(pattern,body)in arms{let mut binding=None;if Self::matches_pattern(pattern,&v,&mut binding){if let Pattern::Enum{binding:Some(name),..}=pattern{if let Some(value)=binding{self.vars.insert(name.clone(),value);}}if let Some(r)=self.exec(body)?{return Ok(Some(r))}done=true;break}}if !done{if let Some(r)=self.exec(otherwise)?{return Ok(Some(r))}}},Stmt::Import(path)=>{let resolved={let p=std::path::Path::new(path);if p.is_absolute(){p.to_path_buf()}else if let Some(base)=self.module_stack.last(){base.join(p)}else{p.to_path_buf()}};let key=resolved.to_string_lossy().to_string();if !self.modules.contains_key(&key){let src=fs::read_to_string(&resolved).map_err(|e|format!("cannot import {}: {}",resolved.display(),e))?;let toks=lex(&src)?;let mut p=Parser::new(toks);let program=p.program()?;self.modules.insert(key.clone(),true);let parent=resolved.parent().map(|x|x.to_path_buf()).unwrap_or_else(||std::path::PathBuf::from("."));self.module_stack.push(parent);let r=self.exec(&program);self.module_stack.pop();r?;}},Stmt::Fn(n,a,_,b)=>{self.fns.insert(n.clone(),Function{args:a.iter().map(|x|x.0.clone()).collect(),body:b.clone()});},Stmt::StructDecl(_,_)=>{},Stmt::EnumDecl(_,_)=>{}}}Ok(None)}
}
fn num(v:Value)->Result<f64,String>{match v{Value::Num(n)=>Ok(n),_=>Err("number expected".into())}}
fn num2(a:Value,b:Value,f:fn(f64,f64)->f64)->Result<Value,String>{Ok(Value::Num(f(num(a)?,num(b)?)))}
fn cmp2(a:Value,b:Value,f:fn(f64,f64)->bool)->Result<Value,String>{Ok(Value::Bool(f(num(a)?,num(b)?)))}
fn div2(a:Value,b:Value)->Result<Value,String>{let x=num(a)?;let y=num(b)?;if y==0.0{return Err("division by zero".into())}Ok(Value::Num(x/y))}
fn mod2(a:Value,b:Value)->Result<Value,String>{let x=num(a)?;let y=num(b)?;if y==0.0{return Err("modulo by zero".into())}Ok(Value::Num(x%y))}
