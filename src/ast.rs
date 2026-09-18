#[derive(Clone, Debug)]
pub enum Expr {
    Val(Value), Var(String), Unary(Token, Box<Expr>),
    Binary(Box<Expr>, Token, Box<Expr>), Call(String, Vec<Expr>),
    Array(Vec<Expr>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Expr(Expr), Let(String, Expr), Assign(String, Expr), Print(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>), While(Expr, Vec<Stmt>), For(String, Expr, Vec<Stmt>), Import(String), Match(Expr, Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    Fn(String, Vec<String>, Vec<Stmt>), Return(Expr),
}
#[derive(Clone, Debug)]
pub enum Value { Num(f64), Str(String), Bool(bool), Array(Vec<Value>), Null }
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

