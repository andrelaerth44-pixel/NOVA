use crate::{Expr, Stmt, Token, Value, Pattern, Parser, lex, EnvFrame, EnvRef, to_map_key, MapKey};
use std::{collections::HashMap, fs};

#[derive(Clone, Debug)]
pub enum RuntimeError {
    Failure(String),
    Propagate(Value),
}
impl From<String> for RuntimeError {
    fn from(value: String) -> Self { RuntimeError::Failure(value) }
}
impl From<&str> for RuntimeError {
    fn from(value: &str) -> Self { RuntimeError::Failure(value.into()) }
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Failure(msg) => write!(f, "{}", msg),
            RuntimeError::Propagate(v) => write!(f, "uncaught propagation of {}", v),
        }
    }
}
impl std::error::Error for RuntimeError {}

#[derive(Clone)]
struct Function { pub args: Vec<String>, pub body: Vec<Stmt> }

pub struct Vm {
    env: EnvRef,
    fns: HashMap<String, Function>,
    enums: HashMap<String, HashMap<String, Option<crate::types::Type>>>,
    modules: HashMap<String, bool>,
    module_stack: Vec<std::path::PathBuf>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            env: std::rc::Rc::new(std::cell::RefCell::new(EnvFrame { values: HashMap::new(), parent: None })),
            fns: HashMap::new(),
            enums: HashMap::new(),
            modules: HashMap::new(),
            module_stack: Vec::new(),
        }
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        let mut current = Some(self.env.clone());
        while let Some(env) = current {
            let (value, parent) = {
                let frame = env.borrow();
                (frame.values.get(name).cloned(), frame.parent.clone())
            };
            if value.is_some() { return value; }
            current = parent;
        }
        None
    }

    fn define(&mut self, name: String, value: Value) {
        self.env.borrow_mut().values.insert(name, value);
    }

    fn assign_in(env: &EnvRef, name: &str, value: Value) -> bool {
        let parent = {
            let mut frame = env.borrow_mut();
            if frame.values.contains_key(name) {
                frame.values.insert(name.to_string(), value);
                return true;
            }
            frame.parent.clone()
        };
        parent.map(|p| Self::assign_in(&p, name, value)).unwrap_or(false)
    }

    fn assign(&mut self, name: String, value: Value) {
        if !Self::assign_in(&self.env, &name, value.clone()) {
            self.define(name, value);
        }
    }

    fn enter_scope(&mut self) -> EnvRef {
        let parent = self.env.clone();
        self.env = std::rc::Rc::new(std::cell::RefCell::new(EnvFrame { values: HashMap::new(), parent: Some(parent.clone()) }));
        parent
    }

    fn leave_scope(&mut self, parent: EnvRef) {
        self.env = parent;
    }

    pub fn exec_program(&mut self, program: &[Stmt]) -> Result<(), String> {
        self.exec(program).map(|_| ()).map_err(|e| e.to_string())
    }

    fn eval(&mut self, e: &Expr) -> Result<Value, RuntimeError> {
        match e {
            Expr::Val(v) => Ok(v.clone()),
            Expr::Try(inner) => {
                match self.eval(inner)? {
                    Value::Enum { variant, value, .. } if variant == "Some" || variant == "Ok" => {
                        Ok(value.map(|v| *v).unwrap_or(Value::Null))
                    }
                    ref v @ Value::Enum { ref variant, .. } if variant == "None" || variant == "Err" => {
                        Err(RuntimeError::Propagate(v.clone()))
                    }
                    v => Err(RuntimeError::Failure(format!("try requires Option/Result, got {}", v))),
                }
            }
            Expr::EnumInit(name, variant, value) => Ok(Value::Enum {
                name: name.clone(),
                variant: variant.clone(),
                value: match value { Some(v) => Some(Box::new(self.eval(v)?)), None => None },
            }),
            Expr::StructInit(name, fields) => {
                let mut out = HashMap::new();
                for (field, expr) in fields { out.insert(field.clone(), self.eval(expr)?); }
                Ok(Value::Struct { name: name.clone(), fields: out })
            }
            Expr::Closure(args, body) => Ok(Value::Closure {
                args: args.clone(),
                body: body.clone(),
                env: self.env.clone(),
            }),
            Expr::CallValue(callee, a) => {
                let v = self.eval(callee)?;
                let vals = a.iter().map(|e| self.eval(e)).collect::<Result<Vec<_>, _>>()?;
                match v {
                    Value::Closure { args, body, env } => {
                        if args.len() != vals.len() {
                            return Err(format!("closure expects {} arguments", args.len()).into());
                        }
                        let old_env = self.env.clone();
                        self.env = std::rc::Rc::new(std::cell::RefCell::new(EnvFrame {
                            values: HashMap::new(),
                            parent: Some(env.clone()),
                        }));
                        for (k, v) in args.iter().zip(vals) { self.define(k.clone(), v); }
                        let result = self.exec(&body);
                        self.env = old_env;
                        match result {
                            Ok(Some(v)) => Ok(v),
                            Ok(None) => Ok(Value::Null),
                            Err(RuntimeError::Propagate(v)) => Ok(v),
                            Err(e) => Err(e),
                        }
                    }
                    _ => Err("value is not callable".into()),
                }
            }
            Expr::Field(base, field) => {
                match self.eval(base)? {
                    Value::Struct { fields, .. } => fields.get(field).cloned().ok_or_else(|| format!("unknown field {}", field).into()),
                    _ => Err("field access requires struct".into()),
                }
            }
            Expr::Var(n) => {
                if n == "None" {
                    return Ok(Value::Enum { name: "Option".into(), variant: "None".into(), value: None });
                }
                self.lookup(n).ok_or_else(|| format!("undefined variable {}", n).into())
            },
            Expr::Array(a) => Ok(Value::Array(a.iter().map(|x| self.eval(x)).collect::<Result<_, _>>()?)),
            Expr::Map(entries) => {
                let mut map = HashMap::new();
                for (key_expr, value_expr) in entries {
                    let key = to_map_key(&self.eval(key_expr)?)
                        .ok_or_else(|| "map keys must be number, string, bool or null".to_string())?;
                    let value = self.eval(value_expr)?;
                    map.insert(key, value);
                }
                Ok(Value::Map(std::rc::Rc::new(std::cell::RefCell::new(map))))
            }
            Expr::Set(values) => {
                let mut set = std::collections::HashSet::new();
                for value_expr in values {
                    let key = to_map_key(&self.eval(value_expr)?)
                        .ok_or_else(|| "set values must be number, string, bool or null".to_string())?;
                    set.insert(key);
                }
                Ok(Value::Set(std::rc::Rc::new(std::cell::RefCell::new(set))))
            }
            Expr::Index(base, index) => {
                let base = self.eval(base)?;
                let index = self.eval(index)?;
                match base {
                    Value::Array(values) => {
                        let i = num(index)? as i64;
                        if i < 0 || i as usize >= values.len() { return Err("array index out of bounds".into()); }
                        Ok(values[i as usize].clone())
                    }
                    Value::Map(map) => {
                        let key = to_map_key(&index).ok_or_else(|| "map key must be number, string, bool or null".to_string())?;
                        Ok(map.borrow().get(&key).cloned().unwrap_or(Value::Null))
                    }
                    _ => Err("indexing requires an array or map".into()),
                }
            }
            Expr::Unary(op, x) => {
                let v = self.eval(x)?;
                match op {
                    Token::Minus => match v {
                        Value::Num(n) => Ok(Value::Num(-n)),
                        _ => Err("unary - expects number".into()),
                    },
                    Token::Bang => Ok(Value::Bool(!v.truth())),
                    _ => Err("bad unary".into()),
                }
            }
            Expr::Binary(a, op, b) => {
                let x = self.eval(a)?;
                if *op == Token::And && !x.truth() { return Ok(Value::Bool(false)); }
                if *op == Token::Or && x.truth() { return Ok(Value::Bool(true)); }
                let y = self.eval(b)?;
                self.bin(x, op, y)
            }
            Expr::Call(n, a) => {
                if let Some((enum_name, payload)) = self.enums.iter().find_map(|(enum_name, variants)| variants.get(n).map(|p| (enum_name.clone(), p.clone()))) {
                    if a.len() != if payload.is_some() { 1 } else { 0 } {
                        return Err(format!("{} expects {} arguments", n, if payload.is_some() { 1 } else { 0 }).into());
                    }
                    let value = if payload.is_some() { Some(Box::new(self.eval(&a[0])?)) } else { None };
                    return Ok(Value::Enum { name: enum_name, variant: n.clone(), value });
                }

                if n == "None" {
                    if !a.is_empty() { return Err("None expects 0 arguments".into()); }
                    return Ok(Value::Enum { name: "Option".into(), variant: "None".into(), value: None });
                }
                if n == "Some" {
                    if a.len() != 1 { return Err("Some expects 1 argument".into()); }
                    return Ok(Value::Enum { name: "Option".into(), variant: "Some".into(), value: Some(Box::new(self.eval(&a[0])?)) });
                }
                if n == "Ok" {
                    if a.len() != 1 { return Err("Ok expects 1 argument".into()); }
                    return Ok(Value::Enum { name: "Result".into(), variant: "Ok".into(), value: Some(Box::new(self.eval(&a[0])?)) });
                }
                if n == "Err" {
                    if a.len() != 1 { return Err("Err expects 1 argument".into()); }
                    return Ok(Value::Enum { name: "Result".into(), variant: "Err".into(), value: Some(Box::new(self.eval(&a[0])?)) });
                }
                if n == "iter" {
                    if a.len() != 1 { return Err("iter expects 1 argument".into()); }
                    let value = self.eval(&a[0])?;
                    let values = match value {
                        Value::Array(values) => values,
                        Value::Str(text) => text.chars().map(|ch| Value::Str(ch.to_string())).collect(),
                        Value::Iterator(iterator) => {
                            let mut state = iterator.borrow_mut();
                            let remaining = state.values[state.index..].to_vec();
                            state.index = state.values.len();
                            remaining
                        }
                        _ => return Err("iter expects an array, string or iterator".into()),
                    };
                    return Ok(Value::Iterator(std::rc::Rc::new(std::cell::RefCell::new(
                        crate::ast::IteratorState { values, index: 0 }
                    ))));
                }
                if n == "next" {
                    if a.len() != 1 { return Err("next expects 1 argument".into()); }
                    let value = self.eval(&a[0])?;
                    match value {
                        Value::Iterator(iterator) => {
                            let mut state = iterator.borrow_mut();
                            if state.index < state.values.len() {
                                let item = state.values[state.index].clone();
                                state.index += 1;
                                return Ok(Value::Enum {
                                    name: "Option".into(),
                                    variant: "Some".into(),
                                    value: Some(Box::new(item)),
                                });
                            }
                            return Ok(Value::Enum { name: "Option".into(), variant: "None".into(), value: None });
                        }
                        _ => return Err("next expects an Iterator".into()),
                    }
                }
                if n == "has_next" {
                    if a.len() != 1 { return Err("has_next expects 1 argument".into()); }
                    let value = self.eval(&a[0])?;
                    match value {
                        Value::Iterator(iterator) => {
                            let state = iterator.borrow();
                            return Ok(Value::Bool(state.index < state.values.len()));
                        }
                        _ => return Err("has_next expects an Iterator".into()),
                    }
                }
                if n == "collect" {
                    if a.len() != 1 { return Err("collect expects 1 argument".into()); }
                    let value = self.eval(&a[0])?;
                    match value {
                        Value::Iterator(iterator) => {
                            let mut state = iterator.borrow_mut();
                            let remaining = state.values[state.index..].to_vec();
                            state.index = state.values.len();
                            return Ok(Value::Array(remaining));
                        }
                        _ => return Err("collect expects an Iterator".into()),
                    }
                }
                if n == "range" {
                    if a.len() != 2 { return Err("range expects 2 arguments".into()); }
                    let x = self.eval(&a[0])?;
                    let y = self.eval(&a[1])?;
                    let (x, y) = (num(x)? as i64, num(y)? as i64);
                    return Ok(Value::Array((x..y).map(|n| Value::Num(n as f64)).collect()));
                }
                if n == "str" {
                    if a.len() != 1 { return Err("str expects 1 argument".into()); }
                    return Ok(Value::Str(self.eval(&a[0])?.to_string()));
                }
                if n == "len" {
                    if a.len() != 1 { return Err("len expects 1 argument".into()); }
                    let v = self.eval(&a[0])?;
                    return Ok(Value::Num(match v {
                        Value::Str(x) => x.chars().count() as f64,
                        Value::Array(x) => x.len() as f64,
                        Value::Map(x) => x.borrow().len() as f64,
                        Value::Set(x) => x.borrow().len() as f64,
                        _ => return Err("len expects string, array, map or set".into()),
                    }));
                }
                if n == "abs" {
                    if a.len() != 1 { return Err("abs expects 1 argument".into()); }
                    return Ok(Value::Num(num(self.eval(&a[0])?)?.abs()));
                }
                if n == "sqrt" {
                    if a.len() != 1 { return Err("sqrt expects 1 argument".into()); }
                    let v = num(self.eval(&a[0])?)?;
                    if v < 0.0 { return Err("sqrt expects a non-negative number".into()); }
                    return Ok(Value::Num(v.sqrt()));
                }
                if n == "read_file" {
                    if a.len() != 1 { return Err("read_file expects 1 argument".into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("read_file expects a string path".into()) };
                    return Ok(Value::Str(fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path, e))?));
                }
                if n == "write_file" {
                    if a.len() != 2 { return Err("write_file expects 2 arguments".into()); }
                    let path = self.eval(&a[0])?;
                    let data = self.eval(&a[1])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("write_file expects a string path".into()) };
                    let data = match data { Value::Str(x) => x, _ => return Err("write_file expects string data".into()) };
                    fs::write(&path, &data).map_err(|e| format!("cannot write {}: {}", path, e))?;
                    return Ok(Value::Null);
                }
                if n == "exists" {
                    if a.len() != 1 { return Err("exists expects 1 argument".into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("exists expects a string path".into()) };
                    return Ok(Value::Bool(std::path::Path::new(&path).exists()));
                }
                if n == "is_some" || n == "is_none" || n == "is_ok" || n == "is_err" {
                    if a.len() != 1 { return Err(format!("{} expects 1 argument", n).into()); }
                    let v = self.eval(&a[0])?;
                    let ok = match (&v, n.as_str()) {
                        (Value::Enum { variant, .. }, "is_some") => variant == "Some",
                        (Value::Enum { variant, .. }, "is_none") => variant == "None",
                        (Value::Enum { variant, .. }, "is_ok") => variant == "Ok",
                        (Value::Enum { variant, .. }, "is_err") => variant == "Err",
                        _ => false,
                    };
                    return Ok(Value::Bool(ok));
                }
                if n == "unwrap" {
                    if a.len() != 1 { return Err("unwrap expects 1 argument".into()); }
                    return match self.eval(&a[0])? {
                        Value::Enum { variant, value, .. } if variant == "Some" || variant == "Ok" => Ok(value.map(|v| *v).unwrap_or(Value::Null)),
                        Value::Enum { variant, .. } if variant == "None" || variant == "Err" => Err(format!("unwrap called on {}", variant).into()),
                        v => Ok(v),
                    };
                }
                if n == "unwrap_or" {
                    if a.len() != 2 { return Err("unwrap_or expects 2 arguments".into()); }
                    let v = self.eval(&a[0])?;
                    let fallback = self.eval(&a[1])?;
                    return match v {
                        Value::Enum { variant, value, .. } if variant == "Some" || variant == "Ok" => Ok(value.map(|v| *v).unwrap_or(fallback)),
                        Value::Enum { variant, .. } if variant == "None" || variant == "Err" => Ok(fallback),
                        v => Ok(v),
                    };
                }
                if n == "env" {
                    if a.len() != 1 { return Err("env expects 1 argument".into()); }
                    let key = self.eval(&a[0])?;
                    let key = match key { Value::Str(x) => x, _ => return Err("env expects a string key".into()) };
                    return Ok(std::env::var(&key).map(Value::Str).unwrap_or(Value::Null));
                }

                if n == "now_ms" || n == "now_s" {
                    if !a.is_empty() { return Err(format!("{} expects 0 arguments", n).into()); }
                    let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| format!("clock error: {}", e))?;
                    let value = if n == "now_ms" {
                        duration.as_secs_f64() * 1000.0
                    } else {
                        duration.as_secs_f64()
                    };
                    return Ok(Value::Num(value));
                }
                if n == "sleep_ms" {
                    if a.len() != 1 { return Err("sleep_ms expects 1 argument".into()); }
                    let ms = num(self.eval(&a[0])?)?;
                    if ms < 0.0 { return Err("sleep_ms expects a non-negative number".into()); }
                    std::thread::sleep(std::time::Duration::from_secs_f64(ms / 1000.0));
                    return Ok(Value::Null);
                }
                if n == "current_dir" {
                    if !a.is_empty() { return Err("current_dir expects 0 arguments".into()); }
                    return Ok(Value::Str(std::env::current_dir().map_err(|e| format!("cannot read current directory: {}", e))?.display().to_string()));
                }
                if n == "path_join" {
                    if a.len() != 2 { return Err("path_join expects 2 arguments".into()); }
                    let left = self.eval(&a[0])?;
                    let right = self.eval(&a[1])?;
                    let left = match left { Value::Str(x) => x, _ => return Err("path_join expects string paths".into()) };
                    let right = match right { Value::Str(x) => x, _ => return Err("path_join expects string paths".into()) };
                    return Ok(Value::Str(std::path::Path::new(&left).join(&right).display().to_string()));
                }
                if n == "path_basename" || n == "path_dirname" || n == "path_ext" || n == "path_stem" {
                    if a.len() != 1 { return Err(format!("{} expects 1 argument", n).into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err(format!("{} expects a string path", n).into()) };
                    let p = std::path::Path::new(&path);
                    let out = match n {
                        "path_basename" => p.file_name().and_then(|x| x.to_str()).map(str::to_owned).unwrap_or_default(),
                        "path_dirname" => p.parent().map(|x| x.display().to_string()).unwrap_or_default(),
                        "path_ext" => p.extension().and_then(|x| x.to_str()).map(str::to_owned).unwrap_or_default(),
                        "path_stem" => p.file_stem().and_then(|x| x.to_str()).map(str::to_owned).unwrap_or_default(),
                        _ => unreachable!(),
                    };
                    return Ok(Value::Str(out));
                }
                if n == "make_dir" {
                    if a.len() != 1 { return Err("make_dir expects 1 argument".into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("make_dir expects a string path".into()) };
                    std::fs::create_dir_all(&path).map_err(|e| format!("cannot create {}: {}", path, e))?;
                    return Ok(Value::Null);
                }
                if n == "remove_file" {
                    if a.len() != 1 { return Err("remove_file expects 1 argument".into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("remove_file expects a string path".into()) };
                    std::fs::remove_file(&path).map_err(|e| format!("cannot remove {}: {}", path, e))?;
                    return Ok(Value::Null);
                }
                if n == "list_dir" {
                    if a.len() != 1 { return Err("list_dir expects 1 argument".into()); }
                    let path = self.eval(&a[0])?;
                    let path = match path { Value::Str(x) => x, _ => return Err("list_dir expects a string path".into()) };
                    let mut entries = Vec::new();
                    for entry in std::fs::read_dir(&path).map_err(|e| format!("cannot list {}: {}", path, e))? {
                        let entry = entry.map_err(|e| format!("cannot read directory entry: {}", e))?;
                        entries.push(Value::Str(entry.file_name().to_string_lossy().into_owned()));
                    }
                    entries.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
                    return Ok(Value::Array(entries));
                }
                if n == "json_parse" {
                    if a.len() != 1 { return Err("json_parse expects 1 argument".into()); }
                    let source = self.eval(&a[0])?;
                    let source = match source { Value::Str(x) => x, _ => return Err("json_parse expects a string".into()) };
                    return Ok(parse_json(&source)?);
                }
                if n == "json_stringify" {
                    if a.len() != 1 { return Err("json_stringify expects 1 argument".into()); }
                    let value = self.eval(&a[0])?;
                    return Ok(Value::Str(stringify_json(&value)?));
                }

                if n == "map_get" || n == "map_has" || n == "map_set" || n == "map_remove" {
                    if (n == "map_get" || n == "map_has") && a.len() != 2 { return Err(format!("{} expects 2 arguments", n).into()); }
                    if (n == "map_set") && a.len() != 3 { return Err("map_set expects 3 arguments".into()); }
                    if (n == "map_remove") && a.len() != 2 { return Err("map_remove expects 2 arguments".into()); }
                    let map_value = self.eval(&a[0])?;
                    let key_value = self.eval(&a[1])?;
                    let key = to_map_key(&key_value).ok_or_else(|| "map key must be number, string, bool or null".to_string())?;
                    match map_value {
                        Value::Map(map) => {
                            if n == "map_get" {
                                return Ok(map.borrow().get(&key).cloned().unwrap_or(Value::Null));
                            }
                            if n == "map_has" {
                                return Ok(Value::Bool(map.borrow().contains_key(&key)));
                            }
                            if n == "map_remove" {
                                map.borrow_mut().remove(&key);
                                return Ok(Value::Null);
                            }
                            let value = self.eval(&a[2])?;
                            map.borrow_mut().insert(key, value);
                            return Ok(Value::Null);
                        }
                        _ => return Err("map_* expects a Map value".into()),
                    }
                }
                if n == "set_add" || n == "set_has" || n == "set_remove" {
                    if a.len() != 2 { return Err(format!("{} expects 2 arguments", n).into()); }
                    let set_value = self.eval(&a[0])?;
                    let item_value = self.eval(&a[1])?;
                    let item = to_map_key(&item_value).ok_or_else(|| "set values must be number, string, bool or null".to_string())?;
                    match set_value {
                        Value::Set(set) => {
                            if n == "set_add" { set.borrow_mut().insert(item); return Ok(Value::Null); }
                            if n == "set_has" { return Ok(Value::Bool(set.borrow().contains(&item))); }
                            set.borrow_mut().remove(&item);
                            return Ok(Value::Null);
                        }
                        _ => return Err("set_* expects a Set value".into()),
                    }
                }

                if let Some(callable) = self.lookup(n) {
                    if let Value::Closure { args, body, env } = callable {
                        if args.len() != a.len() {
                            return Err(format!("closure expects {} arguments", args.len()).into());
                        }
                        let vals = a.iter().map(|e| self.eval(e)).collect::<Result<Vec<_>, _>>()?;
                        let old_env = self.env.clone();
                        self.env = std::rc::Rc::new(std::cell::RefCell::new(EnvFrame {
                            values: HashMap::new(),
                            parent: Some(env.clone()),
                        }));
                        for (k, v) in args.iter().zip(vals) { self.define(k.clone(), v); }
                        let result = self.exec(&body);
                        self.env = old_env;
                        return match result {
                            Ok(Some(v)) => Ok(v),
                            Ok(None) => Ok(Value::Null),
                            Err(RuntimeError::Propagate(v)) => Ok(v),
                            Err(e) => Err(e),
                        };
                    }
                }

                if let Some(callable) = self.lookup(n) {
                    if let Value::Closure { args, body, env } = callable {
                        if args.len() != a.len() {
                            return Err(format!("closure expects {} arguments", args.len()).into());
                        }
                        let vals = a.iter().map(|e| self.eval(e)).collect::<Result<Vec<_>, _>>()?;
                        let caller_env = self.env.clone();
                        self.env = std::rc::Rc::new(std::cell::RefCell::new(EnvFrame {
                            values: HashMap::new(),
                            parent: Some(env.clone()),
                        }));
                        for (k, v) in args.iter().zip(vals) { self.define(k.clone(), v); }
                        let result = self.exec(&body);
                        self.env = caller_env;
                        return match result {
                            Ok(Some(v)) => Ok(v),
                            Ok(None) => Ok(Value::Null),
                            Err(RuntimeError::Propagate(v)) => Ok(v),
                            Err(e) => Err(e),
                        };
                    }
                }

                let f = self.fns.get(n).cloned().ok_or_else(|| format!("undefined function {}", n))?;
                if f.args.len() != a.len() { return Err(format!("{} expects {} arguments", n, f.args.len()).into()); }
                let caller_env = self.env.clone();
                let vals = a.iter().map(|e| self.eval(e)).collect::<Result<Vec<_>, _>>()?;
                self.env = std::rc::Rc::new(std::cell::RefCell::new(EnvFrame {
                    values: HashMap::new(),
                    parent: Some(caller_env.clone()),
                }));
                for (i, k) in f.args.iter().enumerate() { self.define(k.clone(), vals[i].clone()); }
                let result = self.exec(&f.body);
                self.env = caller_env;
                match result {
                    Ok(Some(v)) => Ok(v),
                    Ok(None) => Ok(Value::Null),
                    Err(RuntimeError::Propagate(v)) => Ok(v),
                    Err(e) => Err(e),
                }
            }
        }
    }

    fn matches_pattern(pattern: &Pattern, value: &Value, binding: &mut Option<Value>) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Literal(expr) => match expr {
                Expr::Val(v) => v.equals(value),
                _ => false,
            },
            Pattern::Enum { variant, .. } => match value {
                Value::Enum { variant: actual, value: payload, .. } if actual == variant => {
                    *binding = payload.as_deref().cloned();
                    true
                }
                _ => false,
            }
        }
    }

    fn bin(&self, a: Value, o: &Token, b: Value) -> Result<Value, RuntimeError> {
        match o {
            Token::Plus => match (a, b) {
                (Value::Num(x), Value::Num(y)) => Ok(Value::Num(x + y)),
                (Value::Str(x), Value::Str(y)) => Ok(Value::Str(x + &y)),
                _ => Err("unsupported +".into()),
            },
            Token::Minus => num2(a, b, |x, y| x - y),
            Token::Star => num2(a, b, |x, y| x * y),
            Token::Slash => div2(a, b),
            Token::Percent => mod2(a, b),
            Token::EqEq => Ok(Value::Bool(a.equals(&b))),
            Token::Ne => Ok(Value::Bool(!a.equals(&b))),
            Token::Lt => cmp2(a, b, |x, y| x < y),
            Token::Le => cmp2(a, b, |x, y| x <= y),
            Token::Gt => cmp2(a, b, |x, y| x > y),
            Token::Ge => cmp2(a, b, |x, y| x >= y),
            Token::And => Ok(Value::Bool(a.truth() && b.truth())),
            Token::Or => Ok(Value::Bool(a.truth() || b.truth())),
            _ => Err("bad operator".into()),
        }
    }

    pub fn exec(&mut self, s: &[Stmt]) -> Result<Option<Value>, RuntimeError> {
        for x in s {
            match x {
                Stmt::Expr(e) => { self.eval(e)?; }
                Stmt::Let(n, _, e) => {
                    let v = self.eval(e)?;
                    self.define(n.clone(), v);
                }
                Stmt::Assign(n, e) => {
                    let v = self.eval(e)?;
                    self.assign(n.clone(), v);
                }
                Stmt::Print(e) => println!("{}", self.eval(e)?),
                Stmt::Return(e) => return Ok(Some(self.eval(e)?)),
                Stmt::If(c, a, b) => {
                    if self.eval(c)?.truth() {
                        if let Some(v) = self.exec(a)? { return Ok(Some(v)); }
                    } else if let Some(v) = self.exec(b)? { return Ok(Some(v)); }
                }
                Stmt::While(c, b) => {
                    while self.eval(c)?.truth() {
                        if let Some(v) = self.exec(b)? { return Ok(Some(v)); }
                    }
                }
                Stmt::For(n, it, b) => {
                    let v = self.eval(it)?;
                    match v {
                        Value::Array(xs) => {
                            for x in xs {
                                self.define(n.clone(), x);
                                if let Some(v) = self.exec(b)? { return Ok(Some(v)); }
                            }
                        }
                        Value::Iterator(iterator) => {
                            loop {
                                let item = {
                                    let mut state = iterator.borrow_mut();
                                    if state.index >= state.values.len() {
                                        None
                                    } else {
                                        let item = state.values[state.index].clone();
                                        state.index += 1;
                                        Some(item)
                                    }
                                };
                                match item {
                                    Some(x) => {
                                        self.define(n.clone(), x);
                                        if let Some(v) = self.exec(b)? { return Ok(Some(v)); }
                                    }
                                    None => break,
                                }
                            }
                        }
                        _ => return Err("for expects an array or Iterator".into()),
                    }
                }
                Stmt::Match(value, arms, otherwise) => {
                    let v = self.eval(value)?;
                    let mut done = false;
                    for (pattern, body) in arms {
                        let mut binding = None;
                        if Self::matches_pattern(pattern, &v, &mut binding) {
                            if let Pattern::Enum { binding: Some(name), .. } = pattern {
                                if let Some(value) = binding.take() { self.define(name.clone(), value); }
                            }
                            if let Some(r) = self.exec(body)? { return Ok(Some(r)); }
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        if let Some(r) = self.exec(otherwise)? { return Ok(Some(r)); }
                    }
                }
                Stmt::Import(path) => {
                    let resolved = {
                        let p = std::path::Path::new(path);
                        if p.is_absolute() { p.to_path_buf() }
                        else if let Some(base) = self.module_stack.last() { base.join(p) }
                        else { p.to_path_buf() }
                    };
                    let key = resolved.to_string_lossy().to_string();
                    if !self.modules.contains_key(&key) {
                        let src = fs::read_to_string(&resolved).map_err(|e| format!("cannot import {}: {}", resolved.display(), e))?;
                        let toks = lex(&src)?;
                        let mut p = Parser::new(toks);
                        let program = p.program()?;
                        self.modules.insert(key.clone(), true);
                        let parent = resolved.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
                        self.module_stack.push(parent);
                        let r = self.exec(&program);
                        self.module_stack.pop();
                        r?;
                    }
                }
                Stmt::Fn(n, _, a, _, b) => {
                    self.fns.insert(n.clone(), Function { args: a.iter().map(|x| x.0.clone()).collect(), body: b.clone() });
                }
                Stmt::StructDecl(_, _) => {}
                Stmt::EnumDecl(name, variants) => {
                    let table = self.enums.entry(name.clone()).or_default();
                    for (variant, payload) in variants {
                        table.insert(variant.clone(), payload.clone());
                    }
                }
            }
        }
        Ok(None)
    }
}

fn num(v: Value) -> Result<f64, RuntimeError> {
    match v { Value::Num(n) => Ok(n), _ => Err("number expected".into()) }
}
fn num2(a: Value, b: Value, f: fn(f64,f64)->f64) -> Result<Value, RuntimeError> {
    Ok(Value::Num(f(num(a)?, num(b)?)))
}
fn cmp2(a: Value, b: Value, f: fn(f64,f64)->bool) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(f(num(a)?, num(b)?)))
}
fn div2(a: Value, b: Value) -> Result<Value, RuntimeError> {
    let x = num(a)?; let y = num(b)?;
    if y == 0.0 { return Err("division by zero".into()); }
    Ok(Value::Num(x / y))
}
fn mod2(a: Value, b: Value) -> Result<Value, RuntimeError> {
    let x = num(a)?; let y = num(b)?;
    if y == 0.0 { return Err("modulo by zero".into()); }
    Ok(Value::Num(x % y))
}
