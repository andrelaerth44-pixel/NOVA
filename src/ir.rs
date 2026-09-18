#[derive(Clone, Debug, PartialEq)]
pub enum IrType {
    Number,
    Bool,
    String,
    Null,
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    ConstNumber(f64),
    ConstString(String),
    ConstBool(bool),
    ConstNull,
    Load(String),
    Store(String),
    Unary { op: String, ty: IrType },
    Binary { op: String, ty: IrType },
    Call { name: String, argc: usize, result: IrType },
    Jump(usize),
    JumpIfFalse(usize),
    Return(IrType),
    Pop,
}

#[derive(Clone, Debug, Default)]
pub struct BasicBlock {
    pub id: usize,
    pub code: Vec<Instr>,
}

#[derive(Clone, Debug, Default)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, IrType)>,
    pub return_type: IrType,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub blocks: Vec<BasicBlock>,
    pub functions: Vec<Function>,
}

impl Module {
    pub fn new_block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock { id, code: Vec::new() });
        id
    }
}

pub struct IrBuilder {
    pub module: Module,
    current: usize,
}

impl IrBuilder {
    pub fn new() -> Self {
        let mut module = Module::default();
        module.new_block();
        Self { module, current: 0 }
    }

    fn push(&mut self, instr: Instr) {
        self.module.blocks[self.current].code.push(instr);
    }

    fn value_type(v: &crate::Value) -> IrType {
        match v {
            crate::Value::Num(_) => IrType::Number,
            crate::Value::Str(_) => IrType::String,
            crate::Value::Bool(_) => IrType::Bool,
            crate::Value::Array(_) => IrType::Any,
            crate::Value::Null => IrType::Null,
        }
    }

    pub fn lower_expr(&mut self, e: &crate::Expr) -> IrType {
        match e {
            crate::Expr::Val(v) => {
                let ty = Self::value_type(v);
                match v {
                    crate::Value::Num(n) => self.push(Instr::ConstNumber(*n)),
                    crate::Value::Str(s) => self.push(Instr::ConstString(s.clone())),
                    crate::Value::Bool(b) => self.push(Instr::ConstBool(*b)),
                    crate::Value::Null => self.push(Instr::ConstNull),
                    crate::Value::Array(_) => self.push(Instr::Call { name: "array".into(), argc: 0, result: IrType::Any }),
                }
                ty
            }
            crate::Expr::Var(n) => {
                self.push(Instr::Load(n.clone()));
                IrType::Any
            }
            crate::Expr::Array(xs) => {
                for x in xs { self.lower_expr(x); }
                self.push(Instr::Call { name: "array".into(), argc: xs.len(), result: IrType::Any });
                IrType::Any
            }
            crate::Expr::Unary(op, x) => {
                let ty = self.lower_expr(x);
                self.push(Instr::Unary { op: format!("{:?}", op), ty: ty.clone() });
                ty
            }
            crate::Expr::Binary(a, op, b) => {
                let _ = self.lower_expr(a);
                let _ = self.lower_expr(b);
                let name = format!("{:?}", op);
                let ty = if matches!(op, crate::Token::EqEq | crate::Token::NotEq | crate::Token::Lt | crate::Token::Le | crate::Token::Gt | crate::Token::Ge | crate::Token::And | crate::Token::Or) { IrType::Bool } else { IrType::Number };
                self.push(Instr::Binary { op: name, ty: ty.clone() });
                ty
            }
            crate::Expr::Call(n, args) => {
                for a in args { self.lower_expr(a); }
                self.push(Instr::Call { name: n.clone(), argc: args.len(), result: IrType::Any });
                IrType::Any
            }
        }
    }

    pub fn lower_stmt(&mut self, s: &crate::Stmt) {
        match s {
            crate::Stmt::Expr(e) => { self.lower_expr(e); self.push(Instr::Pop); }
            crate::Stmt::Let(n,_,e) | crate::Stmt::Assign(n,e) => {
                self.lower_expr(e); self.push(Instr::Store(n.clone()));
            }
            crate::Stmt::Print(e) => {
                self.lower_expr(e);
                self.push(Instr::Call { name: "print".into(), argc: 1, result: IrType::Null });
            }
            crate::Stmt::Return(e) => {
                let ty = self.lower_expr(e);
                self.push(Instr::Return(ty));
            }
            crate::Stmt::If(c,a,b) => {
                self.lower_expr(c);
                let jf=self.module.blocks[self.current].code.len();
                self.push(Instr::JumpIfFalse(usize::MAX));
                for x in a { self.lower_stmt(x); }
                let jend=self.module.blocks[self.current].code.len();
                self.push(Instr::Jump(usize::MAX));
                let else_at=self.module.blocks[self.current].code.len();
                for x in b { self.lower_stmt(x); }
                let end=self.module.blocks[self.current].code.len();
                if let Instr::JumpIfFalse(ref mut target)=self.module.blocks[self.current].code[jf] { *target=else_at; }
                if let Instr::Jump(ref mut target)=self.module.blocks[self.current].code[jend] { *target=end; }
            }
            crate::Stmt::While(c,b) => {
                let head=self.module.blocks[self.current].code.len();
                self.lower_expr(c);
                let exit=self.module.blocks[self.current].code.len();
                self.push(Instr::JumpIfFalse(usize::MAX));
                for x in b { self.lower_stmt(x); }
                self.push(Instr::Jump(head));
                let end=self.module.blocks[self.current].code.len();
                if let Instr::JumpIfFalse(ref mut target)=self.module.blocks[self.current].code[exit] { *target=end; }
            }
            crate::Stmt::For(_,it,b) => {
                self.lower_expr(it);
                self.push(Instr::Call { name: "for_each".into(), argc: 1, result: IrType::Null });
                for x in b { self.lower_stmt(x); }
            }
            crate::Stmt::Import(_) => {}
            crate::Stmt::Match(value,arms,otherwise) => {
                self.lower_expr(value);
                for (pat,body) in arms {
                    self.lower_expr(pat);
                    self.push(Instr::Binary { op: "EqEq".into(), ty: IrType::Bool });
                    let jf=self.module.blocks[self.current].code.len();
                    self.push(Instr::JumpIfFalse(usize::MAX));
                    for x in body { self.lower_stmt(x); }
                    let end=self.module.blocks[self.current].code.len();
                    if let Instr::JumpIfFalse(ref mut target)=self.module.blocks[self.current].code[jf] { *target=end; }
                }
                for x in otherwise { self.lower_stmt(x); }
            }
            crate::Stmt::Fn(n,args,ret,body) => {
                let params=args.iter().map(|(name,t)| (name.clone(), match t {
                    crate::types::Type::Bool=>IrType::Bool,
                    crate::types::Type::String=>IrType::String,
                    crate::types::Type::Null=>IrType::Null,
                    _=>IrType::Any,
                })).collect();
                let return_type=match ret {
                    crate::types::Type::Bool=>IrType::Bool,
                    crate::types::Type::String=>IrType::String,
                    crate::types::Type::Null=>IrType::Null,
                    _=>IrType::Any,
                };
                let mut b=IrBuilder::new();
                for x in body { b.lower_stmt(x); }
                let blocks=b.module.blocks;
                self.module.functions.push(Function{name:n.clone(),params,return_type,blocks});
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
    for block in &m.blocks {
        out.push_str(&format!("block {}\n", block.id));
        for (i,ins) in block.code.iter().enumerate() { out.push_str(&format!("  {:04} {:?}\n",i,ins)); }
    }
    for f in &m.functions {
        out.push_str(&format!("fn {}({}) -> {:?}\n",f.name,f.params.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>().join(", "),f.return_type));
        for block in &f.blocks {
            out.push_str(&format!("  block {}\n", block.id));
            for (i,ins) in block.code.iter().enumerate() { out.push_str(&format!("    {:04} {:?}\n",i,ins)); }
        }
    }
    out
}
