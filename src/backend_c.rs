use crate::ir::{Function, Instr, Module};

fn c_ident(name: &str) -> String {
    let mut out = String::from("nova_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' { out.push(ch); } else { out.push('_'); }
    }
    out
}

fn emit_code(code: &[Instr], current_fn: Option<&Function>) -> Result<String, String> {
    let mut out = String::new();
    let mut stack = 0usize;
    let mut max_stack = 0usize;
    for ins in code {
        match ins {
            Instr::ConstNumber(_) | Instr::ConstBool(_) => { stack += 1; }
            Instr::ConstString(_) => return Err("native C backend currently requires numeric/bool IR; strings need the managed runtime backend".into()),
            Instr::Load(_) => stack += 1,
            Instr::Store(_) => { if stack == 0 { return Err("IR stack underflow at Store".into()); } stack -= 1; }
            Instr::Binary(op) => {
                if stack < 2 { return Err(format!("IR stack underflow at Binary {}", op)); }
                stack -= 1;
            }
            Instr::Call(name, argc) => {
                if stack < *argc { return Err(format!("IR stack underflow at Call {}", name)); }
                if name == "print" { stack -= argc; }
                else if name == "array" || name == "for_each" { return Err(format!("native C backend does not support runtime builtin {}", name)); }
                else { stack = stack - argc + 1; }
            }
            Instr::Jump(_) => {}
            Instr::JumpIfFalse(_) => { if stack == 0 { return Err("IR stack underflow at JumpIfFalse".into()); } stack -= 1; }
            Instr::Return => { if stack == 0 { return Err("IR stack underflow at Return".into()); } stack -= 1; }
            Instr::Pop => { if stack == 0 { return Err("IR stack underflow at Pop".into()); } stack -= 1; }
        }
        max_stack = max_stack.max(stack);
    }
    if stack != 0 && current_fn.is_some() {
        return Err("native C backend requires balanced function stack".into());
    }

    let slots = max_stack.max(1);
    let mut c = format!("  double stack[{}];\n  int sp = 0;\n", slots + 4);
    for (i, ins) in code.iter().enumerate() {
        c.push_str(&format!("L{}: ;\n", i));
        match ins {
            Instr::ConstNumber(n) => c.push_str(&format!("  stack[sp++] = {:.17};\n", n)),
            Instr::ConstBool(b) => c.push_str(&format!("  stack[sp++] = {};\n", if *b {1} else {0})),
            Instr::ConstString(_) => unreachable!(),
            Instr::Load(n) => c.push_str(&format!("  stack[sp++] = {};\n", c_ident(n))),
            Instr::Store(n) => c.push_str(&format!("  {} = stack[--sp];\n", c_ident(n))),
            Instr::Binary(op) => {
                let expr = match op.as_str() {
                    "Plus" => "a+b", "Minus" => "a-b", "Star" => "a*b", "Slash" => "a/b", "Percent" => "fmod(a,b)",
                    "EqEq" => "a==b", "Ne" => "a!=b", "Lt" => "a<b", "Le" => "a<=b", "Gt" => "a>b", "Ge" => "a>=b",
                    "And" => "(a!=0 && b!=0)", "Or" => "(a!=0 || b!=0)",
                    other => return Err(format!("native C backend does not support unary/operator {}", other)),
                };
                c.push_str(&format!("  {{ double b=stack[--sp]; double a=stack[--sp]; stack[sp++] = {}; }}\n", expr));
            }
            Instr::Call(name, argc) if name == "print" && *argc == 1 => c.push_str("  printf("%.15g\\n", stack[--sp]);\n"),
            Instr::Call(name, argc) => {
                let args: Vec<String> = (0..*argc).map(|i| format!("stack[sp-{}]", argc-i)).collect();
                c.push_str(&format!("  {{ double r = {}({}); sp -= {}; stack[sp++] = r; }}\n", c_ident(name), args.join(", "), argc));
            }
            Instr::Jump(t) => c.push_str(&format!("  goto L{};\n", t)),
            Instr::JumpIfFalse(t) => c.push_str(&format!("  if (stack[--sp] == 0) goto L{};\n", t)),
            Instr::Return => c.push_str("  return stack[--sp];\n"),
            Instr::Pop => c.push_str("  --sp;\n"),
        }
    }
    if current_fn.is_none() { c.push_str("  return 0;\n"); }
    Ok(c)
}

pub fn emit_c(m: &Module) -> Result<String, String> {
    let mut out = String::from("#include <stdio.h>\n#include <math.h>\n\n");
    for f in &m.functions {
        out.push_str(&format!("static double {}(", c_ident(&f.name)));
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("double {}", c_ident(p)));
        }
        out.push_str(");\n");
    }
    let mut globals = std::collections::BTreeSet::new();
    for ins in &m.code {
        if let Instr::Load(n) | Instr::Store(n) = ins { globals.insert(c_ident(n)); }
    }
    for g in globals { out.push_str(&format!("static double {} = 0;\n", g)); }
    for f in &m.functions {
        out.push_str(&format!("\nstatic double {}(", c_ident(&f.name)));
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("double {}", c_ident(p)));
        }
        out.push_str(") {\n");
        out.push_str(&emit_code(&f.code, Some(f))?);
        out.push_str("}\n");
    }
    out.push_str("\nint main(void) {\n");
    out.push_str(&emit_code(&m.code, None)?);
    out.push_str("}\n");
    Ok(out)
}
