#[derive(Clone, Debug, PartialEq)]
pub enum IrType {
    I32,
    I64,
    F32,
    F64,
    Number,
    Bool,
    String,
    Null,
    Any,
    Array(Box<IrType>),
    Struct(String),
    Enum(String),
    Generic(String, Vec<IrType>),
    TypeParam(String),
    Function(Vec<IrType>, Box<IrType>),
}

impl IrType {
    pub fn from_type(ty: &crate::types::Type) -> Self {
        match ty {
            crate::types::Type::Any | crate::types::Type::Unknown => Self::Any,
            crate::types::Type::I32 => Self::I32,
            crate::types::Type::I64 => Self::I64,
            crate::types::Type::F32 => Self::F32,
            crate::types::Type::F64 => Self::F64,
            crate::types::Type::Number => Self::Number,
            crate::types::Type::Bool => Self::Bool,
            crate::types::Type::String => Self::String,
            crate::types::Type::Array(inner) => Self::Array(Box::new(Self::from_type(inner))),
            crate::types::Type::Struct(name) => Self::Struct(name.clone()),
            crate::types::Type::Enum(name) => Self::Enum(name.clone()),
            crate::types::Type::Generic(name, args) => Self::Generic(
                name.clone(),
                args.iter().map(Self::from_type).collect(),
            ),
            crate::types::Type::TypeParam(name) => Self::TypeParam(name.clone()),
            crate::types::Type::Null | crate::types::Type::Void => Self::Null,
            crate::types::Type::Function(args, ret) => Self::Function(
                args.iter().map(Self::from_type).collect(),
                Box::new(Self::from_type(ret)),
            ),
        }
    }
}

impl Default for IrType {
    fn default() -> Self { IrType::Any }
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
    StructInit { name: String, fields: Vec<String> },
    FieldGet { field: String, ty: IrType },
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

/// SSA-ready control-flow representation. The legacy stack IR above remains
/// available while the compiler migrates lowering/backend code to SSA.
pub type ValueId = u32;

#[derive(Clone, Debug, PartialEq)]
pub enum SsaValue {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Struct { name: String },
    Param(ValueId),
    Instr(ValueId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SsaInstr {
    Const(SsaValue),
    Load { name: String },
    Store { name: String, value: ValueId },
    Unary { op: String, value: ValueId, ty: IrType },
    Binary { op: String, left: ValueId, right: ValueId, ty: IrType },
    Call { name: String, args: Vec<ValueId>, result: IrType },
    CallIndirect { callee: ValueId, args: Vec<ValueId>, result: IrType },
    StructInit { name: String, fields: Vec<(String, ValueId)> },
    FieldGet { base: ValueId, field: String, ty: IrType },
    EnumInit { name: String, variant: String, payload: Option<ValueId>, ty: IrType },
    EnumTest { value: ValueId, variant: String },
    EnumPayload { value: ValueId, ty: IrType },
    Try { value: ValueId, result: IrType },
    Closure { params: Vec<String>, captures: Vec<(String, ValueId)>, ty: IrType },
    Phi { incomings: Vec<(usize, ValueId)>, ty: IrType },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
    Jump(usize),
    Branch { condition: ValueId, then_block: usize, else_block: usize },
    Return(Option<ValueId>),
}

#[derive(Clone, Debug, Default)]
pub struct SsaBlock {
    pub id: usize,
    pub params: Vec<ValueId>,
    pub instrs: Vec<(ValueId, SsaInstr)>,
    pub terminator: Option<Terminator>,
}

#[derive(Clone, Debug, Default)]
pub struct SsaFunction {
    pub name: String,
    pub params: Vec<(String, IrType, ValueId)>,
    pub return_type: IrType,
    pub blocks: Vec<SsaBlock>,
}

impl SsaFunction {
    pub fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() {
            return Err(format!("SSA function {} has no blocks", self.name));
        }
        let n = self.blocks.len();
        let mut defined = std::collections::HashSet::<ValueId>::new();

        for block in &self.blocks {
            if block.id >= n {
                return Err(format!("SSA function {} has invalid block {}", self.name, block.id));
            }
            for &v in &block.params {
                if !defined.insert(v) {
                    return Err(format!("SSA value {} is defined more than once", v));
                }
            }
            for &(v, ref instr) in &block.instrs {
                if !defined.insert(v) {
                    return Err(format!("SSA value {} is defined more than once", v));
                }
                if let SsaInstr::Phi { incomings, .. } = instr {
                    for (pred, _) in incomings {
                        if *pred >= n {
                            return Err(format!("SSA phi in block {} references invalid predecessor {}", block.id, pred));
                        }
                    }
                }
            }
            match &block.terminator {
                Some(Terminator::Jump(t)) if *t >= n =>
                    return Err(format!("SSA block {} jumps to invalid block {}", block.id, t)),
                Some(Terminator::Branch { then_block, else_block, .. }) => {
                    if *then_block >= n || *else_block >= n {
                        return Err(format!("SSA branch in block {} has invalid target", block.id));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
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
            crate::Value::Map(_) | crate::Value::Set(_) => IrType::Any,
            crate::Value::Struct { name, .. } => IrType::Struct(name.clone()),
            crate::Value::Enum { name, .. } => IrType::Enum(name.clone()),
            crate::Value::Null => IrType::Null,
            crate::Value::Closure { .. } => IrType::Any,
            crate::Value::Iterator(_) => IrType::Any,
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
                    crate::Value::Map(_) => self.push(Instr::Call { name: "map".into(), argc: 0, result: IrType::Any }),
                    crate::Value::Set(_) => self.push(Instr::Call { name: "set".into(), argc: 0, result: IrType::Any }),
                    crate::Value::Struct { name, fields } => self.push(Instr::StructInit { name: name.clone(), fields: fields.keys().cloned().collect() }),
                    crate::Value::Enum { name, variant, .. } => self.push(Instr::Call { name: format!("{}.{}", name, variant), argc: 0, result: IrType::Enum(name.clone()) }),
                    crate::Value::Closure { .. } => self.push(Instr::Call { name: "closure".into(), argc: 0, result: IrType::Any }),
                    crate::Value::Iterator(_) => self.push(Instr::Call { name: "iterator".into(), argc: 0, result: IrType::Any }),
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
            crate::Expr::Map(entries) => {
                for (key, value) in entries {
                    self.lower_expr(key);
                    self.lower_expr(value);
                }
                self.push(Instr::Call { name: "map".into(), argc: entries.len() * 2, result: IrType::Any });
                IrType::Any
            }
            crate::Expr::Set(values) => {
                for value in values { self.lower_expr(value); }
                self.push(Instr::Call { name: "set".into(), argc: values.len(), result: IrType::Any });
                IrType::Any
            }
            crate::Expr::Index(base, index) => {
                self.lower_expr(base);
                self.lower_expr(index);
                self.push(Instr::Call { name: "index".into(), argc: 2, result: IrType::Any });
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
                let ty = if matches!(op, crate::Token::EqEq | crate::Token::Ne | crate::Token::Lt | crate::Token::Le | crate::Token::Gt | crate::Token::Ge | crate::Token::And | crate::Token::Or) { IrType::Bool } else { IrType::Number };
                self.push(Instr::Binary { op: name, ty: ty.clone() });
                ty
            }
            crate::Expr::EnumInit(name, variant, value) => { if let Some(v)=value { self.lower_expr(v); } self.push(Instr::Call { name: format!("{}.{}",name,variant), argc: value.is_some() as usize, result: IrType::Enum(name.clone()) }); IrType::Enum(name.clone()) }
            crate::Expr::Field(base, field) => {
                let ty = self.lower_expr(base);
                self.push(Instr::FieldGet { field: field.clone(), ty: ty.clone() });
                IrType::Any
            }
            crate::Expr::StructInit(name, fields) => {
                for (_, value) in fields { self.lower_expr(value); }
                self.push(Instr::StructInit { name: name.clone(), fields: fields.iter().map(|(f, _)| f.clone()).collect() });
                IrType::Struct(name.clone())
            }
            crate::Expr::Closure(args, _) => { self.push(Instr::Call { name: "closure".into(), argc: args.len(), result: IrType::Any }); IrType::Any }
            crate::Expr::CallValue(callee, args) => { self.lower_expr(callee); for a in args { self.lower_expr(a); } self.push(Instr::Call { name: "call_value".into(), argc: args.len()+1, result: IrType::Any }); IrType::Any }
            crate::Expr::Try(inner) => { self.lower_expr(inner); self.push(Instr::Call { name: "try".into(), argc: 1, result: IrType::Any }); IrType::Any }
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
                let subject_name = "__nova_match_subject".to_string();
                self.lower_expr(value);
                self.push(Instr::Store(subject_name.clone()));
                for (pat,body) in arms {
                    let test = match pat {
                        crate::Pattern::Wildcard => {
                            self.push(Instr::ConstBool(true));
                        }
                        crate::Pattern::Literal(expr) => {
                            self.push(Instr::Load(subject_name.clone()));
                            self.lower_expr(expr);
                            self.push(Instr::Binary { op: "EqEq".into(), ty: IrType::Bool });
                        }
                        crate::Pattern::Enum { variant, .. } => {
                            self.push(Instr::Load(subject_name.clone()));
                            self.push(Instr::ConstString(variant.clone()));
                            self.push(Instr::Call { name: "match_enum".into(), argc: 2, result: IrType::Bool });
                        }
                    };
                    let _ = test;
                    let jf=self.module.blocks[self.current].code.len();
                    self.push(Instr::JumpIfFalse(usize::MAX));
                    for x in body { self.lower_stmt(x); }
                    let end=self.module.blocks[self.current].code.len();
                    if let Instr::JumpIfFalse(ref mut target)=self.module.blocks[self.current].code[jf] { *target=end; }
                }
                for x in otherwise { self.lower_stmt(x); }
            }
            crate::Stmt::StructDecl(_, _) | crate::Stmt::EnumDecl(_, _) => {}
            crate::Stmt::Fn(n,_,args,ret,body) => {
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

impl SsaFunction {
    /// Validate the SSA graph using CFG-aware dominance rules.
    ///
    /// A normal instruction must be dominated by its use. Phi operands are
    /// exceptional: each incoming value must be available at the end of the
    /// corresponding predecessor. This makes loop-carried and branch-merged
    /// values legal without relying on source/block iteration order.
    pub fn verify_operands(&self) -> Result<(), String> {
        self.validate()?;

        let n = self.blocks.len();
        let mut by_id = vec![None; n];
        for block in &self.blocks {
            by_id[block.id] = Some(block);
        }

        let mut preds = vec![Vec::<usize>::new(); n];
        for block in &self.blocks {
            match block.terminator.as_ref() {
                Some(Terminator::Jump(to)) => preds[*to].push(block.id),
                Some(Terminator::Branch { then_block, else_block, .. }) => {
                    preds[*then_block].push(block.id);
                    preds[*else_block].push(block.id);
                }
                Some(Terminator::Return(_)) | None => {}
            }
        }

        // Every non-entry block must be reachable. Keeping unreachable blocks
        // out of the SSA graph prevents silently accepting dead malformed IR.
        let mut reachable = vec![false; n];
        let mut work = vec![0usize];
        reachable[0] = true;
        while let Some(b) = work.pop() {
            match by_id[b].unwrap().terminator.as_ref().unwrap() {
                Terminator::Jump(t) => {
                    if !reachable[*t] { reachable[*t] = true; work.push(*t); }
                }
                Terminator::Branch { then_block, else_block, .. } => {
                    if !reachable[*then_block] { reachable[*then_block] = true; work.push(*then_block); }
                    if !reachable[*else_block] { reachable[*else_block] = true; work.push(*else_block); }
                }
                Terminator::Return(_) => {}
            }
        }
        for block in &self.blocks {
            if !reachable[block.id] {
                return Err(format!("SSA block {} is unreachable", block.id));
            }
        }

        // Compute dominator sets to validate ordinary SSA uses.
        let all: std::collections::HashSet<usize> = (0..n).collect();
        let mut dom = vec![all.clone(); n];
        dom[0].clear();
        dom[0].insert(0);
        let mut changed = true;
        while changed {
            changed = false;
            for b in 1..n {
                if preds[b].is_empty() {
                    continue;
                }
                let mut next = all.clone();
                for p in &preds[b] {
                    next.retain(|x| dom[*p].contains(x));
                }
                next.insert(b);
                if next != dom[b] {
                    dom[b] = next;
                    changed = true;
                }
            }
        }

        let mut defs = std::collections::HashMap::<ValueId, (usize, usize)>::new();
        for block in &self.blocks {
            for &id in &block.params {
                if defs.insert(id, (block.id, usize::MAX)).is_some() {
                    return Err(format!("SSA value {} is defined more than once", id));
                }
            }
            for (idx, (id, _)) in block.instrs.iter().enumerate() {
                if defs.insert(*id, (block.id, idx)).is_some() {
                    return Err(format!("SSA value {} is defined more than once", id));
                }
            }
        }
        for (_, _, id) in &self.params {
            if defs.insert(*id, (0, usize::MAX)).is_some() {
                return Err(format!("SSA value {} is defined more than once", id));
            }
        }

        let check_use = |value: ValueId, use_block: usize, use_index: usize,
                         defs: &std::collections::HashMap<ValueId, (usize, usize)>,
                         dom: &Vec<std::collections::HashSet<usize>>|
                         -> Result<(), String> {
            let (def_block, def_index) = defs.get(&value).copied()
                .ok_or_else(|| format!("SSA value {} is undefined", value))?;
            if !dom[use_block].contains(&def_block) {
                return Err(format!("SSA value {} does not dominate use in block {}", value, use_block));
            }
            if def_block == use_block && def_index != usize::MAX && def_index >= use_index {
                return Err(format!("SSA value {} is used before its definition in block {}", value, use_block));
            }
            Ok(())
        };

        for block in &self.blocks {
            for (idx, (_, instr)) in block.instrs.iter().enumerate() {
                match instr {
                    SsaInstr::Const(_) | SsaInstr::Load { .. } => {}
                    SsaInstr::Store { value, .. } |
                    SsaInstr::Unary { value, .. } => check_use(*value, block.id, idx, &defs, &dom)?,
                    SsaInstr::Binary { left, right, .. } => {
                        check_use(*left, block.id, idx, &defs, &dom)?;
                        check_use(*right, block.id, idx, &defs, &dom)?;
                    }
                    SsaInstr::Call { args, .. } => {
                        for value in args { check_use(*value, block.id, idx, &defs, &dom)?; }
                    }
                    SsaInstr::CallIndirect { callee, args, .. } => {
                        check_use(*callee, block.id, idx, &defs, &dom)?;
                        for value in args { check_use(*value, block.id, idx, &defs, &dom)?; }
                    }
                    SsaInstr::StructInit { fields, .. } => {
                        for (_, value) in fields { check_use(*value, block.id, idx, &defs, &dom)?; }
                    }
                    SsaInstr::FieldGet { base, .. } => {
                        check_use(*base, block.id, idx, &defs, &dom)?;
                    }
                    SsaInstr::EnumInit { payload: Some(value), .. } => {
                        check_use(*value, block.id, idx, &defs, &dom)?;
                    }
                    SsaInstr::EnumInit { payload: None, .. } => {}
                    SsaInstr::EnumTest { value, .. } |
                    SsaInstr::EnumPayload { value, .. } |
                    SsaInstr::Try { value, .. } => {
                        check_use(*value, block.id, idx, &defs, &dom)?;
                    }
                    SsaInstr::Closure { captures, .. } => {
                        for (_, value) in captures {
                            check_use(*value, block.id, idx, &defs, &dom)?;
                        }
                    }
                    SsaInstr::Phi { incomings, .. } => {
                        let expected: std::collections::HashSet<_> = preds[block.id].iter().copied().collect();
                        let actual: std::collections::HashSet<_> = incomings.iter().map(|(p, _)| *p).collect();
                        if actual != expected {
                            return Err(format!("SSA phi in block {} has predecessors {:?}, expected {:?}", block.id, actual, expected));
                        }
                        for (pred, value) in incomings {
                            let pred_block = by_id[*pred].unwrap();
                            let use_index = pred_block.instrs.len();
                            check_use(*value, *pred, use_index, &defs, &dom)?;
                        }
                    }
                }
            }

            match block.terminator.as_ref() {
                Some(Terminator::Branch { condition, .. }) => {
                    check_use(*condition, block.id, block.instrs.len(), &defs, &dom)?;
                }
                Some(Terminator::Return(Some(value))) => {
                    check_use(*value, block.id, block.instrs.len(), &defs, &dom)?;
                }
                Some(Terminator::Jump(_)) | Some(Terminator::Return(None)) => {}
                None => return Err(format!("SSA block {} has no terminator", block.id)),
            }
        }

        Ok(())
    }
}
