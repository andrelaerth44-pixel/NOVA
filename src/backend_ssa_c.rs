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
        "And" => format!("nova_bool(nova_truthy({}) && nova_truthy({}))", left, right),
        "Or" => format!("nova_bool(nova_truthy({}) || nova_truthy({}))", left, right),
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
                    } else if name == "ord" || name == "chr" {
                        if (args.len() != 1) {
                            return Err(format!("SSA C backend: {} expects one argument", name));
                        }
                        let helper = if name == "ord" { "nova_ord" } else { "nova_chr" };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            v(args[0])
                        ));
                    } else if name == "push" {
                        if (args.len() != 2) {
                            return Err("SSA C backend: push expects two arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_push({}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1])
                        ));
                    } else if name == "pop" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: pop expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_pop({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "str" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: str expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_str({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if matches!(name.as_str(), "abs" | "sqrt") {
                        if args.len() != 1 {
                            return Err(format!("SSA C backend: {} expects one argument", name));
                        }
                        let helper = if name == "abs" { "nova_abs" } else { "nova_sqrt" };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            v(args[0])
                        ));
                    } else if name == "env" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: env expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_env({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "json_parse" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: json_parse expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_json_parse({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "json_stringify" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: json_stringify expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_json_stringify({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "read_file" {
                        if args.len() != 1 {
                            return Err("SSA C backend: read_file expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_read_file({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "write_file" {
                        if args.len() != 2 {
                            return Err("SSA C backend: write_file expects two arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_write_file({}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1])
                        ));
                    } else if name == "exists" {
                        if args.len() != 1 {
                            return Err("SSA C backend: exists expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_exists({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "current_dir" {
                        if !args.is_empty() {
                            return Err("SSA C backend: current_dir expects zero arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_current_dir();\n",
                            v(*id)
                        ));
                    } else if name == "path_join" {
                        if args.len() != 2 {
                            return Err("SSA C backend: path_join expects two arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_path_join({}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1])
                        ));
                    } else if name == "path_basename" || name == "path_dirname" || name == "path_ext" || name == "path_stem" {
                        if args.len() != 1 {
                            return Err(format!("SSA C backend: {} expects one argument", name));
                        }
                        let helper = match name.as_str() {
                            "path_basename" => "nova_path_basename",
                            "path_dirname" => "nova_path_dirname",
                            "path_ext" => "nova_path_ext",
                            _ => "nova_path_stem",
                        };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            v(args[0])
                        ));
                    } else if matches!(name.as_str(), "make_dir" | "remove_file" | "list_dir") {
                        if args.len() != 1 {
                            return Err(format!("SSA C backend: {} expects one argument", name));
                        }
                        let helper = match name.as_str() {
                            "make_dir" => "nova_make_dir",
                            "remove_file" => "nova_remove_file",
                            _ => "nova_list_dir",
                        };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            v(args[0])
                        ));
                    } else if matches!(name.as_str(), "now_ms" | "now_s") {
                        if !args.is_empty() {
                            return Err(format!("SSA C backend: {} expects zero arguments", name));
                        }
                        let helper = if name == "now_ms" { "nova_now_ms" } else { "nova_now_s" };
                        out.push_str(&format!(
                            "  {} = {}();\n",
                            v(*id),
                            helper
                        ));
                    } else if name == "sleep_ms" {
                        if args.len() != 1 {
                            return Err("SSA C backend: sleep_ms expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_sleep_ms({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "range" {
                        if (args.len() != 2) {
                            return Err("SSA C backend: range expects two arguments".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_range({}, {});\n",
                            v(*id),
                            v(args[0]),
                            v(args[1])
                        ));
                    } else if name == "iter" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: iter expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_iter({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "next" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: next expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_next({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "has_next" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: has_next expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_has_next({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if name == "collect" {
                        if (args.len() != 1) {
                            return Err("SSA C backend: collect expects one argument".into());
                        }
                        out.push_str(&format!(
                            "  {} = nova_collect({});\n",
                            v(*id),
                            v(args[0])
                        ));
                    } else if matches!(name.as_str(), "map_get" | "map_has" | "map_set" | "map_remove" | "set_add" | "set_has" | "set_remove") {
                        let expected = match name.as_str() {
                            "map_set" => 3,
                            _ => 2,
                        };
                        if args.len() != expected {
                            return Err(format!("SSA C backend: {} expects {} arguments", name, expected));
                        }
                        let helper = match name.as_str() {
                            "map_get" => "nova_map_get",
                            "map_has" => "nova_map_has",
                            "map_set" => "nova_map_set",
                            "map_remove" => "nova_map_remove",
                            "set_add" => "nova_set_add",
                            "set_has" => "nova_set_has",
                            _ => "nova_set_remove",
                        };
                        out.push_str(&format!(
                            "  {} = {}({});\n",
                            v(*id),
                            helper,
                            args.iter().map(|a| v(*a)).collect::<Vec<_>>().join(", ")
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
#define _POSIX_C_SOURCE 200809L
#include <math.h>
#include <setjmp.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>

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
  NOVA_SET,
  NOVA_ITERATOR
} NovaTag;

typedef struct NovaValue NovaValue;
typedef struct NovaClosure NovaClosure;
typedef struct NovaEnv NovaEnv;
typedef struct NovaStruct NovaStruct;
typedef struct NovaEnum NovaEnum;
typedef struct NovaArray NovaArray;
typedef struct NovaMap NovaMap;
typedef struct NovaSet NovaSet;
typedef struct NovaIterator NovaIterator;

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
  NovaIterator* iterator;
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

struct NovaIterator {
  size_t len;
  size_t index;
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
  NovaValue v = { NOVA_NULL, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_num(double x) {
  NovaValue v = { NOVA_NUMBER, x, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_bool(int x) {
  NovaValue v = { NOVA_BOOL, x ? 1.0 : 0.0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_string(const char* x) {
  NovaValue v = { NOVA_STRING, 0, x, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_struct_value(NovaStruct* x) {
  NovaValue v = { NOVA_STRUCT, 0, NULL, NULL, x, NULL, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_enum_value(const char* name, const char* variant, NovaValue payload) {
  NovaEnum* e = (NovaEnum*)calloc(1, sizeof(NovaEnum));
  if (!e) return nova_null();
  e->name = nova_dup(name);
  e->variant = nova_dup(variant);
  e->payload = payload;
  NovaValue v = { NOVA_ENUM, 0, NULL, NULL, NULL, e, NULL, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_closure_value(NovaClosure* c) {
  NovaValue v = { NOVA_CLOSURE, 0, NULL, c, NULL, NULL, NULL, NULL, NULL, NULL };
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

static size_t nova_utf8_length(const char* text);

static NovaValue nova_len(NovaValue value) {
  switch (value.tag) {
    case NOVA_STRING: return nova_num(value.string ? (double)nova_utf8_length(value.string) : 0.0);
    case NOVA_ARRAY: return nova_num(value.array ? (double)value.array->len : 0.0);
    case NOVA_MAP: return nova_num(value.map ? (double)value.map->len : 0.0);
    case NOVA_SET: return nova_num(value.set ? (double)value.set->len : 0.0);
    case NOVA_ITERATOR: return nova_num(value.iterator ? (double)(value.iterator->len - value.iterator->index) : 0.0);
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
  NovaValue v = { NOVA_ARRAY, 0, NULL, NULL, NULL, NULL, a, NULL, NULL, NULL };
  return v;
}

static NovaValue nova_map_value(NovaMap* m) {
  NovaValue v = { NOVA_MAP, 0, NULL, NULL, NULL, NULL, NULL, m, NULL, NULL };
  return v;
}

static NovaValue nova_set_value(NovaSet* s) {
  NovaValue v = { NOVA_SET, 0, NULL, NULL, NULL, NULL, NULL, NULL, s, NULL };
  return v;
}

static NovaValue nova_iterator_value(NovaIterator* iterator) {
  NovaValue v = { NOVA_ITERATOR, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, iterator };
  return v;
}

static NovaValue nova_ord(NovaValue value) {
  if (value.tag != NOVA_STRING || !value.string) return nova_num(0);
  const unsigned char* p = (const unsigned char*)value.string;
  if ((*p & 0x80u) == 0) return nova_num((double)*p);
  if ((*p & 0xe0u) == 0xc0u) return nova_num((double)(((p[0] & 0x1fu) << 6) | (p[1] & 0x3fu)));
  if ((*p & 0xf0u) == 0xe0u) return nova_num((double)(((p[0] & 0x0fu) << 12) | ((p[1] & 0x3fu) << 6) | (p[2] & 0x3fu)));
  if ((*p & 0xf8u) == 0xf0u) return nova_num((double)(((p[0] & 0x07u) << 18) | ((p[1] & 0x3fu) << 12) | ((p[2] & 0x3fu) << 6) | (p[3] & 0x3fu)));
  return nova_num(0);
}

static NovaValue nova_chr(NovaValue value) {
  if (value.tag != NOVA_NUMBER || value.number < 0 || value.number > 0x10ffff) return nova_null();
  uint32_t code = (uint32_t)value.number;
  if (code >= 0xd800 && code <= 0xdfff) return nova_null();
  char* out = (char*)calloc(5, 1);
  if (!out) return nova_null();
  size_t len = 0;
  if (code <= 0x7f) {
    out[len++] = (char)code;
  } else if (code <= 0x7ff) {
    out[len++] = (char)(0xc0 | (code >> 6));
    out[len++] = (char)(0x80 | (code & 0x3f));
  } else if (code <= 0xffff) {
    out[len++] = (char)(0xe0 | (code >> 12));
    out[len++] = (char)(0x80 | ((code >> 6) & 0x3f));
    out[len++] = (char)(0x80 | (code & 0x3f));
  } else {
    out[len++] = (char)(0xf0 | (code >> 18));
    out[len++] = (char)(0x80 | ((code >> 12) & 0x3f));
    out[len++] = (char)(0x80 | ((code >> 6) & 0x3f));
    out[len++] = (char)(0x80 | (code & 0x3f));
  }
  return nova_string(out);
}

static NovaValue nova_str(NovaValue value) {
  char buffer[64];
  switch (value.tag) {
    case NOVA_STRING:
      return nova_string(value.string ? value.string : "");
    case NOVA_BOOL:
      return nova_string(value.number != 0 ? "true" : "false");
    case NOVA_NULL:
      return nova_string("null");
    case NOVA_NUMBER:
      snprintf(buffer, sizeof(buffer), "%.17g", value.number);
      if (strchr(buffer, '.')) {
        size_t len = strlen(buffer);
        while (len > 0 && buffer[len - 1] == '0') buffer[--len] = '\0';
        if (len > 0 && buffer[len - 1] == '.') buffer[--len] = '\0';
      }
      return nova_string(nova_dup(buffer));
    default:
      return nova_string("<value>");
  }
}

static NovaValue nova_abs(NovaValue value) {
  return value.tag == NOVA_NUMBER ? nova_num(fabs(value.number)) : nova_null();
}

static NovaValue nova_sqrt(NovaValue value) {
  if (value.tag != NOVA_NUMBER || value.number < 0) return nova_null();
  return nova_num(sqrt(value.number));
}

static NovaValue nova_env(NovaValue key) {
  if (key.tag != NOVA_STRING || !key.string) return nova_null();
  const char* value = getenv(key.string);
  if (!value) return nova_null();
  return nova_string(nova_dup(value));
}

static NovaValue nova_array(NovaValue* args, size_t argc);

static void nova_json_skip_ws(const char** cursor) {
  const char* p = *cursor;
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  *cursor = p;
}

static int nova_json_hex(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

static void nova_json_append_utf8(char* out, size_t* len, uint32_t code) {
  if (code <= 0x7f) {
    out[(*len)++] = (char)code;
  } else if (code <= 0x7ff) {
    out[(*len)++] = (char)(0xc0 | (code >> 6));
    out[(*len)++] = (char)(0x80 | (code & 0x3f));
  } else if (code <= 0xffff) {
    out[(*len)++] = (char)(0xe0 | (code >> 12));
    out[(*len)++] = (char)(0x80 | ((code >> 6) & 0x3f));
    out[(*len)++] = (char)(0x80 | (code & 0x3f));
  } else if (code <= 0x10ffff) {
    out[(*len)++] = (char)(0xf0 | (code >> 18));
    out[(*len)++] = (char)(0x80 | ((code >> 12) & 0x3f));
    out[(*len)++] = (char)(0x80 | ((code >> 6) & 0x3f));
    out[(*len)++] = (char)(0x80 | (code & 0x3f));
  }
}

static NovaValue nova_json_parse_value(const char** cursor);

static NovaValue nova_json_parse_string(const char** cursor) {
  const char* p = *cursor;
  if (*p != '"') return nova_null();
  p++;

  size_t cap = 32;
  size_t len = 0;
  char* out = (char*)calloc(cap, 1);
  if (!out) return nova_null();

  while (*p && *p != '"') {
    uint32_t code = (unsigned char)*p++;
    if (code == '\\') {
      char esc = *p++;
      switch (esc) {
        case '"': code = '"'; break;
        case '\\': code = '\\'; break;
        case '/': code = '/'; break;
        case 'b': code = 8; break;
        case 'f': code = 12; break;
        case 'n': code = 10; break;
        case 'r': code = 13; break;
        case 't': code = 9; break;
        case 'u': {
          int h0 = nova_json_hex(*p++);
          int h1 = nova_json_hex(*p++);
          int h2 = nova_json_hex(*p++);
          int h3 = nova_json_hex(*p++);
          if (h0 < 0 || h1 < 0 || h2 < 0 || h3 < 0) {
            free(out);
            return nova_null();
          }
          code = (uint32_t)((h0 << 12) | (h1 << 8) | (h2 << 4) | h3);
          break;
        }
        default:
          free(out);
          return nova_null();
      }
    }

    if (len + 5 >= cap) {
      cap *= 2;
      char* next = (char*)realloc(out, cap);
      if (!next) {
        free(out);
        return nova_null();
      }
      out = next;
    }

    if (code >= 0xd800 && code <= 0xdfff) {
      free(out);
      return nova_null();
    }
    nova_json_append_utf8(out, &len, code);
  }

  if (*p != '"') {
    free(out);
    return nova_null();
  }
  p++;
  out[len] = '\0';
  *cursor = p;
  return nova_string(out);
}

static NovaValue nova_json_parse_array(const char** cursor) {
  const char* p = *cursor;
  if (*p != '[') return nova_null();
  p++;

  size_t len = 0;
  NovaValue* items = NULL;
  nova_json_skip_ws(&p);

  if (*p == ']') {
    *cursor = p + 1;
    return nova_array(NULL, 0);
  }

  while (*p) {
    NovaValue item = nova_json_parse_value(&p);
    NovaValue* next = (NovaValue*)realloc(items, (len + 1) * sizeof(NovaValue));
    if (!next) {
      free(items);
      return nova_null();
    }
    items = next;
    items[len++] = item;

    nova_json_skip_ws(&p);
    if (*p == ']') {
      p++;
      NovaValue result = nova_array(items, len);
      free(items);
      *cursor = p;
      return result;
    }
    if (*p != ',') {
      free(items);
      return nova_null();
    }
    p++;
    nova_json_skip_ws(&p);
  }

  free(items);
  return nova_null();
}

static NovaValue nova_json_parse_object(const char** cursor) {
  const char* p = *cursor;
  if (*p != '{') return nova_null();
  p++;

  NovaMap* map = (NovaMap*)calloc(1, sizeof(NovaMap));
  if (!map) return nova_null();

  nova_json_skip_ws(&p);
  if (*p == '}') {
    p++;
    NovaValue result = nova_map_value(map);
    *cursor = p;
    return result;
  }

  while (*p) {
    NovaValue key = nova_json_parse_string(&p);
    if (key.tag != NOVA_STRING) {
      free(map->keys);
      free(map->values);
      free(map);
      return nova_null();
    }

    nova_json_skip_ws(&p);
    if (*p != ':') {
      free(map->keys);
      free(map->values);
      free(map);
      return nova_null();
    }
    p++;
    nova_json_skip_ws(&p);

    NovaValue value = nova_json_parse_value(&p);
    size_t next_len = map->len + 1;
    NovaValue* keys = (NovaValue*)realloc(map->keys, next_len * sizeof(NovaValue));
    if (!keys) {
      free(map->keys);
      free(map->values);
      free(map);
      return nova_null();
    }
    map->keys = keys;
    NovaValue* values = (NovaValue*)realloc(map->values, next_len * sizeof(NovaValue));
    if (!values) {
      free(map->keys);
      free(map);
      return nova_null();
    }
    map->values = values;
    map->keys[map->len] = key;
    map->values[map->len] = value;
    map->len = next_len;

    nova_json_skip_ws(&p);
    if (*p == '}') {
      p++;
      NovaValue result = nova_map_value(map);
      *cursor = p;
      return result;
    }
    if (*p != ',') {
      free(map->keys);
      free(map->values);
      free(map);
      return nova_null();
    }
    p++;
    nova_json_skip_ws(&p);
  }

  free(map->keys);
  free(map->values);
  free(map);
  return nova_null();
}

static NovaValue nova_json_parse_value(const char** cursor) {
  const char* p = *cursor;
  nova_json_skip_ws(&p);

  if (*p == '"') {
    *cursor = p;
    return nova_json_parse_string(cursor);
  }
  if (*p == '[') {
    *cursor = p;
    return nova_json_parse_array(cursor);
  }
  if (*p == '{') {
    *cursor = p;
    return nova_json_parse_object(cursor);
  }
  if (strncmp(p, "true", 4) == 0) {
    *cursor = p + 4;
    return nova_bool(1);
  }
  if (strncmp(p, "false", 5) == 0) {
    *cursor = p + 5;
    return nova_bool(0);
  }
  if (strncmp(p, "null", 4) == 0) {
    *cursor = p + 4;
    return nova_null();
  }

  char* end = NULL;
  double number = strtod(p, &end);
  if (end == p) return nova_null();
  *cursor = end;
  return nova_num(number);
}

static NovaValue nova_json_parse(NovaValue source) {
  if (source.tag != NOVA_STRING || !source.string) return nova_null();
  const char* cursor = source.string;
  NovaValue value = nova_json_parse_value(&cursor);
  nova_json_skip_ws(&cursor);
  if (*cursor != '\0') return nova_null();
  return value;
}

typedef struct {
  char* data;
  size_t len;
  size_t cap;
} NovaJsonBuffer;

static int nova_json_buf_reserve(NovaJsonBuffer* buf, size_t extra) {
  size_t required = buf->len + extra + 1;
  if (required <= buf->cap) return 1;
  size_t cap = buf->cap ? buf->cap : 64;
  while (cap < required) cap *= 2;
  char* next = (char*)realloc(buf->data, cap);
  if (!next) return 0;
  buf->data = next;
  buf->cap = cap;
  return 1;
}

static int nova_json_buf_push(NovaJsonBuffer* buf, char c) {
  if (!nova_json_buf_reserve(buf, 1)) return 0;
  buf->data[buf->len++] = c;
  buf->data[buf->len] = '\0';
  return 1;
}

static int nova_json_buf_text(NovaJsonBuffer* buf, const char* text) {
  size_t len = strlen(text);
  if (!nova_json_buf_reserve(buf, len)) return 0;
  memcpy(buf->data + buf->len, text, len);
  buf->len += len;
  buf->data[buf->len] = '\0';
  return 1;
}

static int nova_json_stringify_value(NovaJsonBuffer* buf, NovaValue value);

static int nova_json_stringify_string(NovaJsonBuffer* buf, const char* text) {
  if (!nova_json_buf_push(buf, '"')) return 0;
  for (const unsigned char* p = (const unsigned char*)(text ? text : ""); *p; p++) {
    switch (*p) {
      case '"': if (!nova_json_buf_text(buf, "\\\"")) return 0; break;
      case '\\': if (!nova_json_buf_text(buf, "\\\\")) return 0; break;
      case '\n': if (!nova_json_buf_text(buf, "\\n")) return 0; break;
      case '\r': if (!nova_json_buf_text(buf, "\\r")) return 0; break;
      case '\t': if (!nova_json_buf_text(buf, "\\t")) return 0; break;
      default: if (!nova_json_buf_push(buf, (char)*p)) return 0; break;
    }
  }
  return nova_json_buf_push(buf, '"');
}

static int nova_json_stringify_value(NovaJsonBuffer* buf, NovaValue value) {
  switch (value.tag) {
    case NOVA_NULL:
      return nova_json_buf_text(buf, "null");
    case NOVA_BOOL:
      return nova_json_buf_text(buf, value.number != 0 ? "true" : "false");
    case NOVA_NUMBER: {
      char number[64];
      snprintf(number, sizeof(number), "%.17g", value.number);
      return nova_json_buf_text(buf, number);
    }
    case NOVA_STRING:
      return nova_json_stringify_string(buf, value.string);
    case NOVA_ARRAY:
      if (!nova_json_buf_push(buf, '[')) return 0;
      for (size_t i = 0; i < (value.array ? value.array->len : 0); i++) {
        if (i && !nova_json_buf_push(buf, ',')) return 0;
        if (!nova_json_stringify_value(buf, value.array->items[i])) return 0;
      }
      return nova_json_buf_push(buf, ']');
    case NOVA_MAP:
      if (!nova_json_buf_push(buf, '{')) return 0;
      for (size_t i = 0; i < (value.map ? value.map->len : 0); i++) {
        if (i && !nova_json_buf_push(buf, ',')) return 0;
        if (value.map->keys[i].tag != NOVA_STRING) return 0;
        if (!nova_json_stringify_string(buf, value.map->keys[i].string)) return 0;
        if (!nova_json_buf_push(buf, ':')) return 0;
        if (!nova_json_stringify_value(buf, value.map->values[i])) return 0;
      }
      return nova_json_buf_push(buf, '}');
    default:
      return 0;
  }
}

static NovaValue nova_json_stringify(NovaValue value) {
  NovaJsonBuffer buf = {0};
  if (!nova_json_stringify_value(&buf, value)) {
    free(buf.data);
    return nova_null();
  }
  return nova_string(buf.data ? buf.data : nova_dup(""));
}

static NovaValue nova_read_file(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  FILE* file = fopen(path.string, "rb");
  if (!file) return nova_null();
  if (fseek(file, 0, SEEK_END) != 0) { fclose(file); return nova_null(); }
  long size = ftell(file);
  if (size < 0) { fclose(file); return nova_null(); }
  rewind(file);
  char* data = (char*)calloc((size_t)size + 1, 1);
  if (!data) { fclose(file); return nova_null(); }
  size_t read = fread(data, 1, (size_t)size, file);
  fclose(file);
  data[read] = '\0';
  return nova_string(data);
}

static NovaValue nova_write_file(NovaValue path, NovaValue data) {
  if (path.tag != NOVA_STRING || data.tag != NOVA_STRING || !path.string || !data.string) return nova_null();
  FILE* file = fopen(path.string, "wb");
  if (!file) return nova_null();
  size_t len = strlen(data.string);
  size_t written = fwrite(data.string, 1, len, file);
  int ok = written == len && fflush(file) == 0;
  fclose(file);
  return nova_null();
}

static NovaValue nova_exists(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_bool(0);
  FILE* file = fopen(path.string, "rb");
  if (!file) return nova_bool(0);
  fclose(file);
  return nova_bool(1);
}

static NovaValue nova_current_dir(void) {
  size_t size = 256;
  for (;;) {
    char* buffer = (char*)calloc(size, 1);
    if (!buffer) return nova_null();
    if (getcwd(buffer, size)) return nova_string(buffer);
    free(buffer);
    if (errno != ERANGE || size > (1u << 20)) return nova_null();
    size *= 2;
  }
}

static NovaValue nova_path_join(NovaValue left, NovaValue right) {
  if (left.tag != NOVA_STRING || right.tag != NOVA_STRING) return nova_null();
  const char* a = left.string ? left.string : "";
  const char* b = right.string ? right.string : "";
  size_t alen = strlen(a);
  size_t blen = strlen(b);
  int slash = alen > 0 && a[alen - 1] != '/';
  char* out = (char*)calloc(alen + blen + (size_t)slash + 1, 1);
  if (!out) return nova_null();
  memcpy(out, a, alen);
  if (slash) out[alen++] = '/';
  memcpy(out + alen, b, blen);
  return nova_string(out);
}

static NovaValue nova_path_basename(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  const char* slash = strrchr(path.string, '/');
  return nova_string(nova_dup(slash ? slash + 1 : path.string));
}

static NovaValue nova_path_dirname(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  const char* slash = strrchr(path.string, '/');
  if (!slash) return nova_string(nova_dup(""));
  size_t len = (size_t)(slash - path.string);
  if (len == 0) len = 1;
  char* out = (char*)calloc(len + 1, 1);
  if (!out) return nova_null();
  memcpy(out, path.string, len);
  return nova_string(out);
}

static NovaValue nova_path_ext(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  const char* base = strrchr(path.string, '/');
  base = base ? base + 1 : path.string;
  const char* dot = strrchr(base, '.');
  if (!dot || dot == base) return nova_string(nova_dup(""));
  return nova_string(nova_dup(dot + 1));
}

static NovaValue nova_path_stem(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  const char* base = strrchr(path.string, '/');
  base = base ? base + 1 : path.string;
  const char* dot = strrchr(base, '.');
  size_t len = dot && dot != base ? (size_t)(dot - base) : strlen(base);
  char* out = (char*)calloc(len + 1, 1);
  if (!out) return nova_null();
  memcpy(out, base, len);
  return nova_string(out);
}

static NovaValue nova_make_dir(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  if (mkdir(path.string, 0777) == 0 || errno == EEXIST) return nova_null();
  return nova_null();
}

static NovaValue nova_remove_file(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  remove(path.string);
  return nova_null();
}

static int nova_string_cmp(const void* left, const void* right) {
  const char* a = *(const char* const*)left;
  const char* b = *(const char* const*)right;
  return strcmp(a, b);
}

static NovaValue nova_list_dir(NovaValue path) {
  if (path.tag != NOVA_STRING || !path.string) return nova_null();
  DIR* dir = opendir(path.string);
  if (!dir) return nova_null();

  size_t len = 0;
  size_t cap = 16;
  char** names = (char**)calloc(cap, sizeof(char*));
  if (!names) { closedir(dir); return nova_null(); }

  struct dirent* entry;
  while ((entry = readdir(dir)) != NULL) {
    if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) continue;
    if (len == cap) {
      cap *= 2;
      char** next = (char**)realloc(names, cap * sizeof(char*));
      if (!next) {
        for (size_t i = 0; i < len; i++) free(names[i]);
        free(names);
        closedir(dir);
        return nova_null();
      }
      names = next;
    }
    names[len] = nova_dup(entry->d_name);
    if (!names[len]) {
      for (size_t i = 0; i < len; i++) free(names[i]);
      free(names);
      closedir(dir);
      return nova_null();
    }
    len++;
  }
  closedir(dir);

  qsort(names, len, sizeof(char*), nova_string_cmp);
  NovaArray* array = (NovaArray*)calloc(1, sizeof(NovaArray));
  if (!array) {
    for (size_t i = 0; i < len; i++) free(names[i]);
    free(names);
    return nova_null();
  }
  array->len = len;
  array->items = len ? (NovaValue*)calloc(len, sizeof(NovaValue)) : NULL;
  if (len && !array->items) {
    for (size_t i = 0; i < len; i++) free(names[i]);
    free(names);
    free(array);
    return nova_null();
  }
  for (size_t i = 0; i < len; i++) {
    array->items[i] = nova_string(names[i]);
  }
  free(names);
  return nova_array_value(array);
}

static NovaValue nova_now_ms(void) {
  struct timespec ts;
  if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return nova_null();
  return nova_num((double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1000000.0);
}

static NovaValue nova_now_s(void) {
  struct timespec ts;
  if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return nova_null();
  return nova_num((double)ts.tv_sec + (double)ts.tv_nsec / 1000000000.0);
}

static NovaValue nova_sleep_ms(NovaValue value) {
  if (value.tag != NOVA_NUMBER || value.number < 0) return nova_null();
  struct timespec ts;
  ts.tv_sec = (time_t)(value.number / 1000.0);
  ts.tv_nsec = (long)((value.number - (double)ts.tv_sec * 1000.0) * 1000000.0);
  nanosleep(&ts, NULL);
  return nova_null();
}

static NovaValue nova_push(NovaValue array, NovaValue item) {
  if (array.tag != NOVA_ARRAY || !array.array) return nova_null();
  size_t next = array.array->len + 1;
  NovaValue* items = (NovaValue*)realloc(array.array->items, next * sizeof(NovaValue));
  if (!items) return nova_null();
  array.array->items = items;
  array.array->items[array.array->len] = item;
  array.array->len = next;
  return array;
}

static NovaValue nova_pop(NovaValue array) {
  if (array.tag != NOVA_ARRAY || !array.array || array.array->len == 0) {
    return nova_enum_value("Option", "None", nova_null());
  }
  NovaValue item = array.array->items[array.array->len - 1];
  array.array->len--;
  return nova_enum_value("Option", "Some", item);
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

static NovaValue nova_range(NovaValue start, NovaValue end) {
  if (start.tag != NOVA_NUMBER || end.tag != NOVA_NUMBER) return nova_null();
  int64_t first = (int64_t)start.number;
  int64_t last = (int64_t)end.number;
  if ((double)first != start.number || (double)last != end.number) return nova_null();
  if (last <= first) return nova_array(NULL, 0);

  size_t len = (size_t)(last - first);
  NovaArray* array = (NovaArray*)calloc(1, sizeof(NovaArray));
  if (!array) return nova_null();
  array->len = len;
  array->items = (NovaValue*)calloc(len, sizeof(NovaValue));
  if (!array->items) return nova_null();
  for (size_t i = 0; i < len; i++) {
    array->items[i] = nova_num((double)(first + (int64_t)i));
  }
  return nova_array_value(array);
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

static size_t nova_utf8_width(unsigned char c) {
  if ((c & 0x80u) == 0) return 1;
  if ((c & 0xe0u) == 0xc0u) return 2;
  if ((c & 0xf0u) == 0xe0u) return 3;
  if ((c & 0xf8u) == 0xf0u) return 4;
  return 1;
}

static size_t nova_utf8_length(const char* text) {
  if (!text) return 0;
  size_t len = strlen(text);
  size_t count = 0;
  size_t offset = 0;
  while (offset < len) {
    size_t width = nova_utf8_width((unsigned char)text[offset]);
    if (offset + width > len) width = 1;
    offset += width;
    count++;
  }
  return count;
}

static NovaValue nova_string_index(NovaValue value, NovaValue index) {
  if (value.tag != NOVA_STRING || !value.string || index.tag != NOVA_NUMBER) return nova_null();
  if (index.number < 0) return nova_null();
  size_t integral = (size_t)index.number;
  if ((double)integral != index.number) return nova_null();

  size_t target = (size_t)index.number;
  size_t byte_len = strlen(value.string);
  size_t offset = 0;
  size_t current = 0;
  while (offset < byte_len) {
    size_t width = nova_utf8_width((unsigned char)value.string[offset]);
    if (offset + width > byte_len) width = 1;
    if (current == target) {
      char* out = (char*)calloc(width + 1, 1);
      if (!out) return nova_null();
      memcpy(out, value.string + offset, width);
      return nova_string(out);
    }
    offset += width;
    current++;
  }
  return nova_null();
}

static NovaValue nova_iter(NovaValue value) {
  NovaIterator* out = (NovaIterator*)calloc(1, sizeof(NovaIterator));
  if (!out) return nova_null();

  if (value.tag == NOVA_ITERATOR && value.iterator) {
    size_t remaining = value.iterator->len - value.iterator->index;
    out->len = remaining;
    out->items = remaining ? (NovaValue*)calloc(remaining, sizeof(NovaValue)) : NULL;
    if (remaining && !out->items) return nova_null();
    for (size_t i = 0; i < remaining; i++) {
      out->items[i] = value.iterator->items[value.iterator->index + i];
    }
    value.iterator->index = value.iterator->len;
    return nova_iterator_value(out);
  }

  if (value.tag == NOVA_ARRAY && value.array) {
    out->len = value.array->len;
    out->items = out->len ? (NovaValue*)calloc(out->len, sizeof(NovaValue)) : NULL;
    if (out->len && !out->items) return nova_null();
    for (size_t i = 0; i < out->len; i++) out->items[i] = value.array->items[i];
    return nova_iterator_value(out);
  }

  if (value.tag == NOVA_STRING && value.string) {
    size_t bytes = strlen(value.string);
    out->items = bytes ? (NovaValue*)calloc(bytes, sizeof(NovaValue)) : NULL;
    if (bytes && !out->items) return nova_null();
    size_t offset = 0;
    while (offset < bytes) {
      size_t width = nova_utf8_width((unsigned char)value.string[offset]);
      if (offset + width > bytes) width = 1;
      char* ch = (char*)calloc(width + 1, 1);
      if (!ch) return nova_null();
      memcpy(ch, value.string + offset, width);
      out->items[out->len++] = nova_string(ch);
      offset += width;
    }
    return nova_iterator_value(out);
  }

  return nova_null();
}

static NovaValue nova_next(NovaValue value) {
  if (value.tag != NOVA_ITERATOR || !value.iterator) return nova_null();
  if (value.iterator->index >= value.iterator->len) {
    return nova_enum_value("Option", "None", nova_null());
  }
  NovaValue item = value.iterator->items[value.iterator->index++];
  return nova_enum_value("Option", "Some", item);
}

static NovaValue nova_has_next(NovaValue value) {
  if (value.tag != NOVA_ITERATOR || !value.iterator) return nova_bool(0);
  return nova_bool(value.iterator->index < value.iterator->len);
}

static NovaValue nova_collect(NovaValue value) {
  if (value.tag != NOVA_ITERATOR || !value.iterator) return nova_null();
  size_t remaining = value.iterator->len - value.iterator->index;
  NovaValue* items = remaining ? (NovaValue*)calloc(remaining, sizeof(NovaValue)) : NULL;
  if (remaining && !items) return nova_null();
  for (size_t i = 0; i < remaining; i++) {
    items[i] = value.iterator->items[value.iterator->index + i];
  }
  value.iterator->index = value.iterator->len;
  NovaArray* array = (NovaArray*)calloc(1, sizeof(NovaArray));
  if (!array) return nova_null();
  array->len = remaining;
  array->items = items;
  return nova_array_value(array);
}

static NovaValue nova_index(NovaValue base, NovaValue index) {
  if (base.tag == NOVA_STRING) return nova_string_index(base, index);
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

static NovaValue nova_map_get(NovaValue map, NovaValue key) {
  if (map.tag != NOVA_MAP || !map.map) return nova_null();
  for (size_t i = 0; i < map.map->len; i++) {
    if (nova_equal(map.map->keys[i], key)) return map.map->values[i];
  }
  return nova_null();
}

static NovaValue nova_map_has(NovaValue map, NovaValue key) {
  if (map.tag != NOVA_MAP || !map.map) return nova_bool(0);
  for (size_t i = 0; i < map.map->len; i++) {
    if (nova_equal(map.map->keys[i], key)) return nova_bool(1);
  }
  return nova_bool(0);
}

static NovaValue nova_map_set(NovaValue map, NovaValue key, NovaValue value) {
  if (map.tag != NOVA_MAP || !map.map) return nova_null();
  for (size_t i = 0; i < map.map->len; i++) {
    if (nova_equal(map.map->keys[i], key)) {
      map.map->values[i] = value;
      return nova_null();
    }
  }
  size_t next = map.map->len + 1;
  NovaValue* keys = (NovaValue*)realloc(map.map->keys, next * sizeof(NovaValue));
  if (!keys) return nova_null();
  NovaValue* values = (NovaValue*)realloc(map.map->values, next * sizeof(NovaValue));
  if (!values) {
    map.map->keys = keys;
    return nova_null();
  }
  map.map->keys = keys;
  map.map->values = values;
  map.map->keys[map.map->len] = key;
  map.map->values[map.map->len] = value;
  map.map->len = next;
  return nova_null();
}

static NovaValue nova_map_remove(NovaValue map, NovaValue key) {
  if (map.tag != NOVA_MAP || !map.map) return nova_null();
  for (size_t i = 0; i < map.map->len; i++) {
    if (nova_equal(map.map->keys[i], key)) {
      for (size_t j = i + 1; j < map.map->len; j++) {
        map.map->keys[j - 1] = map.map->keys[j];
        map.map->values[j - 1] = map.map->values[j];
      }
      map.map->len--;
      return nova_null();
    }
  }
  return nova_null();
}

static NovaValue nova_set_add(NovaValue set, NovaValue item) {
  if (set.tag != NOVA_SET || !set.set) return nova_null();
  for (size_t i = 0; i < set.set->len; i++) {
    if (nova_equal(set.set->items[i], item)) return nova_null();
  }
  size_t next = set.set->len + 1;
  NovaValue* items = (NovaValue*)realloc(set.set->items, next * sizeof(NovaValue));
  if (!items) return nova_null();
  set.set->items = items;
  set.set->items[set.set->len] = item;
  set.set->len = next;
  return nova_null();
}

static NovaValue nova_set_has(NovaValue set, NovaValue item) {
  if (set.tag != NOVA_SET || !set.set) return nova_bool(0);
  for (size_t i = 0; i < set.set->len; i++) {
    if (nova_equal(set.set->items[i], item)) return nova_bool(1);
  }
  return nova_bool(0);
}

static NovaValue nova_set_remove(NovaValue set, NovaValue item) {
  if (set.tag != NOVA_SET || !set.set) return nova_null();
  for (size_t i = 0; i < set.set->len; i++) {
    if (nova_equal(set.set->items[i], item)) {
      for (size_t j = i + 1; j < set.set->len; j++) {
        set.set->items[j - 1] = set.set->items[j];
      }
      set.set->len--;
      return nova_null();
    }
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
    case NOVA_ITERATOR: return v.iterator && v.iterator->index < v.iterator->len;
    default: return 0;
  }
}

static int nova_equal(NovaValue a, NovaValue b) {
  if (a.tag != b.tag) return 0;

  switch (a.tag) {
    case NOVA_NULL:
      return 1;

    case NOVA_NUMBER:
    case NOVA_BOOL:
      return a.number == b.number;

    case NOVA_STRING:
      return strcmp(a.string ? a.string : "", b.string ? b.string : "") == 0;

    case NOVA_ENUM:
      if (!a.enumeration || !b.enumeration) return a.enumeration == b.enumeration;
      return strcmp(a.enumeration->name, b.enumeration->name) == 0
          && strcmp(a.enumeration->variant, b.enumeration->variant) == 0
          && nova_equal(a.enumeration->payload, b.enumeration->payload);

    case NOVA_STRUCT:
      if (!a.structure || !b.structure) return a.structure == b.structure;
      if (strcmp(a.structure->name ? a.structure->name : "",
                 b.structure->name ? b.structure->name : "") != 0
          || a.structure->len != b.structure->len) {
        return 0;
      }
      for (size_t i = 0; i < a.structure->len; i++) {
        if (strcmp(a.structure->fields[i].name ? a.structure->fields[i].name : "",
                   b.structure->fields[i].name ? b.structure->fields[i].name : "") != 0
            || !nova_equal(a.structure->fields[i].value, b.structure->fields[i].value)) {
          return 0;
        }
      }
      return 1;

    case NOVA_ARRAY:
      if (!a.array || !b.array) return a.array == b.array;
      if (a.array->len != b.array->len) return 0;
      for (size_t i = 0; i < a.array->len; i++) {
        if (!nova_equal(a.array->items[i], b.array->items[i])) return 0;
      }
      return 1;

    case NOVA_MAP:
      if (!a.map || !b.map) return a.map == b.map;
      if (a.map->len != b.map->len) return 0;
      for (size_t i = 0; i < a.map->len; i++) {
        int found = 0;
        for (size_t j = 0; j < b.map->len; j++) {
          if (nova_equal(a.map->keys[i], b.map->keys[j])
              && nova_equal(a.map->values[i], b.map->values[j])) {
            found = 1;
            break;
          }
        }
        if (!found) return 0;
      }
      return 1;

    case NOVA_SET:
      if (!a.set || !b.set) return a.set == b.set;
      if (a.set->len != b.set->len) return 0;
      for (size_t i = 0; i < a.set->len; i++) {
        int found = 0;
        for (size_t j = 0; j < b.set->len; j++) {
          if (nova_equal(a.set->items[i], b.set->items[j])) {
            found = 1;
            break;
          }
        }
        if (!found) return 0;
      }
      return 1;

    case NOVA_CLOSURE:
      return a.closure == b.closure;

    case NOVA_ITERATOR:
      return a.iterator == b.iterator;

    default:
      return 0;
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
