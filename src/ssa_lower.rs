use crate::{Expr, Stmt, Value, Token, Pattern};
use crate::ir::{IrType, SsaBlock, SsaFunction, SsaInstr, SsaValue, Terminator, ValueId};
use std::collections::HashMap;


fn collect_expr_vars(
    expr: &Expr,
    bound: &mut std::collections::HashSet<String>,
    used: &mut std::collections::BTreeSet<String>,
) {
    match expr {
        Expr::Val(_) => {}
        Expr::Var(name) => {
            if !bound.contains(name) { used.insert(name.clone()); }
        }
        Expr::Unary(_, inner) | Expr::Try(inner) => collect_expr_vars(inner, bound, used),
        Expr::Binary(left, _, right) => {
            collect_expr_vars(left, bound, used);
            collect_expr_vars(right, bound, used);
        }
        Expr::Call(_, args) | Expr::Array(args) | Expr::Set(args) => {
            for arg in args { collect_expr_vars(arg, bound, used); }
        }
        Expr::CallValue(callee, args) => {
            collect_expr_vars(callee, bound, used);
            for arg in args { collect_expr_vars(arg, bound, used); }
        }
        Expr::Map(entries) => {
            for (key, value) in entries {
                collect_expr_vars(key, bound, used);
                collect_expr_vars(value, bound, used);
            }
        }
        Expr::Index(base, index) => {
            collect_expr_vars(base, bound, used);
            collect_expr_vars(index, bound, used);
        }
        Expr::Field(base, _) => collect_expr_vars(base, bound, used),
        Expr::StructInit(_, fields) => {
            for (_, value) in fields { collect_expr_vars(value, bound, used); }
        }
        Expr::EnumInit(_, _, payload) => {
            if let Some(value) = payload { collect_expr_vars(value, bound, used); }
        }
        Expr::Closure(params, body) => {
            let mut nested_bound = bound.clone();
            for param in params { nested_bound.insert(param.clone()); }
            collect_stmt_vars(body, &mut nested_bound, used);
        }
    }
}

fn collect_stmt_vars(
    stmts: &[Stmt],
    bound: &mut std::collections::HashSet<String>,
    used: &mut std::collections::BTreeSet<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(expr) | Stmt::Print(expr) | Stmt::Return(expr) => {
                collect_expr_vars(expr, bound, used);
            }
            Stmt::Let(name, _, expr) => {
                collect_expr_vars(expr, bound, used);
                bound.insert(name.clone());
            }
            Stmt::Assign(name, expr) => {
                if !bound.contains(name) { used.insert(name.clone()); }
                collect_expr_vars(expr, bound, used);
            }
            Stmt::AssignTarget(target, expr) => {
                collect_expr_vars(target, bound, used);
                collect_expr_vars(expr, bound, used);
            }
            Stmt::If(condition, then_body, else_body) => {
                collect_expr_vars(condition, bound, used);
                let mut then_bound = bound.clone();
                collect_stmt_vars(then_body, &mut then_bound, used);
                let mut else_bound = bound.clone();
                collect_stmt_vars(else_body, &mut else_bound, used);
            }
            Stmt::While(condition, body) => {
                collect_expr_vars(condition, bound, used);
                let mut loop_bound = bound.clone();
                collect_stmt_vars(body, &mut loop_bound, used);
            }
            Stmt::For(name, iterable, body) => {
                collect_expr_vars(iterable, bound, used);
                let mut for_bound = bound.clone();
                for_bound.insert(name.clone());
                collect_stmt_vars(body, &mut for_bound, used);
            }
            Stmt::Match(subject, arms, otherwise) => {
                collect_expr_vars(subject, bound, used);
                for (pattern, body) in arms {
                    let mut arm_bound = bound.clone();
                    if let Pattern::Enum { binding: Some(name), .. } = pattern {
                        arm_bound.insert(name.clone());
                    }
                    collect_stmt_vars(body, &mut arm_bound, used);
                }
                let mut else_bound = bound.clone();
                collect_stmt_vars(otherwise, &mut else_bound, used);
            }
            Stmt::Fn(_, _, _, _, _) | Stmt::Import(_) | Stmt::StructDecl(_, _) | Stmt::EnumDecl(_, _) => {}
        }
    }
}

#[derive(Clone)]
struct ClosureSpec {
    function: String,
    params: Vec<String>,
    body: Vec<Stmt>,
    captures: Vec<String>,
}


type EnumVariants = std::collections::HashMap<String, String>;

fn collect_enum_variants(stmts: &[Stmt]) -> EnumVariants {
    let mut variants = EnumVariants::new();
    for stmt in stmts {
        if let Stmt::EnumDecl(name, members) = stmt {
            for (variant, _) in members {
                variants.insert(variant.clone(), name.clone());
            }
        }
    }
    variants
}

struct Builder {
    blocks: Vec<SsaBlock>,
    current: usize,
    next: ValueId,
    vars: Vec<HashMap<String, ValueId>>,
    owner_name: String,
    closure_counter: usize,
    closures: Vec<ClosureSpec>,
    enum_variants: EnumVariants,
}

impl Builder {
    fn new(owner_name: impl Into<String>) -> Self {
        Self::new_with_enums(owner_name, EnumVariants::new())
    }

    fn new_with_enums(
        owner_name: impl Into<String>,
        enum_variants: EnumVariants,
    ) -> Self {
        Self {
            blocks: vec![SsaBlock { id: 0, ..Default::default() }],
            current: 0,
            next: 0,
            vars: vec![HashMap::new()],
            owner_name: owner_name.into(),
            closure_counter: 0,
            closures: Vec::new(),
            enum_variants,
        }
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
            Value::Enum { name, .. } => SsaValue::Struct { name: name.clone() },
            Value::Array(_) | Value::Map(_) | Value::Set(_) => SsaValue::Null,
            Value::Closure { .. } => SsaValue::Null,
            Value::Iterator(_) => SsaValue::Null,
        };
        self.emit(SsaInstr::Const(sv))
    }

    fn expr(&mut self, e: &Expr) -> ValueId {
        match e {
            Expr::Val(v) => self.const_value(v),
            Expr::Var(name) => {
                if name == "None" {
                    self.emit(SsaInstr::EnumInit {
                        name: "Option".into(),
                        variant: "None".into(),
                        payload: None,
                        ty: IrType::Generic("Option".into(), vec![IrType::Any]),
                    })
                } else if let Some(v) = self.lookup(name) {
                    v
                } else {
                    self.emit(SsaInstr::Load { name: name.clone() })
                }
            }
            Expr::Array(xs) => {
                let args = xs.iter().map(|x| self.expr(x)).collect::<Vec<_>>();
                self.emit(SsaInstr::Call { name: "array".into(), args, result: IrType::Any })
            }
            Expr::Map(entries) => {
                let mut args = Vec::with_capacity(entries.len() * 2);
                for (key, value) in entries {
                    args.push(self.expr(key));
                    args.push(self.expr(value));
                }
                self.emit(SsaInstr::Call { name: "map".into(), args, result: IrType::Any })
            }
            Expr::Set(values) => {
                let args = values.iter().map(|x| self.expr(x)).collect::<Vec<_>>();
                self.emit(SsaInstr::Call { name: "set".into(), args, result: IrType::Any })
            }
            Expr::Index(base, index) => {
                let args = vec![self.expr(base), self.expr(index)];
                self.emit(SsaInstr::Call { name: "index".into(), args, result: IrType::Any })
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
            Expr::EnumInit(name, variant, value) => {
                let payload = value.as_ref().map(|v| self.expr(v));
                self.emit(SsaInstr::EnumInit {
                    name: name.clone(),
                    variant: variant.clone(),
                    payload,
                    ty: IrType::Enum(name.clone()),
                })
            }
            Expr::Field(base, field) => {
                let base = self.expr(base);
                self.emit(SsaInstr::FieldGet { base, field: field.clone(), ty: IrType::Any })
            }
            Expr::StructInit(name, fields) => {
                let fields = fields.iter().map(|(field, value)| (field.clone(), self.expr(value))).collect();
                self.emit(SsaInstr::StructInit { name: name.clone(), fields })
            }
            Expr::Closure(args, body) => {
                let mut bound = std::collections::HashSet::new();
                for arg in args { bound.insert(arg.clone()); }
                let mut used = std::collections::BTreeSet::new();
                collect_stmt_vars(body, &mut bound, &mut used);
                let capture_names = used.into_iter().collect::<Vec<_>>();
                let captures = capture_names
                    .iter()
                    .filter_map(|name| self.lookup(name).map(|value| (name.clone(), value)))
                    .collect::<Vec<_>>();
                let function = format!("{}__closure{}", self.owner_name, self.closure_counter);
                self.closure_counter += 1;
                self.closures.push(ClosureSpec {
                    function: function.clone(),
                    params: args.clone(),
                    body: body.clone(),
                    captures: capture_names,
                });
                let ty = IrType::Function(
                    vec![IrType::Any; args.len()],
                    Box::new(IrType::Any),
                );
                self.emit(SsaInstr::Closure {
                    function,
                    params: args.clone(),
                    captures,
                    ty,
                })
            },
            Expr::CallValue(callee, args) => {
                let callee = self.expr(callee);
                let values = args.iter().map(|x| self.expr(x)).collect();
                self.emit(SsaInstr::CallIndirect {
                    callee,
                    args: values,
                    result: IrType::Any,
                })
            }
            Expr::Try(inner) => {
                let value = self.expr(inner);
                self.emit(SsaInstr::Try { value, result: IrType::Any })
            }
            Expr::Call(name, args) => {
                if let Some(callee) = self.lookup(name) {
                    let values = args.iter().map(|x| self.expr(x)).collect();
                    return self.emit(SsaInstr::CallIndirect {
                        callee,
                        args: values,
                        result: IrType::Any,
                    });
                }

                match name.as_str() {
                    "None" => self.emit(SsaInstr::EnumInit {
                        name: "Option".into(),
                        variant: "None".into(),
                        payload: None,
                        ty: IrType::Generic("Option".into(), vec![IrType::Any]),
                    }),
                    "Some" => {
                        let payload = args.first().map(|x| self.expr(x));
                        self.emit(SsaInstr::EnumInit {
                            name: "Option".into(),
                            variant: "Some".into(),
                            payload,
                            ty: IrType::Generic("Option".into(), vec![IrType::Any]),
                        })
                    }
                    "Ok" => {
                        let payload = args.first().map(|x| self.expr(x));
                        self.emit(SsaInstr::EnumInit {
                            name: "Result".into(),
                            variant: "Ok".into(),
                            payload,
                            ty: IrType::Generic("Result".into(), vec![IrType::Any, IrType::Any]),
                        })
                    }
                    "Err" => {
                        let payload = args.first().map(|x| self.expr(x));
                        self.emit(SsaInstr::EnumInit {
                            name: "Result".into(),
                            variant: "Err".into(),
                            payload,
                            ty: IrType::Generic("Result".into(), vec![IrType::Any, IrType::Any]),
                        })
                    }
                    _ => {
                        let values: Vec<ValueId> = args.iter().map(|x| self.expr(x)).collect();
                        if let Some(enum_name) = self.enum_variants.get(name) {
                            self.emit(SsaInstr::EnumInit {
                                name: enum_name.clone(),
                                variant: name.clone(),
                                payload: values.first().copied(),
                                ty: IrType::Enum(enum_name.clone()),
                            })
                        } else {
                            self.emit(SsaInstr::Call { name: name.clone(), args: values, result: IrType::Any })
                        }
                    }
                }
            }
        }
    }

    fn update_target(&mut self, target: &Expr, value: ValueId) -> (String, ValueId) {
        match target {
            Expr::Var(name) => (name.clone(), value),
            Expr::Field(base, field) => {
                let base_value = self.expr(base);
                let field_value = self.emit(SsaInstr::Const(SsaValue::String(field.clone())));
                let updated_base = self.emit(SsaInstr::Call {
                    name: "field_set".into(),
                    args: vec![base_value, field_value, value],
                    result: IrType::Any,
                });
                self.update_target(base, updated_base)
            }
            Expr::Index(base, index) => {
                let base_value = self.expr(base);
                let index_value = self.expr(index);
                let updated_base = self.emit(SsaInstr::Call {
                    name: "index_set".into(),
                    args: vec![base_value, index_value, value],
                    result: IrType::Any,
                });
                self.update_target(base, updated_base)
            }
            _ => panic!("invalid structured assignment target"),
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
            Stmt::AssignTarget(target, e) => {
                let value = self.expr(e);
                let (root, updated) = self.update_target(target, value);
                self.bind(root.clone(), updated);
                self.emit(SsaInstr::Store { name: root, value: updated });
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
                // Lower a for-loop to ordinary SSA control flow.
                let iterable_v = self.expr(iterable);
                let preheader = self.current;
                let header = self.new_block();
                let loop_body = self.new_block();
                let exit = self.new_block();
                let incoming = self.vars.last().cloned().unwrap_or_default();

                self.blocks[preheader].terminator = Some(Terminator::Jump(header));
                self.set_current(header);

                let mut phis = Vec::<(String, ValueId)>::new();
                for (var_name, value) in incoming.iter() {
                    let phi = self.fresh();
                    self.blocks[header].instrs.push((phi, SsaInstr::Phi {
                        incomings: vec![(preheader, *value)],
                        ty: IrType::Any,
                    }));
                    self.bind(var_name.clone(), phi);
                    phis.push((var_name.clone(), phi));
                }

                let index_phi = self.fresh();
                let zero = self.emit(SsaInstr::Const(SsaValue::Number(0.0)));
                self.blocks[header].instrs.push((index_phi, SsaInstr::Phi {
                    incomings: vec![(preheader, zero)],
                    ty: IrType::Number,
                }));

                let length = self.emit(SsaInstr::Call {
                    name: "len".into(),
                    args: vec![iterable_v],
                    result: IrType::Number,
                });
                let condition = self.emit(SsaInstr::Binary {
                    op: "Lt".into(),
                    left: index_phi,
                    right: length,
                    ty: IrType::Bool,
                });
                self.blocks[header].terminator = Some(Terminator::Branch {
                    condition,
                    then_block: loop_body,
                    else_block: exit,
                });

                self.set_current(loop_body);
                self.push_scope();
                let item = self.emit(SsaInstr::Call {
                    name: "index".into(),
                    args: vec![iterable_v, index_phi],
                    result: IrType::Any,
                });
                self.bind(name.clone(), item);
                self.stmt_list(body);
                let body_vars = self.vars.last().cloned().unwrap_or_default();
                let loops_back = self.blocks[self.current].terminator.is_none();

                if loops_back {
                    let one = self.emit(SsaInstr::Const(SsaValue::Number(1.0)));
                    let next_index = self.emit(SsaInstr::Binary {
                        op: "Plus".into(),
                        left: index_phi,
                        right: one,
                        ty: IrType::Number,
                    });
                    self.blocks[self.current].terminator = Some(Terminator::Jump(header));
                    let backedge = self.current;

                    if let Some((_, instr)) = self.blocks[header]
                        .instrs
                        .iter_mut()
                        .find(|(id, _)| *id == index_phi)
                    {
                        if let SsaInstr::Phi { incomings, .. } = instr {
                            incomings.push((backedge, next_index));
                        }
                    }

                    for (var_name, phi) in &phis {
                        let initial = incoming.get(var_name).copied().unwrap();
                        let value = body_vars.get(var_name).copied().unwrap_or(initial);
                        if let Some((_, instr)) = self.blocks[header]
                            .instrs
                            .iter_mut()
                            .find(|(id, _)| *id == *phi)
                        {
                            if let SsaInstr::Phi { incomings, .. } = instr {
                                incomings.push((backedge, value));
                            }
                        }
                    }
                }

                self.pop_scope();
                self.set_current(exit);
                self.vars.last_mut().unwrap().clone_from(&incoming);
                for (var_name, phi) in phis {
                    self.bind(var_name, phi);
                }
            }
            Stmt::Match(value, arms, otherwise) => {
                let subject = self.expr(value);
                let exit = self.new_block();
                let mut next = self.current;
                for (pattern, body) in arms {
                    self.set_current(next);
                    let test = match pattern {
                        Pattern::Wildcard => self.emit(SsaInstr::Const(SsaValue::Bool(true))),
                        Pattern::Literal(expr) => {
                            let p = self.expr(expr);
                            self.emit(SsaInstr::Binary { op: "EqEq".into(), left: subject, right: p, ty: IrType::Bool })
                        }
                        Pattern::Enum { variant, .. } => {
                            self.emit(SsaInstr::EnumTest { value: subject, variant: variant.clone() })
                        }
                    };
                    let yes = self.new_block();
                    let no = self.new_block();
                    self.blocks[self.current].terminator = Some(Terminator::Branch { condition: test, then_block: yes, else_block: no });
                    self.set_current(yes);
                    if let Pattern::Enum { binding: Some(name), .. } = pattern {
                        let payload = self.emit(SsaInstr::EnumPayload { value: subject, ty: IrType::Any });
                        self.bind(name.to_string(), payload);
                    }
                    self.stmt_list(body);
                    if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(exit)); }
                    next = no;
                }
                self.set_current(next);
                self.stmt_list(otherwise);
                if self.blocks[self.current].terminator.is_none() { self.blocks[self.current].terminator = Some(Terminator::Jump(exit)); }
                self.set_current(exit);
            }
            Stmt::Import(_) | Stmt::StructDecl(_, _) | Stmt::EnumDecl(_, _) | Stmt::Fn(..) => {}
        }
    }
}

fn lower_function_tree_with_captures(
    name: &str,
    args: &[(String, crate::types::Type)],
    ret: &crate::types::Type,
    body: &[Stmt],
    captures: &[String],
    enum_variants: &EnumVariants,
) -> Vec<SsaFunction> {
    let mut b = Builder::new_with_enums(name, enum_variants.clone());
    let mut params = Vec::new();

    let mut capture_params = Vec::new();
    for capture in captures {
        let id = b.fresh();
        b.bind(capture.clone(), id);
        capture_params.push((capture.clone(), IrType::Any, id));
    }

    for (arg, ty) in args {
        let id = b.fresh();
        b.bind(arg.clone(), id);
        params.push((arg.clone(), type_to_ir(ty), id));
    }

    b.stmt_list(body);
    if b.blocks.iter().any(|x| x.terminator.is_none()) {
        for block in &mut b.blocks {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Return(None));
            }
        }
    }

    let closure_specs = b.closures.clone();
    let function = SsaFunction {
        name: name.into(),
        params,
        captures: capture_params,
        return_type: type_to_ir(ret),
        blocks: b.blocks,
    };

    let mut functions = vec![function];
    for spec in closure_specs {
        functions.extend(lower_function_tree_with_captures(
            &spec.function,
            &spec.params.iter().map(|p| (p.clone(), crate::types::Type::Any)).collect::<Vec<_>>(),
            &crate::types::Type::Any,
            &spec.body,
            &spec.captures,
            enum_variants,
        ));
    }
    functions
}

pub fn lower_function(
    name: &str,
    args: &[(String, crate::types::Type)],
    ret: &crate::types::Type,
    body: &[Stmt],
) -> SsaFunction {
    lower_function_tree_with_captures(name, args, ret, body, &[], &EnumVariants::new())
        .into_iter()
        .next()
        .expect("lower_function always produces a function")
}

fn type_to_ir(t: &crate::types::Type) -> IrType {
    IrType::from_type(t)
}

pub fn lower_function_tree(
    name: &str,
    args: &[(String, crate::types::Type)],
    ret: &crate::types::Type,
    body: &[Stmt],
) -> Vec<SsaFunction> {
    lower_function_tree_with_captures(name, args, ret, body, &[], &collect_enum_variants(body))
}

pub fn lower_program(program: &[Stmt]) -> SsaFunction {
    lower_function_tree_with_captures(
        "<main>",
        &[],
        &crate::types::Type::Void,
        program,
        &[],
        &collect_enum_variants(program),
    )
    .into_iter()
    .next()
    .expect("lower_program always produces a function")
}

pub fn lower_program_tree(program: &[Stmt]) -> Vec<SsaFunction> {
    lower_function_tree_with_captures(
        "<main>",
        &[],
        &crate::types::Type::Void,
        program,
        &[],
        &collect_enum_variants(program),
    )
}

pub fn verify_program(program: &[Stmt]) -> Result<(), String> {
    let main = lower_program(program);
    main.validate()?;
    main.verify_operands()?;
    Ok(())
}
