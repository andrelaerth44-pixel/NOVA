use crate::ir::{Function, Instr, Module};

fn c_ident(name: &str) -> String {
    let mut out = String::from("nova_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' { out.push(ch); } else { out.push('_'); }
    }
    out
}

fn emit_code(code: &[Instr], current_fn: Option<&Function>) -> Result<String, String> {
    let mut stack = 0usize;
    let mut max_stack = 0usize;
    for ins in code {
        match ins {
            Instr::ConstNumber(_) | Instr::ConstBool(_) | Instr::ConstNull | Instr::Load(_) => stack += 1,
            Instr::StructInit { .. } | Instr::FieldGet { .. } => return Err("native C backend: struct code generation is not implemented yet".into()),
            Instr::ConstString(_) => return Err("native C backend currently supports numeric/bool IR only".into()),
            Instr::Store(_) | Instr::Pop => { if stack == 0 { return Err("IR stack underflow".into()); } stack -= 1; }
            Instr::Unary { .. } => {}
            Instr::Binary { .. } => { if stack < 2 { return Err("IR stack underflow at Binary".into()); } stack -= 1; }
            Instr::Call { name, argc, result } => {
                if stack < *argc { return Err(format!("IR stack underflow at Call {}", name)); }
                stack -= *argc;
                if !matches!(result, crate::ir::IrType::Null) { stack += 1; }
            }
            Instr::Jump(_) => {}
            Instr::JumpIfFalse(_) => { if stack == 0 { return Err("IR stack underflow at JumpIfFalse".into()); } stack -= 1; }
            Instr::Return(_) => { if stack == 0 { return Err("IR stack underflow at Return".into()); } stack -= 1; }
        }
        max_stack = max_stack.max(stack);
    }
    if stack != 0 && current_fn.is_some() { return Err("native C backend requires balanced function stack".into()); }

    let slots = max_stack.max(1);
    let mut c = format!("  double stack[{}];
  int sp = 0;
", slots + 8);
    for (i, ins) in code.iter().enumerate() {
        c.push_str(&format!("L{}: ;
", i));
        match ins {
            Instr::ConstNumber(n) => c.push_str(&format!("  stack[sp++] = {:.17};
", n)),
            Instr::ConstBool(b) => c.push_str(&format!("  stack[sp++] = {};
", if *b {1} else {0})),
            Instr::ConstNull => c.push_str("  stack[sp++] = 0;
"),
            Instr::ConstString(_) => return Err("native C backend does not support strings yet".into()),
            Instr::StructInit { .. } | Instr::FieldGet { .. } => return Err("native C backend: struct code generation is not implemented yet".into()),
            Instr::Load(n) => c.push_str(&format!("  stack[sp++] = {};
", c_ident(n))),
            Instr::Store(n) => c.push_str(&format!("  {} = stack[--sp];
", c_ident(n))),
            Instr::Unary { op, .. } => {
                let expr = match op.as_str() {
                    "Minus" => "-a",
                    "Bang" => "(a==0)",
                    other => return Err(format!("native C backend does not support unary {}", other)),
                };
                c.push_str(&format!("  {{ double a=stack[--sp]; stack[sp++] = {}; }}
", expr));
            }
            Instr::Binary { op, .. } => {
                let expr = match op.as_str() {
                    "Plus" => "a+b", "Minus" => "a-b", "Star" => "a*b", "Slash" => "a/b", "Percent" => "fmod(a,b)",
                    "EqEq" => "a==b", "NotEq" => "a!=b", "Lt" => "a<b", "Le" => "a<=b", "Gt" => "a>b", "Ge" => "a>=b",
                    "And" => "(a!=0 && b!=0)", "Or" => "(a!=0 || b!=0)",
                    other => return Err(format!("native C backend does not support operator {}", other)),
                };
                c.push_str(&format!("  {{ double b=stack[--sp]; double a=stack[--sp]; stack[sp++] = {}; }}
", expr));
            }
            Instr::Call { name, .. } if name == "try" => return Err("native C backend: Option/Result try propagation is currently VM/SSA only".into()),
            Instr::Call { name, argc: 1, .. } if name == "print" => c.push_str(r#"  printf("%.15g\n", stack[--sp]);\n"#),
            Instr::Call { name, argc, result } => {
                if name == "array" || name == "for_each" { return Err(format!("native C backend does not support builtin {}", name)); }
                let args: Vec<String> = (0..*argc).map(|i| format!("stack[sp-{}]", argc-i)).collect();
                c.push_str(&format!("  {{ double r = {}({}); sp -= {}; ", c_ident(name), args.join(", "), argc));
                if !matches!(result, crate::ir::IrType::Null) { c.push_str("stack[sp++] = r; "); }
                c.push_str("}
");
            }
            Instr::Jump(t) => c.push_str(&format!("  goto L{};
", t)),
            Instr::JumpIfFalse(t) => c.push_str(&format!("  if (stack[--sp] == 0) goto L{};
", t)),
            Instr::Return(_) => c.push_str("  return stack[--sp];
"),
            Instr::Pop => c.push_str("  --sp;
"),
        }
    }
    if current_fn.is_none() { c.push_str("  return 0;
"); }
    Ok(c)
}

fn flatten(blocks: &[crate::ir::BasicBlock]) -> Vec<Instr> {
    blocks.iter().flat_map(|b| b.code.clone()).collect()
}

pub fn emit_c(m: &Module) -> Result<String, String> {
    let mut out = String::from("#include <stdio.h>\n#include <math.h>\n\n");
    for f in &m.functions {
        out.push_str(&format!("static double {}(", c_ident(&f.name)));
        for (i, (p, _)) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("double {}", c_ident(p)));
        }
        out.push_str(");\n");
    }
    let main_code = flatten(&m.blocks);
    let mut globals = std::collections::BTreeSet::new();
    for ins in &main_code {
        if let Instr::Load(n) | Instr::Store(n) = ins { globals.insert(c_ident(n)); }
    }
    for g in globals { out.push_str(&format!("static double {} = 0;\n", g)); }
    for f in &m.functions {
        out.push_str(&format!("\nstatic double {}(", c_ident(&f.name)));
        for (i, (p, _)) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("double {}", c_ident(p)));
        }
        out.push_str(") {\n");
        let code = flatten(&f.blocks);
        out.push_str(&emit_code(&code, Some(f))?);
        out.push_str("}\n");
    }
    out.push_str("\nint main(void) {\n");
    out.push_str(&emit_code(&main_code, None)?);
    out.push_str("}\n");
    Ok(out)
}
