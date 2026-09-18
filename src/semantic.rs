use crate::{Expr, Stmt, Value, Token, Pattern};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct StaticFn {
    args: Vec<crate::types::Type>,
    ret: crate::types::Type,
}

pub struct Checker {
    scopes: Vec<HashMap<String, crate::types::Type>>,
    fns: HashMap<String, StaticFn>,
    structs: HashMap<String, HashMap<String, crate::types::Type>>,
    enums: HashMap<String, HashMap<String, Option<crate::types::Type>>>,
    errors: Vec<String>,
}

impl Checker {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()], fns: HashMap::new(), structs: HashMap::new(), enums: HashMap::new(), errors: Vec::new() }
    }

    fn error(&mut self, msg: impl Into<String>) { self.errors.push(msg.into()); }
    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }

    fn lookup(&self, name: &str) -> Option<crate::types::Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn define(&mut self, name: String, ty: crate::types::Type) {
        if let Some(scope) = self.scopes.last_mut() { scope.insert(name, ty); }
    }

    fn value_type(&self, v: &Value) -> crate::types::Type {
        match v {
            Value::Num(_) => crate::types::Type::Number,
            Value::Str(_) => crate::types::Type::String,
            Value::Bool(_) => crate::types::Type::Bool,
            Value::Null => crate::types::Type::Null,
            Value::Struct { name, .. } => crate::types::Type::Struct(name.clone()),
            Value::Enum { name, .. } => crate::types::Type::Enum(name.clone()),
            Value::Array(xs) => {
                if xs.is_empty() { return crate::types::Type::Array(Box::new(crate::types::Type::Any)); }
                let first = self.value_type(&xs[0]);
                for x in xs.iter().skip(1) {
                    let t = self.value_type(x);
                    if !first.compatible(&t) {
                        return crate::types::Type::Array(Box::new(crate::types::Type::Any));
                    }
                }
                crate::types::Type::Array(Box::new(first))
            }
        }
    }

    fn infer(&mut self, e: &Expr) -> crate::types::Type {
        match e {
            Expr::Val(v) => self.value_type(v),
            Expr::Var(n) => self.lookup(n).unwrap_or_else(|| {
                self.error(format!("undefined variable {}", n));
                crate::types::Type::Unknown
            }),
            Expr::EnumInit(name, variant, value) => {
                match self.enums.get(name).and_then(|m|m.get(variant)).cloned() {
                    Some(expected) => { if let (Some(want), Some(expr)) = (expected, value) { let got=self.infer(expr); if !want.compatible(&got){self.error(format!("enum {}.{} expects {}, got {}",name,variant,want.name(),got.name()));} } }
                    None => self.error(format!("unknown enum variant {}.{}",name,variant)),
                }
                crate::types::Type::Enum(name.clone())
            }
            Expr::StructInit(name, fields) => {
                let schema = match self.structs.get(name).cloned() { Some(x)=>x, None=>{ self.error(format!("undefined struct {}",name)); return crate::types::Type::Unknown; } };
                if fields.len() != schema.len() { self.error(format!("struct {} expects {} fields, got {}", name, schema.len(), fields.len())); }
                let mut seen = std::collections::HashSet::new();
                for (field, expr) in fields {
                    let got = self.infer(expr);
                    if !seen.insert(field) { self.error(format!("duplicate field {} in {}", field, name)); }
                    match schema.get(field) { Some(want) if !want.compatible(&got) => self.error(format!("field {}.{} expects {}, got {}", name, field, want.name(), got.name())), None => self.error(format!("unknown field {}.{}", name, field)), _=>{} }
                }
                crate::types::Type::Struct(name.clone())
            }
            Expr::Field(base, field) => {
                let t = self.infer(base);
                match t {
                    crate::types::Type::Struct(name) => match self.structs.get(&name).and_then(|m|m.get(field)).cloned() { Some(t)=>t, None=>{self.error(format!("unknown field {}.{}",name,field));crate::types::Type::Unknown} },
                    _ => { self.error(format!("field access requires struct, got {}", t.name())); crate::types::Type::Unknown }
                }
            }
            Expr::Array(xs) => {
                if xs.is_empty() { return crate::types::Type::Array(Box::new(crate::types::Type::Any)); }
                let first = self.infer(&xs[0]);
                for x in xs.iter().skip(1) {
                    let t = self.infer(x);
                    if !first.compatible(&t) {
                        self.error(format!("array elements have incompatible types: {} and {}", first.name(), t.name()));
                    }
                }
                crate::types::Type::Array(Box::new(first))
            }
            Expr::Unary(op, x) => {
                let t = self.infer(x);
                match op {
                    Token::Minus if !t.compatible(&crate::types::Type::Number) => {
                        self.error(format!("unary - expects number, got {}", t.name()));
                        crate::types::Type::Unknown
                    }
                    Token::Minus => crate::types::Type::Number,
                    Token::Bang => crate::types::Type::Bool,
                    _ => crate::types::Type::Unknown,
                }
            }
            Expr::Binary(a, op, b) => {
                let x = self.infer(a);
                let y = self.infer(b);
                match op {
                    Token::Plus => {
                        if x.compatible(&crate::types::Type::Number) && y.compatible(&crate::types::Type::Number) {
                            crate::types::Type::Number
                        } else if x.compatible(&crate::types::Type::String) && y.compatible(&crate::types::Type::String) {
                            crate::types::Type::String
                        } else {
                            self.error(format!("operator + cannot combine {} and {}", x.name(), y.name()));
                            crate::types::Type::Unknown
                        }
                    }
                    Token::Minus | Token::Star | Token::Slash | Token::Percent => {
                        if !x.compatible(&crate::types::Type::Number) || !y.compatible(&crate::types::Type::Number) {
                            self.error("arithmetic operator expects numbers");
                            crate::types::Type::Unknown
                        } else { crate::types::Type::Number }
                    }
                    Token::Lt | Token::Le | Token::Gt | Token::Ge => {
                        if !x.compatible(&crate::types::Type::Number) || !y.compatible(&crate::types::Type::Number) {
                            self.error("ordered comparison expects numbers");
                        }
                        crate::types::Type::Bool
                    }
                    Token::EqEq | Token::Ne => crate::types::Type::Bool,
                    Token::And | Token::Or => {
                        if !x.compatible(&crate::types::Type::Bool) || !y.compatible(&crate::types::Type::Bool) {
                            self.error("logical operator expects booleans");
                        }
                        crate::types::Type::Bool
                    }
                    _ => crate::types::Type::Unknown,
                }
            }
            Expr::Call(name, args) => {
                if let Some(f) = self.fns.get(name).cloned() {
                    if f.args.len() != args.len() {
                        self.error(format!("{} expects {} arguments, got {}", name, f.args.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let got = self.infer(arg);
                        if let Some(expected) = f.args.get(i) {
                            if !expected.compatible(&got) {
                                self.error(format!("argument {} of {} expects {}, got {}", i + 1, name, expected.name(), got.name()));
                            }
                        }
                    }
                    return f.ret;
                }

                if name=="None" { return crate::types::Type::Enum("Option".into()); }
                if name=="Some" || name=="Ok" || name=="Err" { if args.len()!=1 { self.error(format!("{} expects 1 argument",name)); } else { self.infer(&args[0]); } return crate::types::Type::Enum(if name=="Some" {"Option"} else {"Result"}.into()); }

                let builtin = match name.as_str() {
                    "range" => Some((vec![crate::types::Type::Number], crate::types::Type::Array(Box::new(crate::types::Type::Number)))),
                    "str" => Some((vec![crate::types::Type::Any], crate::types::Type::String)),
                    "len" => Some((vec![crate::types::Type::Any], crate::types::Type::Number)),
                    "abs" | "sqrt" => Some((vec![crate::types::Type::Number], crate::types::Type::Number)),
                    "read_file" => Some((vec![crate::types::Type::String], crate::types::Type::String)),
                    "write_file" => Some((vec![crate::types::Type::String, crate::types::Type::String], crate::types::Type::Null)),
                    "exists" => Some((vec![crate::types::Type::String], crate::types::Type::Bool)),
                    "env" => Some((vec![crate::types::Type::String], crate::types::Type::String)),
                    _ => None,
                };
                if let Some((expected, ret)) = builtin {
                    if expected.len() != args.len() {
                        self.error(format!("{} expects {} arguments, got {}", name, expected.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let got = self.infer(arg);
                        if let Some(want) = expected.get(i) {
                            if !want.compatible(&got) {
                                self.error(format!("argument {} of {} expects {}, got {}", i + 1, name, want.name(), got.name()));
                            }
                        }
                    }
                    return ret;
                }

                for arg in args { self.infer(arg); }
                self.error(format!("undefined function {}", name));
                crate::types::Type::Unknown
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, subject: &crate::types::Type, covered: &mut std::collections::HashSet<String>, wildcard: &mut bool) {
        match pattern {
            Pattern::Wildcard => *wildcard = true,
            Pattern::Literal(expr) => { let t=self.infer(expr); if !subject.compatible(&t) { self.error(format!("match pattern expects {}, got {}", subject.name(), t.name())); } }
            Pattern::Enum { variant, .. } => match subject {
                crate::types::Type::Enum(name) => {
                    let known = self.enums.get(name).map(|vars| vars.contains_key(variant)).unwrap_or(false);
                    if !known { self.error(format!("unknown variant {}.{}",name,variant)); }
                    else { covered.insert(variant.clone()); }
                }
                _ => self.error(format!("enum pattern {} requires enum subject, got {}",variant,subject.name())),
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, expected_return: Option<&crate::types::Type>) {
        match stmt {
            Stmt::Expr(e) | Stmt::Print(e) => { self.infer(e); }
            Stmt::Let(name, explicit, e) => {
                let inferred = self.infer(e);
                let ty = explicit.clone().unwrap_or_else(|| inferred.clone());
                if let Some(ex) = explicit {
                    if !ex.compatible(&inferred) {
                        self.error(format!("type annotation for {} expects {}, got {}", name, ex.name(), inferred.name()));
                    }
                }
                self.define(name.clone(), ty);
            }
            Stmt::Assign(name, e) => {
                let got = self.infer(e);
                if let Some(old) = self.lookup(name) {
                    if !old.compatible(&got) {
                        self.error(format!("cannot assign {} to {} (expected {})", got.name(), name, old.name()));
                    }
                } else {
                    self.error(format!("assignment to undefined variable {}", name));
                }
            }
            Stmt::Return(e) => {
                let got = self.infer(e);
                if let Some(expected) = expected_return {
                    if !expected.compatible(&got) {
                        self.error(format!("return type mismatch: expected {}, got {}", expected.name(), got.name()));
                    }
                }
            }
            Stmt::If(condition, then_body, else_body) => {
                let t = self.infer(condition);
                if !t.compatible(&crate::types::Type::Bool) {
                    self.error(format!("if condition expects bool, got {}", t.name()));
                }
                self.push_scope();
                for s in then_body { self.check_stmt(s, expected_return); }
                self.pop_scope();
                self.push_scope();
                for s in else_body { self.check_stmt(s, expected_return); }
                self.pop_scope();
            }
            Stmt::While(condition, body) => {
                let t = self.infer(condition);
                if !t.compatible(&crate::types::Type::Bool) {
                    self.error(format!("while condition expects bool, got {}", t.name()));
                }
                self.push_scope();
                for s in body { self.check_stmt(s, expected_return); }
                self.pop_scope();
            }
            Stmt::For(name, iterable, body) => {
                let t = self.infer(iterable);
                let item = match t {
                    crate::types::Type::Array(inner) => *inner,
                    _ => crate::types::Type::Any,
                };
                self.push_scope();
                self.define(name.clone(), item);
                for s in body { self.check_stmt(s, expected_return); }
                self.pop_scope();
            }
            Stmt::Match(value, arms, otherwise) => {
                let subject = self.infer(value);
                let mut covered = std::collections::HashSet::new();
                let mut wildcard = false;
                for (pattern, body) in arms {
                    self.check_pattern(pattern, &subject, &mut covered, &mut wildcard);
                    self.push_scope();
                    if let Pattern::Enum { variant, binding: Some(name) } = pattern {
                        if let crate::types::Type::Enum(enum_name) = &subject {
                            if let Some(Some(payload)) = self.enums.get(enum_name).and_then(|m| m.get(variant)).cloned() {
                                self.define(name.clone(), payload);
                            } else { self.define(name.clone(), crate::types::Type::Any); }
                        } else { self.define(name.clone(), crate::types::Type::Any); }
                    }
                    for s in body { self.check_stmt(s, expected_return); }
                    self.pop_scope();
                }
                self.push_scope();
                for s in otherwise { self.check_stmt(s, expected_return); }
                self.pop_scope();
                if otherwise.is_empty() && !wildcard {
                    if let crate::types::Type::Enum(enum_name) = &subject {
                        if let Some(vars) = self.enums.get(enum_name) {
                            let missing: Vec<_> = vars.keys().filter(|v| !covered.contains(*v)).cloned().collect();
                            if !missing.is_empty() { self.error(format!("non-exhaustive match on {}: missing {}", enum_name, missing.join(", "))); }
                        }
                    }
                }
            }
            Stmt::Import(_) => {}
            Stmt::StructDecl(_, _) => {},
            Stmt::EnumDecl(_, _) => {}
            Stmt::Fn(_, args, ret, body) => {
                self.push_scope();
                for (name, ty) in args { self.define(name.clone(), ty.clone()); }
                for s in body { self.check_stmt(s, Some(ret)); }
                self.pop_scope();
            }
        }
    }

    pub fn check(&mut self, program: &[Stmt]) -> Result<(), Vec<String>> {
        self.enums.entry("Option".into()).or_insert_with(|| [("None".into(),None),("Some".into(),Some(crate::types::Type::Any))].into_iter().collect());
        self.enums.entry("Result".into()).or_insert_with(|| [("Ok".into(),Some(crate::types::Type::Any)),("Err".into(),Some(crate::types::Type::Any))].into_iter().collect());
        for stmt in program {
            if let Stmt::EnumDecl(name, variants) = stmt {
                if self.enums.contains_key(name) { self.error(format!("duplicate enum {}", name)); } else { self.enums.insert(name.clone(), variants.iter().cloned().collect()); }
            }
            if let Stmt::StructDecl(name, fields) = stmt {
                if self.structs.contains_key(name) { self.error(format!("duplicate struct {}", name)); }
                else { self.structs.insert(name.clone(), fields.iter().cloned().collect()); }
            }
            if let Stmt::Fn(name, args, ret, _) = stmt {
                if self.fns.contains_key(name) {
                    self.error(format!("duplicate function {}", name));
                } else {
                    self.fns.insert(name.clone(), StaticFn {
                        args: args.iter().map(|(_, t)| t.clone()).collect(),
                        ret: ret.clone(),
                    });
                }
            }
        }

        for stmt in program { self.check_stmt(stmt, None); }

        if self.errors.is_empty() { Ok(()) } else { Err(self.errors.clone()) }
    }
}
