use crate::{Expr, Stmt, Value, Token};
use crate::ir::{IrType, SsaBlock, SsaFunction, SsaInstr, SsaValue, Terminator, ValueId};
use std::collections::HashMap;

struct Builder {
    blocks: Vec<SsaBlock>,
    current: usize,
    next: ValueId,
    vars: Vec<HashMap<String, ValueId>>,
}

impl Builder {
    fn new() -> Self {
        Self { blocks: vec![SsaBlock { id: 0, ..Default::default() }], current: 0, next: 0, vars: vec![HashMap::new()] }
    }
    fn fresh(&mut self) -> ValueId { let v = self.next; self.next += 1; v }
    fn emit(&mut self, instr: SsaInstr) -> ValueId {
        let id = self.fresh();
        self.blocks[self.current].instrs.push((id, instr));
        id
    }
    fn new_block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(SsaBlock { id, ..Default::default() });
        id
    }
    fn set_current(&mut self, id: usize) { self.current = id; }
    fn bind(&mut self, name: String, value: ValueId) { self.vars.last_mut().unwrap().insert(name, value); }
    fn lookup(&self, name: &str) -> Option<ValueId> {
        self.vars.iter().rev().find_map(|m| m.get(name).copied())
    }
    fn push_scope(&mut self) { self.vars.push(self.vars.last().cloned().unwrap_or_default()); }
    fn pop_scope(&mut self) { self.vars.pop(); }

    fn const_value(&mut self, v: &Value) -> ValueId {
        let sv = match v {
            Value::Num(x) => SsaValue::Number(*x),
            Value::Str(x) => SsaValue::String(x.clone()),
            Value::Bool(x) => SsaValue::Bool(*x),
            Value::Null => SsaValue::Null,
            Value::Struct { name, .. } => SsaValue::Struct { name: name.clone() },
            Value::Array(_) => SsaValue::Null,
        };
        self.emit(SsaInstr::Const(sv))
    }

    fn expr(&mut self, e: &Expr) -> ValueId {
        match e {
            Expr::Val(v) => self.const_value(v),
            Expr::Var(name) => {
                if let Some(v) = self.lookup(name) { v }
                else { self.emit(SsaInstr::Load { name: name.clone() }) }
            }
            Expr::Array(xs) => {
                let args = xs.iter().map(|x| self.expr(x)).collect::<Vec<_>>();
                self.emit(SsaInstr::Call { name: "array".into(), args, result: IrType::Any })
            }
            Expr::Unary(op, x) => {
                let value = self.expr(x);
                let ty = if *op == Token::Bang { IrType::Bool } else { IrType::Number };
                self.emit(SsaInstr::Unary { op: format!("{:?}", op), value, ty })
            }
            Expr::Binary(a, op, b) => {
                let left = self.expr(a);
                let right = self.expr(b);
                let ty = match op {
                    Token::EqEq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::And | Token::Or => IrType::Bool,
                    _ => IrType::Any,
                };
                self.emit(SsaInstr::Binary { op: format!("{:?}", op), left, right, ty })
            }
            Expr::Field(base, field) => {
                let base = self.expr(base);
                self.emit(SsaInstr::FieldGet { base, field: field.clone(), ty: IrType::Any })
            }
            Expr::StructInit(name, fields) => {
                let fields = fields.iter().map(|(field, value)| (field.clone(), self.expr(value))).collect();
                self.emit(SsaInstr::StructInit { name: name.clone(), fields })
            }
            Expr::Call(name, args) => {
                let values = args.iter().map(|x| self.expr(x)).collect();
                self.emit(SsaInstr::Call { name: name.clone(), args: values, result: IrType::Any })
            }
        }
    }

    fn stmt_list(&mut self, body: &[Stmt]) {
        for stmt in body {
            if self.blocks[self.current].terminator.is_some() { break; }
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(e) | Stmt::Print(e) => { let v = self.expr(e); if matches!(stmt, Stmt::Print(_)) { self.emit(SsaInstr::Call { name: "print".into(), args: vec![v], result: IrType::Null }); } }
            Stmt::Let(name, _, e) | Stmt::Assign(name, e) => {
                let v = self.expr(e);
                self.bind(name.clone(), v);
                self.emit(SsaInstr::Store { name: name.clone(), value: v });
            }
            Stmt::Return(e) => {
                let v = self.expr(e);
                self.blocks[self.current].terminator = Some(Terminator::Return(Some(v)));
            }
            Stmt::If(cond, then_body, else_body) => {
                let condition = self.expr(cond);
                let then_id = self.new_block();
                let else_id = self.new_block();
                let merge_id = self.new_block();
                self.blocks[self.current].terminator = Some(Terminator::Branch { condition, then_block: then_id, else_block: else_id });

                let incoming = self.vars.last().cloned().unwrap_or_default();

                self.set_current(then_id);
                self.push_scope();
                self.stmt_list(then_body);
                let then_vars = self.vars.last().cloned().unwrap_or_default();
                if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(merge_id)); }
                self.pop_scope();

                self.set_current(else_id);
                self.vars.last_mut().unwrap().clone_from(&incoming);
                self.push_scope();
                self.stmt_list(else_body);
                let else_vars = self.vars.last().cloned().unwrap_or_default();
                if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(merge_id)); }
                self.pop_scope();

                self.set_current(merge_id);
                self.vars.last_mut().unwrap().clone_from(&incoming);
                let keys = then_vars.keys().chain(else_vars.keys()).cloned().collect::<std::collections::BTreeSet<_>>();
                for name in keys {
                    let a = then_vars.get(&name).copied().or_else(|| incoming.get(&name).copied());
                    let b = else_vars.get(&name).copied().or_else(|| incoming.get(&name).copied());
                    if let (Some(x), Some(y)) = (a, b) {
                        if x == y { self.bind(name, x); }
                        else {
                            let phi = self.emit(SsaInstr::Phi { incomings: vec![(then_id, x), (else_id, y)], ty: IrType::Any });
                            self.bind(name, phi);
                        }
                    }
                }
            }
            Stmt::While(cond, body) => {
                let preheader = self.current;
                let header = self.new_block();
                let loop_body = self.new_block();
                let exit = self.new_block();
                let incoming = self.vars.last().cloned().unwrap_or_default();

                self.blocks[preheader].terminator = Some(Terminator::Jump(header));
                self.set_current(header);

                // Loop-carried values are represented by header phis. The
                // preheader provides the first iteration value; the body
                // provides the backedge value when the body actually loops.
                let mut phis = Vec::<(String, ValueId)>::new();
                for (name, value) in incoming.iter() {
                    let phi = self.fresh();
                    self.blocks[header].instrs.push((phi, SsaInstr::Phi {
                        incomings: vec![(preheader, *value)],
                        ty: IrType::Any,
                    }));
                    self.bind(name.clone(), phi);
                    phis.push((name.clone(), phi));
                }

                let condition = self.expr(cond);
                self.blocks[header].terminator = Some(Terminator::Branch {
                    condition,
                    then_block: loop_body,
                    else_block: exit,
                });

                self.set_current(loop_body);
                self.push_scope();
                self.stmt_list(body);
                let body_vars = self.vars.last().cloned().unwrap_or_default();
                let loops_back = self.blocks[self.current].terminator.is_none();
                if loops_back {
                    self.blocks[self.current].terminator = Some(Terminator::Jump(header));
                }
                let backedge = self.current;
                self.pop_scope();

                if loops_back {
                    for (name, phi) in &phis {
                        let initial = incoming.get(name).copied().unwrap();
                        let value = body_vars.get(name).copied().unwrap_or(initial);
                        if let Some((_, instr)) = self.blocks[header].instrs.iter_mut().find(|(id, _)| *id == *phi) {
                            if let SsaInstr::Phi { incomings, .. } = instr {
                                incomings.push((backedge, value));
                            }
                        }
                    }
                }

                // The header phi is also the value visible after the loop:
                // the exit edge leaves the header after the final condition
                // check, so the phi dominates the exit block.
                self.set_current(exit);
                self.vars.last_mut().unwrap().clone_from(&incoming);
                for (name, phi) in phis {
                    self.bind(name, phi);
                }
            }
            Stmt::For(name, iterable, body) => {
                let iterable_v = self.expr(iterable);
                self.emit(SsaInstr::Call { name: "for_each".into(), args: vec![iterable_v], result: IrType::Null });
                self.push_scope();
                let item = self.emit(SsaInstr::Call { name: "loop_item".into(), args: vec![], result: IrType::Any });
                self.bind(name.clone(), item);
                self.stmt_list(body);
                self.pop_scope();
            }
            Stmt::Match(value, arms, otherwise) => {
                let subject = self.expr(value);
                let exit = self.new_block();
                let mut next = self.current;
                for (pattern, body) in arms {
                    self.set_current(next);
                    let p = self.expr(pattern);
                    let test = self.emit(SsaInstr::Binary { op: "EqEq".into(), left: subject, right: p, ty: IrType::Bool });
                    let yes = self.new_block();
                    let no = self.new_block();
                    self.blocks[self.current].terminator = Some(Terminator::Branch { condition: test, then_block: yes, else_block: no });
                    self.set_current(yes);
                    self.stmt_list(body);
                    if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(exit)); }
                    next = no;
                }
                self.set_current(next);
                self.stmt_list(otherwise);
                if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(exit)); }
                self.set_current(exit);
            }
            Stmt::Import(_) | Stmt::StructDecl(_, _) | Stmt::Fn(..) => {}
        }
    }
}

pub fn lower_function(name: &str, args: &[(String, crate::types::Type)], ret: &crate::types::Type, body: &[Stmt]) -> SsaFunction {
    let mut b = Builder::new();
    let mut params = Vec::new();
    for (arg, ty) in args {
        let id = b.fresh();
        b.bind(arg.clone(), id);
        params.push((arg.clone(), type_to_ir(ty), id));
    }
    b.stmt_list(body);
    if b.blocks.iter().any(|x| x.terminator.is_none()) {
        for block in &mut b.blocks {
            if block.terminator.is_none() { block.terminator = Some(Terminator::Return(None)); }
        }
    }
    SsaFunction { name: name.into(), params, return_type: type_to_ir(ret), blocks: b.blocks }
}

fn type_to_ir(t: &crate::types::Type) -> IrType {
    match t {
        crate::types::Type::Bool => IrType::Bool,
        crate::types::Type::String => IrType::String,
        crate::types::Type::Null | crate::types::Type::Void => IrType::Null,
        crate::types::Type::Struct(name) => IrType::Struct(name.clone()),
        _ => IrType::Any,
    }
}

pub fn lower_program(program: &[Stmt]) -> SsaFunction {
    lower_function("<main>", &[], &crate::types::Type::Void, program)
}

pub fn verify_program(program: &[Stmt]) -> Result<(), String> {
    let main = lower_program(program);
    main.validate()?;
    main.verify_operands()?;
    Ok(())
}
