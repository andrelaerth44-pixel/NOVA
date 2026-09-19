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
                // Normalize every supported iterable through the native
                // Iterator abstraction.
                let source = self.expr(iterable);
                let iterator = self.emit(SsaInstr::Call {
                    name: "iter".into(),
                    args: vec![source],
                    result: IrType::Generic("Iterator".into(), vec![IrType::Any]),
                });
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

                let condition = self.emit(SsaInstr::Call {
                    name: "has_next".into(),
                    args: vec![iterator],
                    result: IrType::Bool,
                });
                self.blocks[header].terminator = Some(Terminator::Branch {
                    condition,
                    then_block: loop_body,
                    else_block: exit,
                });

                self.set_current(loop_body);
                self.push_scope();
                let next_value = self.emit(SsaInstr::Call {
                    name: "next".into(),
                    args: vec![iterator],
                    result: IrType::Generic("Option".into(), vec![IrType::Any]),
                });
                let item = self.emit(SsaInstr::Call {
                    name: "unwrap".into(),
                    args: vec![next_value],
                    result: IrType::Any,
                });
                self.bind(name.clone(), item);
                self.stmt_list(body);
                let body_vars = self.vars.last().cloned().unwrap_or_default();
                let loops_back = self.blocks[self.current].terminator.is_none();

                if loops_back {
                    self.blocks[self.current].terminator = Some(Terminator::Jump(header));
                    let backedge = self.current;

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
