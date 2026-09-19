use crate::ir::{IrType, SsaFunction, SsaInstr, SsaValue, Terminator, ValueId};

fn c_ident(name: &str) -> String {
    if name == "<main>" {
        return "nova_main".into();
    }
    let mut out = String::from("nova_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn v(id: ValueId) -> String {
    format!("v{}", id)
}

fn c_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for byte in value.bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            0x0a => out.push_str("\\n"),
            0x0d => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{:02x}", byte)),
        }
    }
    out.push('"');
    out
}
fn binary_expr(op: &str, left: &str, right: &str) -> Option<String> {
    Some(match op {
        "Plus" => format!("nova_add({}, {})", left, right),
        "Minus" => format!("nova_num({}.number - {}.number)", left, right),
        "Star" => format!("nova_num({}.number * {}.number)", left, right),
        "Slash" => format!("nova_num({}.number / {}.number)", left, right),
        "Percent" => format!("nova_num(fmod({}.number, {}.number))", left, right),
        "EqEq" => format!("nova_bool(nova_equal({}, {}))", left, right),
        "NotEq" => format!("nova_bool(!nova_equal({}, {}))", left, right),
        "Lt" => format!("nova_bool({}.number < {}.number)", left, right),
        "Le" => format!("nova_bool({}.number <= {}.number)", left, right),
        "Gt" => format!("nova_bool({}.number > {}.number)", left, right),
        "Ge" => format!("nova_bool({}.number >= {}.number)", left, right),
        "And" => format!("nova_bool({}.number != 0 && {}.number != 0)", left, right),
        "Or" => format!("nova_bool({}.number != 0 || {}.number != 0)", left, right),
        _ => return None,
    })
}

fn slot_ident(name: &str) -> String {
    let mut out = String::from("nova_slot_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn state_names(function: &SsaFunction) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    for block in &function.blocks {
        for (_, instr) in &block.instrs {
            match instr {
                SsaInstr::Load { name } => {
                    names.insert(name.clone());
                }
                SsaInstr::Store { name, .. } => {
                    names.insert(name.clone());
                }
                _ => {}
            }
        }
    }
    names
}

fn max_value_id(function: &SsaFunction) -> ValueId {
    let mut max = 0;
    for (_, _, id) in &function.params {
        max = max.max(*id);
    }
    for (_, _, id) in &function.captures {
        max = max.max(*id);
    }
    for block in &function.blocks {
        for (id, _) in &block.instrs {
            max = max.max(*id);
        }
        for id in &block.params {
            max = max.max(*id);
        }
    }
    max
}

fn emit_function(
    function: &SsaFunction,
    functions: &std::collections::HashSet<String>,
) -> Result<String, String> {
    let mut out = String::new();
    let max = max_value_id(function);

    out.push_str(&format!(
        "static NovaValue {}(NovaEnv* env, NovaValue* args, size_t argc)",
        c_ident(&function.name)
    ));
    out.push_str(" {\n");
    out.push_str("  jmp_buf nova_jmp;\n");
    out.push_str("  int nova_pred = -1;\n");
    out.push_str("  jmp_buf* nova_prev_jmp = nova_active_jmp;\n");
    out.push_str("  nova_active_jmp = &nova_jmp;\n");
    out.push_str("  int nova_jmp_code = setjmp(nova_jmp);\n");
    out.push_str("  if (nova_jmp_code != 0) {\n");
    out.push_str("    NovaValue nova_result = nova_pending_return;\n");
    out.push_str("    nova_active_jmp = nova_prev_jmp;\n");
    out.push_str("    return nova_result;\n");
    out.push_str("  }\n");

    for id in 0..=max {
        out.push_str(&format!("  NovaValue {} = nova_null();\n", v(id)));
    }

    let state = state_names(function);
    let captures: std::collections::BTreeSet<String> = function
        .captures
        .iter()
        .map(|(name, _, _)| name.clone())
        .collect();
    for name in state.iter().filter(|name| !captures.contains(*name)) {
        out.push_str(&format!(
            "  NovaValue {} = nova_null();\n",
            slot_ident(name)
        ));
    }

    for (slot, (_, _, id)) in function.captures.iter().enumerate() {
        out.push_str(&format!(
            "  {} = env ? env->slots[{}] : nova_null();\n",
            v(*id),
            slot
        ));
    }

    for (index, (_, _, id)) in function.params.iter().enumerate() {
        out.push_str(&format!(
            "  {} = (argc > {}) ? args[{}] : nova_null();\n",
            v(*id),
            index,
            index
        ));
    }

    for block in &function.blocks {
        out.push_str(&format!("B{}:\n", block.id));

        for (id, instr) in &block.instrs {
            match instr {
                SsaInstr::Const(SsaValue::Number(n)) => {
                    out.push_str(&format!("  {} = nova_num({:.17});\n", v(*id), n));
                }
                SsaInstr::Const(SsaValue::Bool(b)) => {
                    out.push_str(&format!(
                        "  {} = nova_bool({});\n",
                        v(*id),
                        if *b { "1" } else { "0" }
                    ));
                }
                SsaInstr::Const(SsaValue::Null) => {
                    out.push_str(&format!("  {} = nova_null();\n", v(*id)));
                }
                SsaInstr::Const(SsaValue::String(s)) => {
                    out.push_str(&format!(
                        "  {} = nova_string({});\n",
                        v(*id),
                        c_string(s)
                    ));
                }
                SsaInstr::Const(SsaValue::Struct { name }) => {
                    out.push_str(&format!(
                        "  {} = nova_make_empty_struct({});\n",
                        v(*id),
                        c_string(name)
                    ));
                }
                SsaInstr::Const(SsaValue::Param(_)) | SsaInstr::Const(SsaValue::Instr(_)) => {
                    return Err(format!(
                        "SSA C backend: unsupported symbolic constant in {}",
                        function.name
                    ));
                }
                SsaInstr::Load { name } => {
                    if let Some(slot) = function
                        .captures
                        .iter()
                        .position(|(capture, _, _)| capture == name)
                    {
                        out.push_str(&format!(
                            "  {} = env ? env->slots[{}] : nova_null();\n",
                            v(*id),
                            slot
                        ));
                    } else {
                        out.push_str(&format!(
                            "  {} = {};\n",
                            v(*id),
                            slot_ident(name)
                        ));
                    }
                }
                SsaInstr::Store { name, value } => {
                    if let Some(slot) = function
                        .captures
                        .iter()
                        .position(|(capture, _, _)| capture == name)
                    {
                        out.push_str(&format!(
                            "  if (env) env->slots[{}] = {};\n",
                            slot,
                            v(*value)
                        ));
                    } else {
                        out.push_str(&format!(
                            "  {} = {};\n",
                            slot_ident(name),
                            v(*value)
                        ));
                    }
                }
                SsaInstr::Unary { op, value, .. } => match op.as_str() {
                    "Minus" => out.push_str(&format!(
                        "  {} = nova_num(-{}.number);\n",
                        v(*id),
                        v(*value)
                    )),
                    "Bang" => out.push_str(&format!(
                        "  {} = nova_bool(!nova_truthy({}));\n",
                        v(*id),
                        v(*value)
                    )),
                    other => {
                        return Err(format!(
                            "SSA C backend: unsupported unary {}",
                            other
                        ));
                    }
                },
                SsaInstr::Binary { op, left, right, .. } => {
                    let Some(expr) = binary_expr(op, &v(*left), &v(*right)) else {
                        return Err(format!(
                            "SSA C backend: unsupported binary {}",
                            op
                        ));
                    };
                    out.push_str(&format!("  {} = {};\n", v(*id), expr));
                }
                SsaInstr::Call {
                    name,
                    args,
                    result,
                } => {
                    if name == "print" {
                        if args.len() != 1 {
                            return Err("SSA C backend: print expects one argument".into());
                        }
                        out.push_str(&format!("  nova_print({});\n", v(args[0])));
                        out.push_str(&format!("  {} = nova_null();\n", v(*id)));
                    } else if matches!(name.as_str(), "array" | "map" | "set") {
                        let args_expr = if args.is_empty() {
                            "NULL".to_string()
                        } else {
                            format!("call_args_{}", id)
                        };
                        if !args.is_empty() {
                            out.push_str(&format!(
                                "  NovaValue call_args_{}[{}] = {{ {} }};\n",
                                id,
                                args.len(),
                                args.iter().map(|a| v(*a)).collect::<Vec<_>>().join(", ")
                            ));
                        }
                        let helper = match name.as_str() {
                            "array" => "nova_array",
                            "map" => "nova_map",
                            _ => "nova_set",
                        };
                        out.push_str(&format!(
                            "  {} = {}({}, {});\n",
                            v(*id),
                            helper,
                            args_expr,
                            args.len()
                        ));
                    } else if name == "index" {
                        if args.len() != 2 {
                            return Err("SSA C backend: index expects two arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_index({}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1])
                        ));
                    } else if name == "index_set" {
                        if args.len() != 3 {
                            return Err("SSA C backend: index_set expects three arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_index_set({}, {}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1]),
                            v(args[2])
                        ));
                    } else if name == "field_set" {
                        if args.len() != 3 {
                            return Err("SSA C backend: field_set expects three arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_field_set({}, {}.string, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1]),
                            v(args[2])
                        ));
                    } else if matches!(name.as_str(), "len" | "unwrap" | "unwrap_or" | "is_none" | "is_some" | "is_ok" | "is_err") {
                        let expected = if name == "unwrap_or" { 2 } else { 1 };
                        if args.len() != expected {
                            return Err(format!("SSA C backend: {} expects {} arguments", name, expected));
                        }
                        let helper = match name.as_str() {
                            "len" => "nova_len",
                            "unwrap" => "nova_unwrap",
                            "unwrap_or" => "nova_unwrap_or",
                            "is_none" => "nova_is_none",
                            "is_some" => "nova_is_some",
                            "is_ok" => "nova_is_ok",
                            _ => "nova_is_err",
                        };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            args.iter().map(|a| v(*a)).collect::<Vec<_>>().join(", ")
                        ));
                    } else if functions.contains(name) {
                        let cname = c_ident(name);
                        if args.is_empty() {
                            out.push_str(&format!(
                                "  {} = {}(NULL, NULL, 0);\n",
                                v(*id),
                                cname
                            ));
                        } else {
                            out.push_str(&format!(
                                "  NovaValue call_args_{}[{}] = {{ {} }};\n",
                                id,
                                args.len(),
                                args.iter().map(|a| v(*a)).collect::<Vec<_>>().join(", ")
                            ));
                            out.push_str(&format!(
                                "  {} = {}(NULL, call_args_{}, {});\n",
                                v(*id),
                                cname,
                                id,
                                args.len()
                            ));
                        }
                    } else {
                        return Err(format!(
                            "SSA C backend: unknown or unsupported call {}",
                            name
                        ));
                    }
                    if matches!(result, IrType::Null) && name != "print" {
                        out.push_str(&format!("  {} = nova_null();\n", v(*id)));
                    }
                }
                SsaInstr::CallIndirect { callee, args, .. } => {
                    if args.is_empty() {
                        out.push_str(&format!(
                            "  {} = nova_call_closure({}, NULL, 0);\n",
                            v(*id),
                            v(*callee)
                        ));
                    } else {
                        out.push_str(&format!(
                            "  NovaValue indirect_args_{}[{}] = {{ {} }};\n",
                            id,
                            args.len(),
                            args.iter().map(|a| v(*a)).collect::<Vec<_>>().join(", ")
                        ));
                        out.push_str(&format!(
                            "  {} = nova_call_closure({}, indirect_args_{}, {});\n",
                            v(*id),
                            v(*callee),
                            id,
                            args.len()
                        ));
                    }
                }
                SsaInstr::StructInit { name, fields } => {
                    out.push_str(&format!(
                        "  NovaStruct* struct_{} = nova_make_struct({}, {});\n",
                        id,
                        c_string(name),
                        fields.len()
                    ));
                    for (slot, (field, value)) in fields.iter().enumerate() {
                        out.push_str(&format!(
                            "  nova_struct_set(struct_{}, {}, {}, {});\n",
                            id,
                            slot,
                            c_string(field),
                            v(*value)
                        ));
                    }
                    out.push_str(&format!(
                        "  {} = nova_struct_value(struct_{});\n",
                        v(*id),
                        id
                    ));
                }
                SsaInstr::FieldGet { base, field, .. } => {
                    out.push_str(&format!(
                        "  {} = nova_field_get({}, {});\n",
                        v(*id),
                        v(*base),
                        c_string(field)
                    ));
                }
                SsaInstr::EnumInit {
                    name,
                    variant,
                    payload,
                    ..
                } => {
                    let payload_expr = payload
                        .map(|value| v(value))
                        .unwrap_or_else(|| "nova_null()".into());
                    out.push_str(&format!(
                        "  {} = nova_enum_value({}, {}, {});\n",
                        v(*id),
                        c_string(name),
                        c_string(variant),
                        payload_expr
                    ));
                }
                SsaInstr::EnumTest { value, variant } => {
                    out.push_str(&format!(
                        "  {} = nova_bool(nova_enum_is({}, {}));\n",
                        v(*id),
                        v(*value),
                        c_string(variant)
                    ));
                }
                SsaInstr::EnumPayload { value, .. } => {
                    out.push_str(&format!(
                        "  {} = nova_enum_payload({});\n",
                        v(*id),
                        v(*value)
                    ));
                }
                SsaInstr::Try { value, .. } => {
                    out.push_str(&format!(
                        "  {} = nova_try({});\n",
                        v(*id),
                        v(*value)
                    ));
                }
                SsaInstr::Closure {
                    function: closure_function,
                    captures,
                    ..
                } => {
                    out.push_str(&format!(
                        "  NovaClosure* closure_{} = nova_make_closure({}, {});\n",
                        id,
                        c_ident(closure_function),
                        captures.len()
                    ));
                    for (slot, (_, value)) in captures.iter().enumerate() {
                        out.push_str(&format!(
                            "  if (closure_{}) closure_{}->env->slots[{}] = {};\n",
                            id,
                            id,
                            slot,
                            v(*value)
                        ));
                    }
                    out.push_str(&format!(
                        "  {} = nova_closure_value(closure_{});\n",
                        v(*id),
                        id
                    ));
                }
                SsaInstr::Phi { incomings, .. } => {
                    if incomings.is_empty() {
                        return Err(format!("SSA C backend: empty Phi in {}", function.name));
                    }
                    for (index, (pred, value)) in incomings.iter().enumerate() {
                        if index == 0 {
                            out.push_str(&format!(
                                "  if (nova_pred == {}) {} = {};\n",
                                pred,
                                v(*id),
                                v(*value)
                            ));
                        } else {
                            out.push_str(&format!(
                                "  else if (nova_pred == {}) {} = {};\n",
                                pred,
                                v(*id),
                                v(*value)
                            ));
                        }
                    }
                }
            }
        }

        match &block.terminator {
            Some(Terminator::Jump(target)) => {
                out.push_str(&format!("  nova_pred = {};\n", block.id));
                out.push_str(&format!("  goto B{};\n", target));
            }
            Some(Terminator::Branch {
                condition,
                then_block,
                else_block,
            }) => {
                out.push_str(&format!(
                    "  if (nova_truthy({})) {{ nova_pred = {}; goto B{}; }} else {{ nova_pred = {}; goto B{}; }}\n",
                    v(*condition),
                    block.id,
                    then_block,
                    block.id,
                    else_block
                ));
            }
            Some(Terminator::Return(Some(value))) => {
                out.push_str("  nova_active_jmp = nova_prev_jmp;\n");
                out.push_str(&format!("  return {};\n", v(*value)));
            }
            Some(Terminator::Return(None)) => {
                out.push_str("  nova_active_jmp = nova_prev_jmp;\n");
                out.push_str("  return nova_null();\n");
            }
            None => {
                return Err(format!(
                    "SSA C backend: block {} in {} has no terminator",
                    block.id, function.name
                ));
            }
        }
    }

    out.push_str("}\n\n");
    Ok(out)
}

pub fn emit_c(functions: &[SsaFunction]) -> Result<String, String> {
    let names = functions
        .iter()
        .map(|f| f.name.clone())
        .collect::<std::collections::HashSet<_>>();

    let runtime = r#"
#include <math.h>
#include <setjmp.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef enum {
  NOVA_NULL,
  NOVA_NUMBER,
  NOVA_BOOL,
  NOVA_STRING,
  NOVA_STRUCT,
  NOVA_ENUM,
  NOVA_CLOSURE,
  NOVA_ARRAY,
  NOVA_MAP,
  NOVA_SET
} NovaTag;

typedef struct NovaValue NovaValue;
typedef struct NovaClosure NovaClosure;
typedef struct NovaEnv NovaEnv;
typedef struct NovaStruct NovaStruct;
typedef struct NovaEnum NovaEnum;
typedef struct NovaArray NovaArray;
typedef struct NovaMap NovaMap;
typedef struct NovaSet NovaSet;

struct NovaValue {
  NovaTag tag;
  double number;
  const char* string;
  NovaClosure* closure;
  NovaStruct* structure;
  NovaEnum* enumeration;
  NovaArray* array;
  NovaMap* map;
  NovaSet* set;
};

typedef struct {
  const char* name;
  NovaValue value;
} NovaField;

struct NovaStruct {
  char* name;
  size_t len;
  NovaField* fields;
};

struct NovaEnum {
  char* name;
  char* variant;
  NovaValue payload;
};

struct NovaArray {
  size_t len;
  NovaValue* items;
};

struct NovaMap {
  size_t len;
  NovaValue* keys;
  NovaValue* values;
};

struct NovaSet {
  size_t len;
  NovaValue* items;
};

struct NovaEnv {
  size_t len;
  NovaValue slots[32];
};

struct NovaClosure {
  NovaEnv* env;
  NovaValue (*invoke)(NovaEnv*, NovaValue*, size_t);
};

static jmp_buf* nova_active_jmp = NULL;
static NovaValue nova_pending_return;

static char* nova_dup(const char* value) {
  size_t len = strlen(value) + 1;
  char* out = (char*)malloc(len);
  if (!out) return NULL;
  memcpy(out, value, len);
  return out;
}

static NovaValue nova_null(void) {
  NovaValue v = { NOVA_NULL, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_num(double x) {
  NovaValue v = { NOVA_NUMBER, x, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_bool(int x) {
  NovaValue v = { NOVA_BOOL, x ? 1.0 : 0.0, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_string(const char* x) {
  NovaValue v = { NOVA_STRING, 0, x, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_struct_value(NovaStruct* x) {
  NovaValue v = { NOVA_STRUCT, 0, NULL, NULL, x, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_enum_value(const char* name, const char* variant, NovaValue payload) {
  NovaEnum* e = (NovaEnum*)calloc(1, sizeof(NovaEnum));
  if (!e) return nova_null();
  e->name = nova_dup(name);
  e->variant = nova_dup(variant);
  e->payload = payload;
  NovaValue v = { NOVA_ENUM, 0, NULL, NULL, NULL, e, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_closure_value(NovaClosure* c) {
  NovaValue v = { NOVA_CLOSURE, 0, NULL, c, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static int nova_equal(NovaValue a, NovaValue b);

static NovaValue nova_propagate(NovaValue value) {
  nova_pending_return = value;
  if (nova_active_jmp) longjmp(*nova_active_jmp, 1);
  fprintf(stderr, "NOVA: uncaught Option/Result error during native execution\n");
  exit(1);
}

static NovaValue nova_try(NovaValue value) {
  if (value.tag != NOVA_ENUM || !value.enumeration) return value;
  if (strcmp(value.enumeration->name, "Option") == 0) {
    if (strcmp(value.enumeration->variant, "Some") == 0) return value.enumeration->payload;
    return nova_propagate(value);
  }
  if (strcmp(value.enumeration->name, "Result") == 0) {
    if (strcmp(value.enumeration->variant, "Ok") == 0) return value.enumeration->payload;
    return nova_propagate(value);
  }
  return value;
}

static NovaValue nova_unwrap(NovaValue value) {
  if (value.tag == NOVA_ENUM && value.enumeration &&
      ((strcmp(value.enumeration->name, "Option") == 0 && strcmp(value.enumeration->variant, "Some") == 0) ||
       (strcmp(value.enumeration->name, "Result") == 0 && strcmp(value.enumeration->variant, "Ok") == 0))) {
    return value.enumeration->payload;
  }
  return nova_propagate(value);
}

static NovaValue nova_unwrap_or(NovaValue value, NovaValue fallback) {
  if (value.tag == NOVA_ENUM && value.enumeration &&
      ((strcmp(value.enumeration->name, "Option") == 0 && strcmp(value.enumeration->variant, "Some") == 0) ||
       (strcmp(value.enumeration->name, "Result") == 0 && strcmp(value.enumeration->variant, "Ok") == 0))) {
    return value.enumeration->payload;
  }
  return fallback;
}

static NovaValue nova_is_none(NovaValue value) {
  return nova_bool(value.tag == NOVA_ENUM && value.enumeration &&
      strcmp(value.enumeration->name, "Option") == 0 &&
      strcmp(value.enumeration->variant, "None") == 0);
}

static NovaValue nova_is_some(NovaValue value) {
  return nova_bool(value.tag == NOVA_ENUM && value.enumeration &&
      strcmp(value.enumeration->name, "Option") == 0 &&
      strcmp(value.enumeration->variant, "Some") == 0);
}

static NovaValue nova_is_ok(NovaValue value) {
  return nova_bool(value.tag == NOVA_ENUM && value.enumeration &&
      strcmp(value.enumeration->name, "Result") == 0 &&
      strcmp(value.enumeration->variant, "Ok") == 0);
}

static NovaValue nova_is_err(NovaValue value) {
  return nova_bool(value.tag == NOVA_ENUM && value.enumeration &&
      strcmp(value.enumeration->name, "Result") == 0 &&
      strcmp(value.enumeration->variant, "Err") == 0);
}

static NovaValue nova_len(NovaValue value) {
  switch (value.tag) {
    case NOVA_STRING: return nova_num(value.string ? (double)strlen(value.string) : 0.0);
    case NOVA_ARRAY: return nova_num(value.array ? (double)value.array->len : 0.0);
    case NOVA_MAP: return nova_num(value.map ? (double)value.map->len : 0.0);
    case NOVA_SET: return nova_num(value.set ? (double)value.set->len : 0.0);
    default: return nova_num(0.0);
  }
}

static NovaValue nova_add(NovaValue left, NovaValue right) {
  if (left.tag == NOVA_NUMBER && right.tag == NOVA_NUMBER) return nova_num(left.number + right.number);
  if (left.tag == NOVA_STRING && right.tag == NOVA_STRING) {
    const char* a = left.string ? left.string : "";
    const char* b = right.string ? right.string : "";
    size_t a_len = strlen(a);
    size_t b_len = strlen(b);
    char* joined = (char*)malloc(a_len + b_len + 1);
    if (!joined) return nova_null();
    memcpy(joined, a, a_len);
    memcpy(joined + a_len, b, b_len + 1);
    return nova_string(joined);
  }
  return nova_null();
}

static NovaValue nova_array_value(NovaArray* a) {
  NovaValue v = { NOVA_ARRAY, 0, NULL, NULL, NULL, NULL, a, NULL, NULL };
  return v;
}

static NovaValue nova_map_value(NovaMap* m) {
  NovaValue v = { NOVA_MAP, 0, NULL, NULL, NULL, NULL, NULL, m, NULL };
  return v;
}

static NovaValue nova_set_value(NovaSet* s) {
  NovaValue v = { NOVA_SET, 0, NULL, NULL, NULL, NULL, NULL, NULL, s };
  return v;
}

static NovaValue nova_array(NovaValue* args, size_t argc) {
  NovaArray* a = (NovaArray*)calloc(1, sizeof(NovaArray));
  if (!a) return nova_null();
  a->len = argc;
  a->items = argc ? (NovaValue*)calloc(argc, sizeof(NovaValue)) : NULL;
  if (argc && !a->items) return nova_null();
  for (size_t i = 0; i < argc; i++) a->items[i] = args[i];
  return nova_array_value(a);
}

static NovaValue nova_map(NovaValue* args, size_t argc) {
  if (argc % 2 != 0) return nova_null();
  NovaMap* m = (NovaMap*)calloc(1, sizeof(NovaMap));
  if (!m) return nova_null();
  m->len = argc / 2;
  m->keys = m->len ? (NovaValue*)calloc(m->len, sizeof(NovaValue)) : NULL;
  m->values = m->len ? (NovaValue*)calloc(m->len, sizeof(NovaValue)) : NULL;
  if (m->len && (!m->keys || !m->values)) return nova_null();
  for (size_t i = 0; i < m->len; i++) {
    m->keys[i] = args[i * 2];
    m->values[i] = args[i * 2 + 1];
  }
  return nova_map_value(m);
}

static NovaValue nova_set(NovaValue* args, size_t argc) {
  NovaSet* set = (NovaSet*)calloc(1, sizeof(NovaSet));
  if (!set) return nova_null();
  set->items = argc ? (NovaValue*)calloc(argc, sizeof(NovaValue)) : NULL;
  if (argc && !set->items) return nova_null();
  for (size_t i = 0; i < argc; i++) {
    int duplicate = 0;
    for (size_t j = 0; j < set->len; j++) {
      if (nova_equal(set->items[j], args[i])) { duplicate = 1; break; }
    }
    if (!duplicate) set->items[set->len++] = args[i];
  }
  return nova_set_value(set);
}

static NovaValue nova_index(NovaValue base, NovaValue index) {
  if (base.tag == NOVA_ARRAY && base.array && index.tag == NOVA_NUMBER) {
    size_t i = (size_t)index.number;
    if (index.number >= 0 && (double)i == index.number && i < base.array->len)
      return base.array->items[i];
  }
  if (base.tag == NOVA_MAP && base.map) {
    for (size_t i = 0; i < base.map->len; i++) {
      if (nova_equal(base.map->keys[i], index)) return base.map->values[i];
    }
  }
  return nova_null();
}

static NovaValue nova_index_set(NovaValue base, NovaValue index, NovaValue value) {
  if (base.tag == NOVA_ARRAY && base.array && index.tag == NOVA_NUMBER) {
    size_t i = (size_t)index.number;
    if (index.number >= 0 && (double)i == index.number && i < base.array->len) {
      base.array->items[i] = value;
      return base;
    }
  }
  if (base.tag == NOVA_MAP && base.map) {
    for (size_t i = 0; i < base.map->len; i++) {
      if (nova_equal(base.map->keys[i], index)) {
        base.map->values[i] = value;
        return base;
      }
    }
    size_t next = base.map->len + 1;
    NovaValue* keys = (NovaValue*)realloc(base.map->keys, next * sizeof(NovaValue));
    NovaValue* values = (NovaValue*)realloc(base.map->values, next * sizeof(NovaValue));
    if (!keys || !values) return nova_null();
    base.map->keys = keys;
    base.map->values = values;
    base.map->keys[base.map->len] = index;
    base.map->values[base.map->len] = value;
    base.map->len = next;
    return base;
  }
  return nova_null();
}

static int nova_truthy(NovaValue v) {
  switch (v.tag) {
    case NOVA_BOOL: return v.number != 0;
    case NOVA_NUMBER: return v.number != 0;
    case NOVA_STRING: return v.string && v.string[0] != '\0';
    case NOVA_STRUCT: return 1;
    case NOVA_ENUM: return 1;
    case NOVA_CLOSURE: return 1;
    case NOVA_ARRAY: return 1;
    case NOVA_MAP: return 1;
    case NOVA_SET: return 1;
    default: return 0;
  }
}

static int nova_equal(NovaValue a, NovaValue b) {
  if (a.tag != b.tag) return 0;
  switch (a.tag) {
    case NOVA_NULL: return 1;
    case NOVA_NUMBER:
    case NOVA_BOOL: return a.number == b.number;
    case NOVA_STRING: return strcmp(a.string ? a.string : "", b.string ? b.string : "") == 0;
    case NOVA_ENUM:
      return strcmp(a.enumeration->name, b.enumeration->name) == 0
          && strcmp(a.enumeration->variant, b.enumeration->variant) == 0;
    case NOVA_STRUCT:
      return a.structure == b.structure;
    case NOVA_CLOSURE:
      return a.closure == b.closure;
    case NOVA_ARRAY:
      return a.array == b.array;
    case NOVA_MAP:
      return a.map == b.map;
    case NOVA_SET:
      return a.set == b.set;
    default: return 0;
  }
}

static NovaStruct* nova_make_empty_struct(const char* name) {
  NovaStruct* s = (NovaStruct*)calloc(1, sizeof(NovaStruct));
  if (!s) return NULL;
  s->name = nova_dup(name);
  return s;
}

static NovaStruct* nova_make_struct(const char* name, size_t len) {
  NovaStruct* s = nova_make_empty_struct(name);
  if (!s) return NULL;
  s->len = len;
  s->fields = (NovaField*)calloc(len, sizeof(NovaField));
  if (!s->fields) return NULL;
  return s;
}

static void nova_struct_set(NovaStruct* s, size_t slot, const char* name, NovaValue value) {
  if (!s || slot >= s->len) return;
  s->fields[slot].name = name;
  s->fields[slot].value = value;
}

static NovaValue nova_field_get(NovaValue base, const char* field) {
  if (base.tag != NOVA_STRUCT || !base.structure) return nova_null();
  for (size_t i = 0; i < base.structure->len; i++) {
    if (base.structure->fields[i].name
        && strcmp(base.structure->fields[i].name, field) == 0) {
      return base.structure->fields[i].value;
    }
  }
  return nova_null();
}

static NovaValue nova_field_set(NovaValue base, const char* field, NovaValue value) {
  if (base.tag != NOVA_STRUCT || !base.structure || !field) return nova_null();
  for (size_t i = 0; i < base.structure->len; i++) {
    if (base.structure->fields[i].name
        && strcmp(base.structure->fields[i].name, field) == 0) {
      base.structure->fields[i].value = value;
      return base;
    }
  }
  return nova_null();
}

static int nova_enum_is(NovaValue value, const char* variant) {
  return value.tag == NOVA_ENUM
      && value.enumeration
      && strcmp(value.enumeration->variant, variant) == 0;
}

static NovaValue nova_enum_payload(NovaValue value) {
  if (value.tag != NOVA_ENUM || !value.enumeration) return nova_null();
  return value.enumeration->payload;
}

static void nova_print(NovaValue v) {
  switch (v.tag) {
    case NOVA_BOOL:
      printf("%s\n", v.number ? "true" : "false");
      break;
    case NOVA_NUMBER:
      printf("%.15g\n", v.number);
      break;
    case NOVA_STRING:
      printf("%s\n", v.string ? v.string : "");
      break;
    case NOVA_STRUCT:
      printf("<struct %s>\n", v.structure && v.structure->name ? v.structure->name : "?");
      break;
    case NOVA_ENUM:
      if (v.enumeration && v.enumeration->payload.tag != NOVA_NULL)
        printf("%s.%s\n", v.enumeration->name, v.enumeration->variant);
      else
        printf("%s.%s\n",
               v.enumeration ? v.enumeration->name : "?",
               v.enumeration ? v.enumeration->variant : "?");
      break;
    case NOVA_CLOSURE:
      printf("<closure>\n");
      break;
    default:
      printf("null\n");
      break;
  }
}

static NovaClosure* nova_make_closure(
    NovaValue (*invoke)(NovaEnv*, NovaValue*, size_t),
    size_t captures
) {
  if (captures > 32) return NULL;
  NovaClosure* closure = (NovaClosure*)calloc(1, sizeof(NovaClosure));
  if (!closure) return NULL;
  closure->env = (NovaEnv*)calloc(1, sizeof(NovaEnv));
  if (!closure->env) {
    free(closure);
    return NULL;
  }
  closure->env->len = captures;
  closure->invoke = invoke;
  return closure;
}

static NovaValue nova_call_closure(
    NovaValue callee,
    NovaValue* args,
    size_t argc
) {
  if (
      callee.tag != NOVA_CLOSURE ||
      !callee.closure ||
      !callee.closure->invoke
  ) {
    return nova_null();
  }
  return callee.closure->invoke(callee.closure->env, args, argc);
}

"#.to_string();

    let mut out = runtime;

    for function in functions {
        out.push_str(&format!(
            "static NovaValue {}(NovaEnv* env, NovaValue* args, size_t argc);\n",
            c_ident(&function.name)
        ));
    }
    out.push('\n');

    for function in functions {
        out.push_str(&emit_function(function, &names)?);
    }

    out.push_str(
        "int main(void) {\n"
    );
    out.push_str("  NovaValue result = nova_main(NULL, NULL, 0);\n");
    out.push_str("  (void)result;\n");
    out.push_str("  return 0;\n");
    out.push_str("}\n");

    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn c_string_escapes_quotes_and_newlines() {
        assert_eq!(super::c_string("a\"b\n"), "\"a\\\"b\\n\"");
    }
}


#[cfg(test)]
mod native_execution_tests {
    use std::process::Command;

    #[test]
    fn native_load_store_executes_real_binary() {
        use crate::ir::{IrType, SsaBlock, SsaFunction, SsaInstr, SsaValue, Terminator};

        let function = SsaFunction {
            name: "<main>".into(),
            params: Vec::new(),
            captures: Vec::new(),
            return_type: IrType::Null,
            blocks: vec![SsaBlock {
                id: 0,
                params: Vec::new(),
                instrs: vec![
                    (0, SsaInstr::Const(SsaValue::Number(7.0))),
                    (1, SsaInstr::Store {
                        name: "answer".into(),
                        value: 0,
                    }),
                    (2, SsaInstr::Load {
                        name: "answer".into(),
                    }),
                    (3, SsaInstr::Call {
                        name: "print".into(),
                        args: vec![2],
                        result: IrType::Null,
                    }),
                ],
                terminator: Some(Terminator::Return(None)),
            }],
        };

        let generated = crate::backend_ssa_c::emit_c(&[function])
            .expect("SSA native backend should emit load/store program");

        let dir = std::env::temp_dir().join(format!(
            "nova-native-load-store-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create native test directory");
        let c_path = dir.join("state.c");
        let bin_path = dir.join("state-bin");
        std::fs::write(&c_path, generated).expect("write generated C");

        let compile = Command::new("cc")
            .args([
                "-O2",
                "-std=c11",
                c_path.to_str().expect("C path"),
                "-o",
                bin_path.to_str().expect("binary path"),
            ])
            .status()
            .expect("invoke C compiler");
        assert!(compile.success(), "C compiler failed: {compile}");

        let output = Command::new(&bin_path)
            .output()
            .expect("execute native load/store binary");
        assert!(output.status.success(), "native program failed");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn native_struct_array_map_mutation_executes_real_binary() {
        let source = r#"
            struct Person {
                name: string
                age: i64
            }

            person = Person { name: "Laerth", age: 18 }
            person.age = 21

            values = [1, 2, 3]
            values[1] = 9

            scores = map{"x": 10}
            scores["x"] = 42
            scores["y"] = 7

            print person.age
            print values[1]
            print scores["x"]
            print scores["y"]
        "#;

        let compilation = crate::compiler::compile_source(source)
            .expect("structured mutation source should compile");
        let generated = crate::backend_ssa_c::emit_c(&compilation.ssa_functions)
            .expect("SSA native backend should emit structured mutation");

        let dir = std::env::temp_dir().join(format!(
            "nova-native-mutation-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create native test directory");
        let c_path = dir.join("mutation.c");
        let bin_path = dir.join("mutation-bin");
        std::fs::write(&c_path, generated).expect("write generated C");

        let compile = Command::new("cc")
            .args([
                "-O2",
                "-std=c11",
                c_path.to_str().expect("C path"),
                "-o",
                bin_path.to_str().expect("binary path"),
            ])
            .status()
            .expect("invoke C compiler");
        assert!(compile.success(), "C compiler failed: {compile}");

        let output = Command::new(&bin_path)
            .output()
            .expect("execute native structured mutation binary");
        assert!(output.status.success(), "native program failed");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "21\n9\n42\n7\n");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn native_option_result_try_executes_real_binary() {
        let source = r#"
            fn maybe_number(flag: bool) -> Option<i64> {
                if flag {
                    return Some(42)
                }
                return None
            }

            fn compute(flag: bool) -> Option<i64> {
                value = maybe_number(flag)?
                return Some(value + 8)
            }

            fn parse_name(ok: bool) -> Result<string, string> {
                if ok {
                    return Ok("NOVA")
                }
                return Err("name unavailable")
            }

            fn greet(ok: bool) -> Result<string, string> {
                name = parse_name(ok)?
                return Ok("Hello " + name)
            }

            print unwrap(compute(true))
            print is_none(compute(false))
            print unwrap(greet(true))
            print unwrap_or(greet(false), "fallback")
        "#;

        let compilation = crate::compiler::compile_source(source)
            .expect("Option/Result/Try source should compile");
        let generated = crate::backend_ssa_c::emit_c(&compilation.ssa_functions)
            .expect("SSA native backend should emit Option/Result/Try runtime");

        assert!(generated.contains("nova_try"));
        assert!(generated.contains("setjmp"));
        assert!(generated.contains("longjmp"));
        assert!(generated.contains("nova_unwrap_or"));

        let dir = std::env::temp_dir().join(format!(
            "nova-native-try-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create native test directory");
        let c_path = dir.join("try.c");
        let bin_path = dir.join("try-bin");
        std::fs::write(&c_path, generated).expect("write generated C");

        let compile = Command::new("cc")
            .args([
                "-O2",
                "-std=c11",
                c_path.to_str().expect("C path"),
                "-o",
                bin_path.to_str().expect("binary path"),
            ])
            .status()
            .expect("invoke C compiler");
        assert!(compile.success(), "C compiler failed: {compile}");

        let output = Command::new(&bin_path)
            .output()
            .expect("execute native Option/Result/Try binary");
        assert!(
            output.status.success(),
            "native program failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "50\ntrue\nHello NOVA\nfallback\n"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
