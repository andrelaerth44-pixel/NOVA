use crate::ir::{IrType, SsaFunction, SsaInstr, SsaValue, Terminator, ValueId};

fn c_ident(name: &str) -> String {
    let mut out = String::from("nova_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' { out.push(ch); } else { out.push('_'); }
    }
    out
}

fn v(id: ValueId) -> String { format!("v{}", id) }

fn value_expr(id: ValueId) -> String { v(id) }

fn binary_expr(op: &str, left: &str, right: &str) -> Option<String> {
    let x = match op {
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
    };
    Some(x)
}

fn max_value_id(function: &SsaFunction) -> ValueId {
    let mut max = 0;
    for (_, _, id) in &function.params { max = max.max(*id); }
    for (_, _, id) in &function.captures { max = max.max(*id); }
    for block in &function.blocks {
        for (id, _) in &block.instrs { max = max.max(*id); }
        for id in &block.params { max = max.max(*id); }
    }
    max
}

fn emit_function(
    function: &SsaFunction,
    functions: &std::collections::HashSet<String>,
) -> Result<String, String> {
    let max = max_value_id(function);
    let mut out = String::new();
    out.push_str(&format!("static NovaValue {}(NovaEnv* env, NovaValue* args, size_t argc) {{
", c_ident(&function.name)));
    for id in 0..=max { out.push_str(&format!("  NovaValue {} = nova_null();
", v(id))); }

    for (slot, (_, _, id)) in function.captures.iter().enumerate() {
        out.push_str(&format!("  {} = env ? env->slots[{}] : nova_null();
", v(*id), slot));
    }
    for (index, (_, _, id)) in function.params.iter().enumerate() {
        out.push_str(&format!("  {} = (argc > {}) ? args[{}] : nova_null();
", v(*id), index, index));
    }

    for block in &function.blocks {
        out.push_str(&format!("  B{}:
", block.id));
        for (id, instr) in &block.instrs {
            match instr {
                SsaInstr::Const(SsaValue::Number(n)) => {
                    out.push_str(&format!("  {} = nova_num({:.17});
", v(*id), n));
                }
                SsaInstr::Const(SsaValue::Bool(b)) => {
                    out.push_str(&format!("  {} = nova_bool({});
", v(*id), if *b { "1" } else { "0" }));
                }
                SsaInstr::Const(SsaValue::Null) => {
                    out.push_str(&format!("  {} = nova_null();
", v(*id)));
                }
                SsaInstr::Const(SsaValue::String(_))
                | SsaInstr::Const(SsaValue::Struct { .. })
                | SsaInstr::Const(SsaValue::Param(_))
                | SsaInstr::Const(SsaValue::Instr(_)) => {
                    return Err(format!("SSA C backend: unsupported constant in {}", function.name));
                }
                SsaInstr::Load { name } => {
                    return Err(format!("SSA C backend: unresolved Load({}) in {}", name, function.name));
                }
                SsaInstr::Store { name, value } => {
                    if let Some(slot) = function.captures.iter().position(|(capture, _, _)| capture == name) {
                        out.push_str(&format!("  if (env) env->slots[{}] = {};
", slot, value_expr(*value)));
                    }
                }
                SsaInstr::Unary { op, value, .. } => match op.as_str() {
                    "Minus" => out.push_str(&format!("  {} = nova_num(-{}.number);
", v(*id), value_expr(*value))),
                    "Bang" => out.push_str(&format!("  {} = nova_bool({}.number == 0);
", v(*id), value_expr(*value))),
                    other => return Err(format!("SSA C backend: unsupported unary {}", other)),
                },
                SsaInstr::Binary { op, left, right, .. } => {
                    let Some(expr) = binary_expr(op, &value_expr(*left), &value_expr(*right)) else {
                        return Err(format!("SSA C backend: unsupported binary {}", op));
                    };
                    out.push_str(&format!("  {} = {};
", v(*id), expr));
                }
                SsaInstr::Call { name, args, result } => {
                    if name == "print" {
                        if args.len() != 1 { return Err("SSA C backend: print expects one argument".into()); }
                        out.push_str(&format!("  nova_print({});
", value_expr(args[0])));
                        out.push_str(&format!("  {} = nova_null();
", v(*id)));
                    } else if functions.contains(name) {
                        let cname = c_ident(name);
                        let arr = format!("call_args_{}", id);
                        if args.is_empty() {
                            out.push_str(&format!("  {} = {}(NULL, NULL, 0);
", v(*id), cname));
                        } else {
                            out.push_str(&format!("  NovaValue {}[{}] = {{{}}};
", arr, args.len(), args.iter().map(|a| value_expr(*a)).collect::<Vec<_>>().join(", ")));
                            out.push_str(&format!("  {} = {}(NULL, {}, {});
", v(*id), cname, arr, args.len()));
                        }
                    } else if matches!(name.as_str(), "array" | "map" | "set" | "index" | "for_each" | "loop_item" | "try") {
                        return Err(format!("SSA C backend: builtin {} is not implemented", name));
                    } else {
                        return Err(format!("SSA C backend: unknown function {}", name));
                    }

                    if matches!(result, IrType::Null) && name != "print" {
                        out.push_str(&format!("  {} = nova_null();
", v(*id)));
                    }
                }
                SsaInstr::CallIndirect { callee, args, .. } => {
                    let arr = format!("indirect_args_{}", id);
                    if args.is_empty() {
                        out.push_str(&format!("  {} = nova_call_closure({}, NULL, 0);
", v(*id), value_expr(*callee)));
                    } else {
                        out.push_str(&format!("  NovaValue {}[{}] = {{{}}};
", arr, args.len(), args.iter().map(|a| value_expr(*a)).collect::<Vec<_>>().join(", ")));
                        out.push_str(&format!("  {} = nova_call_closure({}, {}, {});
", v(*id), value_expr(*callee), arr, args.len()));
                    }
                }
                SsaInstr::Closure { function, captures, .. } => {
                    let arr = format!("capture_{}", id);
                    out.push_str(&format!("  NovaClosure* closure_{} = nova_make_closure({}, {});
", id, c_ident(function), captures.len()));
                    for (slot, (_, value)) in captures.iter().enumerate() {
                        out.push_str(&format!("  closure_{}->env->slots[{}] = {};
", id, slot, value_expr(*value)));
                    }
                    out.push_str(&format!("  {} = nova_closure_value(closure_{});
", v(*id), id));
                    let _ = arr;
                }
                SsaInstr::Phi { .. } => return Err(format!("SSA C backend: Phi not yet lowered for {}", function.name)),
                SsaInstr::StructInit { .. }
                | SsaInstr::FieldGet { .. }
                | SsaInstr::EnumInit { .. }
                | SsaInstr::EnumTest { .. }
                | SsaInstr::EnumPayload { .. }
                | SsaInstr::Try { .. } => {
                    return Err(format!("SSA C backend: instruction {:?} not implemented", instr));
                }
            }
        }
        match &block.terminator {
            Some(Terminator::Jump(target)) => out.push_str(&format!("  goto B{};
", target)),
            Some(Terminator::Branch { condition, then_block, else_block }) => {
                out.push_str(&format!("  if ({}.number != 0) goto B{}; else goto B{};
", value_expr(*condition), then_block, else_block));
            }
            Some(Terminator::Return(Some(value))) => out.push_str(&format!("  return {};
", value_expr(*value))),
            Some(Terminator::Return(None)) => out.push_str("  return nova_null();
"),
            None => return Err(format!("SSA C backend: block {} in {} has no terminator", block.id, function.name)),
        }
    }
    out.push_str("}

");
    Ok(out)
}

pub fn emit_c(functions: &[SsaFunction]) -> Result<String, String> {
    let names = functions.iter().map(|f| f.name.clone()).collect::<std::collections::HashSet<_>>();
    let mut out = String::from(
        "#include <math.h>\n#include <stdint.h>\n#include <stddef.h>\n#include <stdio.h>\n#include <stdlib.h>\n\n         typedef enum { NOVA_NULL, NOVA_NUMBER, NOVA_BOOL, NOVA_CLOSURE } NovaTag;\n         typedef struct NovaValue NovaValue;\n         typedef struct NovaClosure NovaClosure;\n         typedef struct NovaEnv NovaEnv;\n         struct NovaValue { NovaTag tag; double number; NovaClosure* closure; };\n         struct NovaEnv { size_t len; NovaValue slots[32]; };\n         struct NovaClosure { NovaEnv* env; NovaValue (*invoke)(NovaEnv*, NovaValue*, size_t); };\n\n         static NovaValue nova_null(void) { NovaValue v={NOVA_NULL,0,NULL}; return v; }\n         static NovaValue nova_num(double x) { NovaValue v={NOVA_NUMBER,x,NULL}; return v; }\n         static NovaValue nova_bool(int x) { NovaValue v={NOVA_BOOL,x?1.0:0.0,NULL}; return v; }\n         static NovaValue nova_closure_value(NovaClosure* c) { NovaValue v={NOVA_CLOSURE,0,c}; return v; }\n         static void nova_print(NovaValue v) {\n           if(v.tag==NOVA_BOOL) printf("%s\\n", v.number ? "true" : "false");\n           else if(v.tag==NOVA_NUMBER) printf("%.15g\\n", v.number);\n           else if(v.tag==NOVA_NULL) printf("null\\n");\n           else printf("<closure>\\n");\n         }\n         static NovaClosure* nova_make_closure(NovaValue (*invoke)(NovaEnv*, NovaValue*, size_t), size_t captures) {\n           if(captures > 32) return NULL;\n           NovaClosure* c=(NovaClosure*)calloc(1,sizeof(NovaClosure));\n           if(!c) return NULL;\n           c->env=(NovaEnv*)calloc(1,sizeof(NovaEnv));\n           if(!c->env) { free(c); return NULL; }\n           c->env->len=captures; c->invoke=invoke; return c;\n         }\n         static NovaValue nova_call_closure(NovaValue callee, NovaValue* args, size_t argc) {\n           if(callee.tag!=NOVA_CLOSURE || !callee.closure || !callee.closure->invoke) return nova_null();\n           return callee.closure->invoke(callee.closure->env,args,argc);\n         }\n\n"
    );

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

    out.push_str("int main(void) { NovaValue result = nova_main(NULL, NULL, 0); (void)result; return 0; }\n");
    Ok(out)
}
