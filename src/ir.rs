#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    ConstNumber(f64),
    ConstString(String),
    ConstBool(bool),
    Load(String),
    Store(String),
    Binary(String),
    Call(String, usize),
    Jump(usize),
    JumpIfFalse(usize),
    Return,
    Pop,
}

#[derive(Clone, Debug, Default)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub code: Vec<Instr>,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub code: Vec<Instr>,
    pub functions: Vec<Function>,
}

impl Module {
    pub fn push(&mut self, instr: Instr) { self.code.push(instr); }
}

pub struct IrBuilder {
    pub module: Module,
}

impl IrBuilder {
    pub fn new() -> Self { Self { module: Module::default() } }

    pub fn lower_expr(&mut self, e: &crate::Expr) {
        match e {
            crate::Expr::Val(crate::Value::Num(n)) => self.module.push(Instr::ConstNumber(*n)),
            crate::Expr::Val(crate::Value::Str(s)) => self.module.push(Instr::ConstString(s.clone())),
            crate::Expr::Val(crate::Value::Bool(b)) => self.module.push(Instr::ConstBool(*b)),
            crate::Expr::Val(crate::Value::Null) => self.module.push(Instr::ConstString("null".into())),
            crate::Expr::Var(n) => self.module.push(Instr::Load(n.clone())),
            crate::Expr::Array(xs) => {
                for x in xs { self.lower_expr(x); }
                self.module.push(Instr::Call("array".into(), xs.len()));
            }
            crate::Expr::Unary(op, x) => {
                self.lower_expr(x);
                self.module.push(Instr::Binary(format!("{:?}", op)));
            }
            crate::Expr::Binary(a, op, b) => {
                self.lower_expr(a);
                self.lower_expr(b);
                self.module.push(Instr::Binary(format!("{:?}", op)));
            }
            crate::Expr::Call(n, args) => {
                for a in args { self.lower_expr(a); }
                self.module.push(Instr::Call(n.clone(), args.len()));
            }
        }
    }

    pub fn lower_stmt(&mut self, s: &crate::Stmt) {
        match s {
            crate::Stmt::Expr(e) => { self.lower_expr(e); self.module.push(Instr::Pop); }
            crate::Stmt::Let(n,e) | crate::Stmt::Assign(n,e) => {
                self.lower_expr(e); self.module.push(Instr::Store(n.clone()));
            }
            crate::Stmt::Print(e) => {
                self.lower_expr(e); self.module.push(Instr::Call("print".into(),1)); self.module.push(Instr::Pop);
            }
            crate::Stmt::Return(e) => { self.lower_expr(e); self.module.push(Instr::Return); }
            crate::Stmt::If(c,a,b) => {
                self.lower_expr(c);
                let jf=self.module.code.len(); self.module.push(Instr::JumpIfFalse(usize::MAX));
                for x in a { self.lower_stmt(x); }
                let jend=self.module.code.len(); self.module.push(Instr::Jump(usize::MAX));
                let else_at=self.module.code.len();
                for x in b { self.lower_stmt(x); }
                let end=self.module.code.len();
                if let Instr::JumpIfFalse(ref mut target)=self.module.code[jf] { *target=else_at; }
                if let Instr::Jump(ref mut target)=self.module.code[jend] { *target=end; }
            }
            crate::Stmt::While(c,b) => {
                let head=self.module.code.len();
                self.lower_expr(c);
                let exit=self.module.code.len(); self.module.push(Instr::JumpIfFalse(usize::MAX));
                for x in b { self.lower_stmt(x); }
                self.module.push(Instr::Jump(head));
                let end=self.module.code.len();
                if let Instr::JumpIfFalse(ref mut target)=self.module.code[exit] { *target=end; }
            }
            crate::Stmt::For(_,it,b) => {
                self.lower_expr(it);
                self.module.push(Instr::Call("for_each".into(),1));
                for x in b { self.lower_stmt(x); }
            }
            crate::Stmt::Import(_) => {}
            crate::Stmt::Match(value,arms,otherwise) => {
                self.lower_expr(value);
                for (pat,body) in arms {
                    self.lower_expr(pat);
                    self.module.push(Instr::Binary("EqEq".into()));
                    let jf=self.module.code.len(); self.module.push(Instr::JumpIfFalse(usize::MAX));
                    for x in body { self.lower_stmt(x); }
                    let end=self.module.code.len();
                    if let Instr::JumpIfFalse(ref mut target)=self.module.code[jf] { *target=end; }
                }
                for x in otherwise { self.lower_stmt(x); }
            }
            crate::Stmt::Fn(n,args,body) => {
                let mut f=Function{name:n.clone(),params:args.clone(),code:Vec::new()};
                let mut b=IrBuilder{module:Module::default()};
                for x in body { b.lower_stmt(x); }
                f.code=b.module.code;
                self.module.functions.push(f);
            }
        }
    }

    pub fn lower_program(mut self, program: &[crate::Stmt]) -> Module {
        for s in program { self.lower_stmt(s); }
        self.module
    }
}

pub fn format_module(m: &Module) -> String {
    let mut out=String::new();
    for (i,ins) in m.code.iter().enumerate() { out.push_str(&format!("{:04} {:?}\n",i,ins)); }
    for f in &m.functions {
        out.push_str(&format!("fn {}({})\n",f.name,f.params.join(", ")));
        for (i,ins) in f.code.iter().enumerate() { out.push_str(&format!("  {:04} {:?}\n",i,ins)); }
    }
    out
}
