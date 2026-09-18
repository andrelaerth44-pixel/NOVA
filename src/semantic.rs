use crate::{Expr, Stmt, Value, Token};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct StaticFn { args: Vec<crate::types::Type>, ret: crate::types::Type }

pub struct Checker { scopes: Vec<HashMap<String, crate::types::Type>>, fns: HashMap<String, StaticFn>, errors: Vec<String> }
impl Checker {
    pub fn new() -> Self { Self { scopes: vec![HashMap::new()], fns: HashMap::new(), errors: Vec::new() } }
    fn error(&mut self, msg: impl Into<String>) { self.errors.push(msg.into()); }
    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { if self.scopes.len()>1 { self.scopes.pop(); } }
    fn lookup(&self,n:&str)->Option<crate::types::Type>{self.scopes.iter().rev().find_map(|s|s.get(n).cloned())}
    fn define(&mut self,n:String,t:crate::types::Type){if let Some(s)=self.scopes.last_mut(){s.insert(n,t);}}
    fn infer(&mut self,e:&Expr)->crate::types::Type {
        match e {
            Expr::Val(Value::Num(_))=>crate::types::Type::Number, Expr::Val(Value::Str(_))=>crate::types::Type::String, Expr::Val(Value::Bool(_))=>crate::types::Type::Bool, Expr::Val(Value::Null)=>crate::types::Type::Null,
            Expr::Val(Value::Array(xs))|Expr::Array(xs)=>{if xs.is_empty(){return crate::types::Type::Array(Box::new(crate::types::Type::Any));}let first=match &xs[0]{Value::Num(_)=>crate::types::Type::Number,Value::Str(_)=>crate::types::Type::String,Value::Bool(_)=>crate::types::Type::Bool,Value::Null=>crate::types::Type::Null,Value::Array(_)=>crate::types::Type::Any};for x in xs.iter().skip(1){let t=match x{Value::Num(_)=>crate::types::Type::Number,Value::Str(_)=>crate::types::Type::String,Value::Bool(_)=>crate::types::Type::Bool,Value::Null=>crate::types::Type::Null,Value::Array(_)=>crate::types::Type::Any};if !first.compatible(&t){self.error(format!("array elements have incompatible types: {} and {}",first.name(),t.name()));}}crate::types::Type::Array(Box::new(first))},
            Expr::Var(n)=>self.lookup(n).unwrap_or_else(||{self.error(format!("undefined variable {}",n));crate::types::Type::Unknown}),
            Expr::Unary(op,x)=>{let t=self.infer(x);match op{Token::Minus if !t.compatible(&crate::types::Type::Number)=>{self.error(format!("unary - expects number, got {}",t.name()));crate::types::Type::Unknown},Token::Minus=>crate::types::Type::Number,Token::Bang=>crate::types::Type::Bool,_=>crate::types::Type::Unknown}},
            Expr::Binary(a,op,b)=>{let x=self.infer(a);let y=self.infer(b);match op{Token::Plus=>if x.compatible(&crate::types::Type::Number)&&y.compatible(&crate::types::Type::Number){crate::types::Type::Number}else if x.compatible(&crate::types::Type::String)&&y.compatible(&crate::types::Type::String){crate::types::Type::String}else{self.error(format!("operator + cannot combine {} and {}",x.name(),y.name()));crate::types::Type::Unknown},Token::Minus|Token::Star|Token::Slash|Token::Percent=>{if !x.compatible(&crate::types::Type::Number)||!y.compatible(&crate::types::Type::Number){self.error("arithmetic operator expects numbers");crate::types::Type::Unknown}else{crate::types::Type::Number}},Token::Lt|Token::Le|Token::Gt|Token::Ge=>{if !x.compatible(&crate::types::Type::Number)||!y.compatible(&crate::types::Type::Number){self.error("comparison expects numbers");}crate::types::Type::Bool},Token::EqEq|Token::Ne|Token::And|Token::Or=>crate::types::Type::Bool,_=>crate::types::Type::Unknown}},
            Expr::Call(n,args)=>{if let Some(f)=self.fns.get(n).cloned(){if f.args.len()!=args.len(){self.error(format!("{} expects {} arguments, got {}",n,f.args.len(),args.len()));}for(i,a)in args.iter().enumerate(){let t=self.infer(a);if let Some(expected)=f.args.get(i){if !expected.compatible(&t){self.error(format!("argument {} of {} expects {}, got {}",i+1,n,expected.name(),t.name()));}}}return f.ret}for a in args{self.infer(a);}crate::types::Type::Any}
        }
    }
    pub fn check(&mut self,program:&[Stmt])->Result<(),Vec<String>>{for s in program{if let Stmt::Fn(n,args,ret,_)=s{if self.fns.contains_key(n){self.error(format!("duplicate function {}",n));}else{self.fns.insert(n.clone(),StaticFn{args:args.iter().map(|x|x.1.clone()).collect(),ret:ret.clone()});}}}for s in program{if let Stmt::Let(n,explicit,e)=s{let t=self.infer(e);if let Some(ex)=explicit{if !ex.compatible(&t){self.error(format!("type annotation for {} expects {}, got {}",n,ex.name(),t.name()));}self.define(n.clone(),ex.clone());}else{self.define(n.clone(),t);}}else if let Stmt::Assign(n,e)=s{let t=self.infer(e);if let Some(old)=self.lookup(n){if !old.compatible(&t){self.error(format!("cannot assign {} to {} (expected {})",t.name(),n,old.name()));}}else{self.error(format!("assignment to undefined variable {}",n));}}else if let Stmt::Fn(_,args,ret,body)=s{self.push_scope();for(a,t)in args{self.define(a.clone(),t.clone());}for b in body{if let Stmt::Return(e)=b{let t=self.infer(e);if !ret.compatible(&t){self.error(format!("return type mismatch: expected {}, got {}",ret.name(),t.name()));}}else if let Stmt::Expr(e)|Stmt::Print(e)=b{self.infer(e);}}self.pop_scope();}}if self.errors.is_empty(){Ok(())}else{Err(self.errors.clone())}}
}
