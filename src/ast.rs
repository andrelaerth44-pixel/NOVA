use crate::Token;
#[derive(Clone, Debug)]
pub enum Expr {
    Val(Value), Var(String), Unary(Token, Box<Expr>),
    Binary(Box<Expr>, Token, Box<Expr>), Try(Box<Expr>), Call(String, Vec<Expr>), CallValue(Box<Expr>, Vec<Expr>),
    Array(Vec<Expr>), Field(Box<Expr>, String), Closure(Vec<String>, Vec<Stmt>),
    StructInit(String, Vec<(String, Expr)>),
    EnumInit(String, String, Option<Box<Expr>>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Expr(Expr), Let(String, Option<crate::types::Type>, Expr), Assign(String, Expr), Print(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>), While(Expr, Vec<Stmt>), For(String, Expr, Vec<Stmt>), Import(String), Match(Expr, Vec<(Pattern, Vec<Stmt>)>, Vec<Stmt>),
    Fn(String, Vec<String>, Vec<(String, crate::types::Type)>, crate::types::Type, Vec<Stmt>), Return(Expr),
    StructDecl(String, Vec<(String, crate::types::Type)>),
    EnumDecl(String, Vec<(String, Option<crate::types::Type>)>),
}
#[derive(Clone, Debug)]
pub enum Pattern {
    Wildcard,
    Literal(Expr),
    Enum { variant: String, binding: Option<String> },
}

#[derive(Clone, Debug)]
pub enum Value {
    Num(f64), Str(String), Bool(bool), Array(Vec<Value>),
    Struct { name: String, fields: std::collections::HashMap<String, Value> },
    Enum { name: String, variant: String, value: Option<Box<Value>> },
    Closure { args: Vec<String>, body: Vec<Stmt>, env: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, Value>>> },
    Null,
}
impl Value {
    pub fn equals(&self, other:&Value)->bool {
        match (self, other) {
            (Value::Num(a),Value::Num(b))=>a==b,
            (Value::Str(a),Value::Str(b))=>a==b,
            (Value::Bool(a),Value::Bool(b))=>a==b,
            (Value::Null,Value::Null)=>true,
            (Value::Array(a),Value::Array(b))=>a.len()==b.len()&&a.iter().zip(b).all(|(x,y)|x.equals(y)),
            (Value::Struct{name:an,fields:af},Value::Struct{name:bn,fields:bf})=>an==bn&&af.len()==bf.len()&&af.iter().all(|(k,v)|bf.get(k).is_some_and(|x|v.equals(x))),
            (Value::Enum{name:an,variant:av,value:ax},Value::Enum{name:bn,variant:bv,value:bx})=>an==bn&&av==bv&&match (ax,bx){(None,None)=>true,(Some(a),Some(b))=>a.equals(b),_=>false},
            (Value::Closure{..},Value::Closure{..})=>false,
            _=>false
        }
    }
    pub fn truth(&self)->bool {
        match self {
            Value::Bool(x)=>*x, Value::Num(x)=>*x!=0.0, Value::Str(x)=>!x.is_empty(),
            Value::Array(x)=>!x.is_empty(), Value::Struct{..}=>true, Value::Enum{..}=>true, Value::Closure{..}=>true, Value::Null=>false
        }
    }
}
impl std::fmt::Display for Value {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        match self {
            Value::Num(x)=>write!(f,"{}", if x.fract()==0.0 {format!("{}",*x as i64)} else {x.to_string()}),
            Value::Str(x)=>write!(f,"{}",x), Value::Bool(x)=>write!(f,"{}",x),
            Value::Array(x)=>{write!(f,"[")?;for(i,v) in x.iter().enumerate(){if i>0{write!(f,", ")?;}write!(f,"{}",v)?;}write!(f,"]")},
            Value::Struct{name,fields}=>{write!(f,"{} {{ ",name)?;let mut first=true;for(k,v)in fields{if !first{write!(f,", ")?;}first=false;write!(f,"{}: {}",k,v)?;}write!(f," }}")},
            Value::Enum{name,variant,value}=>match value{Some(v)=>write!(f,"{}.{}({})",name,variant,v),None=>write!(f,"{}.{}",name,variant)},
            Value::Closure{..}=>write!(f,"<closure>"),
            Value::Null=>write!(f,"null")
        }
    }
}
