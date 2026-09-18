use crate::ir::{IrType, SsaFunction, SsaInstr, SsaValue, Terminator, ValueId};

fn c_ident(name: &str) -> String {
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

fn binary_expr(op: &str, left: &str, right: &str) -> Option<String> {
    Some(match op {
        "Plus" => format!("nova_num({}.number + {}.number)", left, right),
        "Minus" => format!("nova_num({}.number - {}.number)", left, right),
        "Star" => format!("nova_num({}.number * {}.number)", left, right),
        "Slash" => format!("nova_num({}.number / {}.number)", left, right),
        "Percent" => format!("nova_num(fmod({}.number, {}.number))", left, right),
        "EqEq" => format!("nova_bool({}.number == {}.number)", left, right),
        "NotEq" => format!("nova_bool({}.number != {}.number)", left, right),
        "Lt" => format!("nova_bool({}.number < {}.number)", left, right),
        "Le" => format!("nova_bool({}.number <= {}.number)", left, right),
        "Gt" => format!("nova_bool({}.number > {}.number)", left, right),
        "Ge" => format!("nova_bool({}.number >= {}.number)", left, right),
        "And" => format!("nova_bool({}.number != 0 && {}.number != 0)", left, right),
        "Or" => format!("nova_bool({}.number != 0 || {}.number != 0)", left, right),
        _ => return None,
    })
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
    out.push_str(" {");
    out.push('\n');

    for id in 0..=max {
        out.push_str(&format!("  NovaValue {} = nova_null();", v(id)));
        out.push('\n');
    }

    for (slot, (_, _, id)) in function.captures.iter().enumerate() {
        out.push_str(&format!(
            "  {} = env ? env->slots[{}] : nova_null();",
            v(*id),
            slot
        ));
        out.push('\n');
    }

    for (index, (_, _, id)) in function.params.iter().enumerate() {
        out.push_str(&format!(
            "  {} = (argc > {}) ? args[{}] : nova_null();",
            v(*id),
            index,
            index
        ));
        out.push('\n');
    }

    for block in &function.blocks {
        out.push_str(&format!("B{}:", block.id));
        out.push('\n');

        for (id, instr) in &block.instrs {
            match instr {
                SsaInstr::Const(SsaValue::Number(n)) => {
                    out.push_str(&format!("  {} = nova_num({:.17});", v(*id), n));
                    out.push('\n');
                }
                SsaInstr::Const(SsaValue::Bool(b)) => {
                    out.push_str(&format!(
                        "  {} = nova_bool({});",
                        v(*id),
                        if *b { "1" } else { "0" }
                    ));
                    out.push('\n');
                }
                SsaInstr::Const(SsaValue::Null) => {
                    out.push_str(&format!("  {} = nova_null();", v(*id)));
                    out.push('\n');
                }
                SsaInstr::Const(SsaValue::String(_))
                | SsaInstr::Const(SsaValue::Struct { .. })
                | SsaInstr::Const(SsaValue::Param(_))
                | SsaInstr::Const(SsaValue::Instr(_)) => {
                    return Err(format!(
                        "SSA C backend: unsupported constant in {}",
                        function.name
                    ));
                }
                SsaInstr::Load { name } => {
                    return Err(format!(
                        "SSA C backend: unresolved Load({}) in {}",
                        name, function.name
                    ));
                }
                SsaInstr::Store { name, value } => {
                    if let Some(slot) = function
                        .captures
                        .iter()
                        .position(|(capture, _, _)| capture == name)
                    {
                        out.push_str(&format!(
                            "  if (env) env->slots[{}] = {};",
                            slot,
                            v(*value)
                        ));
                        out.push('\n');
                    }
                }
                SsaInstr::Unary { op, value, .. } => match op.as_str() {
                    "Minus" => {
                        out.push_str(&format!(
                            "  {} = nova_num(-{}.number);",
                            v(*id),
                            v(*value)
                        ));
                        out.push('\n');
                    }
                    "Bang" => {
                        out.push_str(&format!(
                            "  {} = nova_bool({}.number == 0);",
                            v(*id),
                            v(*value)
                        ));
                        out.push('\n');
                    }
                    other => {
                        return Err(format!(
                            "SSA C backend: unsupported unary {}",
                            other
                        ));
                    }
                },
                SsaInstr::Binary {
                    op, left, right, ..
                } => {
                    let Some(expr) = binary_expr(op, &v(*left), &v(*right)) else {
                        return Err(format!(
                            "SSA C backend: unsupported binary {}",
                            op
                        ));
                    };
                    out.push_str(&format!("  {} = {};", v(*id), expr));
                    out.push('\n');
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
                        out.push_str(&format!("  nova_print({});", v(args[0])));
                        out.push('\n');
                        out.push_str(&format!("  {} = nova_null();", v(*id)));
                        out.push('\n');
                    } else if functions.contains(name) {
                        let cname = c_ident(name);
                        if args.is_empty() {
                            out.push_str(&format!(
                                "  {} = {}(NULL, NULL, 0);",
                                v(*id),
                                cname
                            ));
                            out.push('\n');
                        } else {
                            out.push_str(&format!(
                                "  NovaValue call_args_{}[{}] = {{ {} }};",
                                id,
                                args.len(),
                                args.iter()
                                    .map(|a| v(*a))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                            out.push('\n');
                            out.push_str(&format!(
                                "  {} = {}(NULL, call_args_{}, {});",
                                v(*id),
                                cname,
                                id,
                                args.len()
                            ));
                            out.push('\n');
                        }
                    } else {
                        return Err(format!(
                            "SSA C backend: unknown or unsupported call {}",
                            name
                        ));
                    }

                    if matches!(result, IrType::Null) && name != "print" {
                        out.push_str(&format!("  {} = nova_null();", v(*id)));
                        out.push('\n');
                    }
                }
                SsaInstr::CallIndirect { callee, args, .. } => {
                    if args.is_empty() {
                        out.push_str(&format!(
                            "  {} = nova_call_closure({}, NULL, 0);",
                            v(*id),
                            v(*callee)
                        ));
                        out.push('\n');
                    } else {
                        out.push_str(&format!(
                            "  NovaValue indirect_args_{}[{}] = {{ {} }};",
                            id,
                            args.len(),
                            args.iter()
                                .map(|a| v(*a))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        out.push('\n');
                        out.push_str(&format!(
                            "  {} = nova_call_closure({}, indirect_args_{}, {});",
                            v(*id),
                            v(*callee),
                            id,
                            args.len()
                        ));
                        out.push('\n');
                    }
                }
                SsaInstr::Closure {
                    function,
                    captures,
                    ..
                } => {
                    out.push_str(&format!(
                        "  NovaClosure* closure_{} = nova_make_closure({}, {});",
                        id,
                        c_ident(function),
                        captures.len()
                    ));
                    out.push('\n');

                    for (slot, (_, value)) in captures.iter().enumerate() {
                        out.push_str(&format!(
                            "  if (closure_{}) closure_{}->env->slots[{}] = {};",
                            id,
                            id,
                            slot,
                            v(*value)
                        ));
                        out.push('\n');
                    }

                    out.push_str(&format!(
                        "  {} = nova_closure_value(closure_{});",
                        v(*id),
                        id
                    ));
                    out.push('\n');
                }
                SsaInstr::Phi { .. } => {
                    return Err(format!(
                        "SSA C backend: Phi not yet lowered for {}",
                        function.name
                    ));
                }
                SsaInstr::StructInit { .. }
                | SsaInstr::FieldGet { .. }
                | SsaInstr::EnumInit { .. }
                | SsaInstr::EnumTest { .. }
                | SsaInstr::EnumPayload { .. }
                | SsaInstr::Try { .. } => {
                    return Err(format!(
                        "SSA C backend: instruction {:?} not implemented",
                        instr
                    ));
                }
            }
        }

        match &block.terminator {
            Some(Terminator::Jump(target)) => {
                out.push_str(&format!("  goto B{};", target));
                out.push('\n');
            }
            Some(Terminator::Branch {
                condition,
                then_block,
                else_block,
            }) => {
                out.push_str(&format!(
                    "  if ({}.number != 0) goto B{}; else goto B{};",
                    v(*condition),
                    then_block,
                    else_block
                ));
                out.push('\n');
            }
            Some(Terminator::Return(Some(value))) => {
                out.push_str(&format!("  return {};", v(*value)));
                out.push('\n');
            }
            Some(Terminator::Return(None)) => {
                out.push_str("  return nova_null();");
                out.push('\n');
            }
            None => {
                return Err(format!(
                    "SSA C backend: block {} in {} has no terminator",
                    block.id, function.name
                ));
            }
        }
    }

    out.push_str("}");
    out.push('\n');
    out.push('\n');
    Ok(out)
}

pub fn emit_c(functions: &[SsaFunction]) -> Result<String, String> {
    let names = functions
        .iter()
        .map(|f| f.name.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut out = r#"#include <math.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

typedef enum { NOVA_NULL, NOVA_NUMBER, NOVA_BOOL, NOVA_CLOSURE } NovaTag;
typedef struct NovaValue NovaValue;
typedef struct NovaClosure NovaClosure;
typedef struct NovaEnv NovaEnv;

struct NovaValue {
  NovaTag tag;
  double number;
  NovaClosure* closure;
};

struct NovaEnv {
  size_t len;
  NovaValue slots[32];
};

struct NovaClosure {
  NovaEnv* env;
  NovaValue (*invoke)(NovaEnv*, NovaValue*, size_t);
};

static NovaValue nova_null(void) {
  NovaValue v = { NOVA_NULL, 0, NULL };
  return v;
}

static NovaValue nova_num(double x) {
  NovaValue v = { NOVA_NUMBER, x, NULL };
  return v;
}

static NovaValue nova_bool(int x) {
  NovaValue v = { NOVA_BOOL, x ? 1.0 : 0.0, NULL };
  return v;
}

static NovaValue nova_closure_value(NovaClosure* c) {
  NovaValue v = { NOVA_CLOSURE, 0, c };
  return v;
}

static void nova_print(NovaValue v) {
  if (v.tag == NOVA_BOOL) {
    printf("%s\n", v.number ? "true" : "false");
  } else if (v.tag == NOVA_NUMBER) {
    printf("%.15g\n", v.number);
  } else if (v.tag == NOVA_NULL) {
    printf("null\n");
  } else {
    printf("<closure>\n");
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

    for function in functions {
        out.push_str(&format!(
            "static NovaValue {}(NovaEnv* env, NovaValue* args, size_t argc);",
            c_ident(&function.name)
        ));
        out.push('\n');
    }
    out.push('\n');

    for function in functions {
        out.push_str(&emit_function(function, &names)?);
    }

    out.push_str("int main(void) {");
    out.push('\n');
    out.push_str("  NovaValue result = nova_main(NULL, NULL, 0);");
    out.push('\n');
    out.push_str("  (void)result;");
    out.push('\n');
    out.push_str("  return 0;");
    out.push('\n');
    out.push_str("}");
    out.push('\n');

    Ok(out)
}
