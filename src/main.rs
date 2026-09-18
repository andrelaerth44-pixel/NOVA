mod span;
mod types;
mod ir;
mod diagnostics;
mod lower;
mod optimizer;\nmod backend_c;

use std::{collections::HashMap, env, fs};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Num(f64), Str(String), Ident(String),
    Plus, Minus, Star, Slash, Percent,
    Eq, EqEq, Ne, Lt, Le, Gt, Ge,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semi, Dot, Bang, And, Or,
    If, Else, While, Fn, Return, True, False, Let,
    Print, In, For, Import, Match, Eof,
}

fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '#' => { while i < c.len() && c[i] != '\n' { i += 1; } }
            '0'..='9' => {
                let s = i; while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
                out.push(Token::Num(src[s..i].parse().map_err(|_| "invalid number")?));
            }
            '"' => {
                i += 1; let mut s = String::new();
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        i += 1; s.push(match c[i] { 'n'=>'\n','r'=>'\r','t'=>'\t','"'=>'"','\\'=>'\\',x=>x });
                    } else { s.push(c[i]); }
                    i += 1;
                }
                if i == c.len() { return Err("unterminated string".into()); }
                i += 1; out.push(Token::Str(s));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let s = i; while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i]=='_') { i += 1; }
                let w = &src[s..i];
                out.push(match w {
                    "if"=>Token::If, "else"=>Token::Else, "while"=>Token::While,
                    "fn"=>Token::Fn, "return"=>Token::Return, "true"=>Token::True,
                    "false"=>Token::False, "let"=>Token::Let, "print"=>Token::Print, "in"=>Token::In, "for"=>Token::For, "import"=>Token::Import, "match"=>Token::Match,
                    "and"=>Token::And, "or"=>Token::Or, _=>Token::Ident(w.into())
                });
            }
            '+' => { out.push(Token::Plus); i+=1; }
            '-' => { out.push(Token::Minus); i+=1; }
            '*' => { out.push(Token::Star); i+=1; }
            '/' => { out.push(Token::Slash); i+=1; }
            '%' => { out.push(Token::Percent); i+=1; }
            '(' => { out.push(Token::LParen); i+=1; }
            ')' => { out.push(Token::RParen); i+=1; }
            '{' => { out.push(Token::LBrace); i+=1; }
            '}' => { out.push(Token::RBrace); i+=1; }
            '[' => { out.push(Token::LBracket); i+=1; }
            ']' => { out.push(Token::RBracket); i+=1; }
            ',' => { out.push(Token::Comma); i+=1; }
            ';' => { out.push(Token::Semi); i+=1; }
            '.' => { out.push(Token::Dot); i+=1; }
            '!' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Ne);i+=2} else {out.push(Token::Bang);i+=1} }
            '=' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::EqEq);i+=2} else {out.push(Token::Eq);i+=1} }
            '<' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Le);i+=2} else {out.push(Token::Lt);i+=1} }
            '>' => { if i+1<c.len() && c[i+1]=='=' {out.push(Token::Ge);i+=2} else {out.push(Token::Gt);i+=1} }
            _ => return Err(format!("unexpected character {}", c[i])),
        }
    }
    out.push(Token::Eof); Ok(out)
}

#[derive(Clone, Debug)]
enum Expr {
    Val(Value), Var(String), Unary(Token, Box<Expr>),
    Binary(Box<Expr>, Token, Box<Expr>), Call(String, Vec<Expr>),
    Array(Vec<Expr>),
}
#[derive(Clone, Debug)]
enum Stmt {
    Expr(Expr), Let(String, Expr), Assign(String, Expr), Print(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>), While(Expr, Vec<Stmt>), For(String, Expr, Vec<Stmt>), Import(String), Match(Expr, Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    Fn(String, Vec<String>, Vec<Stmt>), Return(Expr),
}
#[derive(Clone, Debug)]
enum Value { Num(f64), Str(String), Bool(bool), Array(Vec<Value>), Null }
impl Value {
    fn equals(&self, other:&Value)->bool { match (self, other) { (Value::Num(a),Value::Num(b))=>a==b,(Value::Str(a),Value::Str(b))=>a==b,(Value::Bool(a),Value::Bool(b))=>a==b,(Value::Null,Value::Null)=>true,(Value::Array(a),Value::Array(b))=>a.len()==b.len()&&a.iter().zip(b).all(|(x,y)|x.equals(y)), _=>false } }\n    fn truth(&self)->bool { match self { Value::Bool(x)=>*x, Value::Num(x)=>*x!=0.0, Value::Str(x)=>!x.is_empty(), Value::Array(x)=>!x.is_empty(), Value::Null=>false } }
}
impl std::fmt::Display for Value {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        match self { Value::Num(x)=>write!(f,"{}", if x.fract()==0.0 {format!("{}",*x as i64)} else {x.to_string()}),
            Value::Str(x)=>write!(f,"{}",x), Value::Bool(x)=>write!(f,"{}",x),
            Value::Array(x)=>{write!(f,"[")?;for(i,v) in x.iter().enumerate(){if i>0{write!(f,", ")?;}write!(f,"{}",v)?;}write!(f,"]")},
            Value::Null=>write!(f,"null") }
    }
}

struct Parser { t: Vec<Token>, p: usize }
impl Parser {
    fn new(t:Vec<Token>)->Self{Self{t,p:0}}
    fn peek(&self)->&Token{&self.t[self.p]}
    fn take(&mut self)->Token{let x=self.t[self.p].clone();self.p+=1;x}
    fn eat(&mut self,x:&Token)->bool{if self.peek()==x{self.p+=1;true}else{false}}
    fn block(&mut self)->Result<Vec<Stmt>,String>{if !self.eat(&Token::LBrace){return Err("expected {".into())}let mut v=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{v.push(self.stmt()?);self.eat(&Token::Semi);}if !self.eat(&Token::RBrace){return Err("expected }".into())}Ok(v)}
    fn program(&mut self)->Result<Vec<Stmt>,String>{let mut v=vec![];while *self.peek()!=Token::Eof{v.push(self.stmt()?);self.eat(&Token::Semi);}Ok(v)}
    fn stmt(&mut self)->Result<Stmt,String>{
        match self.peek() {
            Token::Let=>{self.take();let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected identifier".into())};if !self.eat(&Token::Eq){return Err("expected =".into())}Ok(Stmt::Let(n,self.expr()?))},
            Token::Print=>{self.take();Ok(Stmt::Print(self.expr()?))},
            Token::Return=>{self.take();Ok(Stmt::Return(self.expr()?))},
            Token::If=>{self.take();let c=self.expr()?;let a=self.block()?;let b=if self.eat(&Token::Else){self.block()?}else{vec![]};Ok(Stmt::If(c,a,b))},
            Token::While=>{self.take();let c=self.expr()?;Ok(Stmt::While(c,self.block()?))},
            Token::For=>{self.take();let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected loop variable".into())};if !self.eat(&Token::In){return Err("expected in".into())}let it=self.expr()?;Ok(Stmt::For(n,it,self.block()?))},
            Token::Import=>{self.take();match self.take(){Token::Str(x)=>Ok(Stmt::Import(x)),_=>Err("import expects a string path".into())}},
            Token::Match=>{self.take();let value=self.expr()?;if !self.eat(&Token::LBrace){return Err("expected { after match".into())}let mut arms=vec![];let mut otherwise=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{if let Token::Ident(n)=self.peek(){if n=="else"{self.take();otherwise=self.block()?;self.eat(&Token::Comma);continue}}let pat=self.expr()?;if !self.eat(&Token::LBrace){return Err("expected { in match arm".into())}let mut body=vec![];while *self.peek()!=Token::RBrace&&*self.peek()!=Token::Eof{body.push(self.stmt()?);self.eat(&Token::Semi);}if !self.eat(&Token::RBrace){return Err("expected } in match arm".into())}arms.push((pat,body));self.eat(&Token::Comma);}if !self.eat(&Token::RBrace){return Err("expected } after match".into())}Ok(Stmt::Match(value,arms,otherwise))},
            Token::Fn=>{self.take();let n=match self.take(){Token::Ident(x)=>x,_=>return Err("expected function name".into())};if !self.eat(&Token::LParen){return Err("expected (".into())}let mut a=vec![];if !self.eat(&Token::RParen){loop{a.push(match self.take(){Token::Ident(x)=>x,_=>return Err("expected parameter".into())});if self.eat(&Token::RParen){break}if !self.eat(&Token::Comma){return Err("expected ,".into())}}}Ok(Stmt::Fn(n,a,self.block()?))},
            Token::Ident(n)=>{
                let name=n.clone(); if self.p+1<self.t.len() && self.t[self.p+1]==Token::Eq {self.take();self.take();return Ok(Stmt::Assign(name,self.expr()?));}
                Ok(Stmt::Expr(self.expr()?))
            },
            _=>Ok(Stmt::Expr(self.expr()?))
        }
    }
    fn expr(&mut self)->Result<Expr,String>{self.or()}
    fn or(&mut self)->Result<Expr,String>{let mut x=self.and()?;while self.eat(&Token::Or){x=Expr::Binary(Box::new(x),Token::Or,Box::new(self.and()?));}Ok(x)}
    fn and(&mut self)->Result<Expr,String>{let mut x=self.eq()?;while self.eat(&Token::And){x=Expr::Binary(Box::new(x),Token::And,Box::new(self.eq()?));}Ok(x)}
    fn eq(&mut self)->Result<Expr,String>{let mut x=self.cmp()?;loop{let op=match self.peek(){Token::EqEq=>Token::EqEq,Token::Ne=>Token::Ne,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.cmp()?));}Ok(x)}
    fn cmp(&mut self)->Result<Expr,String>{let mut x=self.term()?;loop{let op=match self.peek(){Token::Lt=>Token::Lt,Token::Le=>Token::Le,Token::Gt=>Token::Gt,Token::Ge=>Token::Ge,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.term()?));}Ok(x)}
    fn term(&mut self)->Result<Expr,String>{let mut x=self.factor()?;loop{let op=match self.peek(){Token::Plus=>Token::Plus,Token::Minus=>Token::Minus,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.factor()?));}Ok(x)}
    fn factor(&mut self)->Result<Expr,String>{let mut x=self.unary()?;loop{let op=match self.peek(){Token::Star=>Token::Star,Token::Slash=>Token::Slash,Token::Percent=>Token::Percent,_=>break};self.take();x=Expr::Binary(Box::new(x),op,Box::new(self.unary()?));}Ok(x)}
    fn unary(&mut self)->Result<Expr,String>{if self.eat(&Token::Minus){Ok(Expr::Unary(Token::Minus,Box::new(self.unary()?)))}else if self.eat(&Token::Bang){Ok(Expr::Unary(Token::Bang,Box::new(self.unary()?)))}else{self.primary()}}
    fn primary(&mut self)->Result<Expr,String>{
        let x=match self.take(){Token::Num(x)=>Expr::Val(Value::Num(x)),Token::Str(x)=>Expr::Val(Value::Str(x)),Token::True=>Expr::Val(Value::Bool(true)),Token::False=>Expr::Val(Value::Bool(false)),
            Token::Ident(n)=>{if self.eat(&Token::LParen){let mut a=vec![];if !self.eat(&Token::RParen){loop{a.push(self.expr()?);if self.eat(&Token::RParen){break}if !self.eat(&Token::Comma){return Err("expected ,".into())}}}Expr::Call(n,a)}else{Expr::Var(n)}},
            Token::LBracket=>{let mut a=vec![];if !self.eat(&Token::RBracket){loop{a.push(self.expr()?);if self.eat(&Token::RBracket){break}if !self.eat(&Token::Comma){return Err("expected ,".into())}}}Expr::Array(a)},
            Token::LParen=>{let x=self.expr()?;if !self.eat(&Token::RParen){return Err("expected )".into())}x},
            t=>return Err(format!("unexpected token {:?}",t))};Ok(x)
    }
}

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
            Stmt::Expr(e)=>{self.eval(e)?;}, Stmt::Let(n,e)|Stmt::Assign(n,e)=>{let v=self.eval(e)?;self.vars.insert(n.clone(),v);},
            Stmt::Print(e)=>println!("{}",self.eval(e)?), Stmt::Return(e)=>return Ok(Some(self.eval(e)?)),
            Stmt::If(c,a,b)=>{if self.eval(c)?.truth(){if let Some(v)=self.exec(a)?{return Ok(Some(v))}}else if let Some(v)=self.exec(b)?{return Ok(Some(v))}},
            Stmt::While(c,b)=>{while self.eval(c)?.truth(){if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},
            Stmt::For(n,it,b)=>{let v=self.eval(it)?;match v{Value::Array(xs)=>{for x in xs{self.vars.insert(n.clone(),x);if let Some(v)=self.exec(b)?{return Ok(Some(v))}}},_=>return Err("for expects an array or range".into())}},
            Stmt::Match(value,arms,otherwise)=>{let v=self.eval(value)?;let mut done=false;for (pat,body) in arms{if self.eval(pat)?.to_string()==v.to_string(){if let Some(r)=self.exec(body)?{return Ok(Some(r))}done=true;break}}if !done{if let Some(r)=self.exec(otherwise)?{return Ok(Some(r))}}},
            Stmt::Import(path)=>{ let resolved={let p=std::path::Path::new(path);if p.is_absolute(){p.to_path_buf()}else if let Some(base)=self.module_stack.last(){base.join(p)}else{p.to_path_buf()}}; let key=resolved.to_string_lossy().to_string(); if !self.modules.contains_key(&key){let src=fs::read_to_string(&resolved).map_err(|e|format!("cannot import {}: {}",resolved.display(),e))?;let toks=lex(&src)?;let mut p=Parser::new(toks);let program=p.program()?;self.modules.insert(key.clone(),true);let parent=resolved.parent().map(|x|x.to_path_buf()).unwrap_or_else(||std::path::PathBuf::from("."));self.module_stack.push(parent);let r=self.exec(&program);self.module_stack.pop();r?;}},
            Stmt::Fn(n,a,b)=>{self.fns.insert(n.clone(),Function{args:a.clone(),body:b.clone()});}
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
                Stmt::Let(n, e) | Stmt::Assign(n, e) => {
                    let t = self.infer(e);
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
                Stmt::Fn(n, args, body) => {
                    if self.fns.contains_key(n) { self.error(format!("duplicate function {}", n)); continue; }
                    self.fns.insert(n.clone(), StaticFn { args: args.iter().map(|_| types::Type::Any).collect(), ret: types::Type::Any });
                    self.push_scope();
                    for a in args { self.define(a.clone(), types::Type::Any); }
                    self.check_block(body, Some(types::Type::Any));
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
    if a[1]=="ir"{let m=optimizer::optimize(lower::lower(&program));if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}print!("{}",ir::format_module(&m));return}\n    if a[1]=="build-c"||a[1]=="build-native"{let m=optimizer::optimize(lower::lower(&program));if let Err(e)=lower::verify(&m){eprintln!("{}",e);std::process::exit(1)}let out=if a.len()>3{&a[3]}else{"a.out"};let c=match backend_c::emit_c(&m){Ok(x)=>x,Err(e)=>{eprintln!("native backend error: {}",e);std::process::exit(1)}};if a[1]=="build-c"{if let Err(e)=fs::write(out,&c){eprintln!("cannot write {}: {}",out,e);std::process::exit(1)}println!("{}",out);return}let status=std::process::Command::new("cc").args(["-O3","-std=c11","-x","c","-","-o",out]).stdin(std::process::Stdio::piped()).spawn().and_then(|mut child|{use std::io::Write;if let Some(mut stdin)=child.stdin.take(){stdin.write_all(c.as_bytes())?;}child.wait()});match status{Ok(s) if s.success()=>println!("{}",out),Ok(s)=>{eprintln!("C compiler exited with {}",s);std::process::exit(1)},Err(e)=>{eprintln!("cannot invoke cc: {}",e);std::process::exit(1)}}return}
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
    #[test] fn short_circuit(){let vm=run("let x = false and (1 / 0)").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"false");}\n    #[test] fn typed_equality(){assert_eq!(run("let x = 1 == \"1\"").unwrap().vars.get("x").unwrap().to_string(),"false");assert_eq!(run("let x = [1,2] == [1,2]").unwrap().vars.get("x").unwrap().to_string(),"true");}\n    #[test] fn relative_imports(){let dir=std::env::temp_dir().join("nova_import_test");let _=std::fs::create_dir_all(&dir);let child=dir.join("child.nova");let main=dir.join("main.nova");std::fs::write(&child,"let imported = 42").unwrap();std::fs::write(&main,"import \"child.nova\"").unwrap();let src=std::fs::read_to_string(&main).unwrap();let mut vm=Vm::new();vm.modules.insert(main.to_string_lossy().to_string(),true);vm.module_stack.push(dir.clone());let p=Parser::new(lex(&src).unwrap());let mut p=p;let program=p.program().unwrap();vm.exec(&program).unwrap();assert_eq!(vm.vars.get("imported").unwrap().to_string(),"42");let _=std::fs::remove_dir_all(&dir);}\n
    #[test] fn math_builtins(){let vm=run("let x = abs(-4) let y = sqrt(9)").unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"4");assert_eq!(vm.vars.get("y").unwrap().to_string(),"3");}
    #[test] fn filesystem_builtins(){let path="/tmp/nova_test_runtime.txt";let src=format!("write_file(\"{}\", \"hello\") let x = read_file(\"{}\")",path,path);let vm=run(&src).unwrap();assert_eq!(vm.vars.get("x").unwrap().to_string(),"hello");assert_eq!(run(&format!("let x = exists(\"{}\")",path)).unwrap().vars.get("x").unwrap().to_string(),"true");let _=std::fs::remove_file(path);}

}
