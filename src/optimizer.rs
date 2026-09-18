use crate::ir::{Instr, Module};

pub fn optimize(mut module: Module) -> Module {
    module.code = fold(module.code);
    for f in &mut module.functions {
        f.code = fold(std::mem::take(&mut f.code));
    }
    module
}

fn fold(code: Vec<Instr>) -> Vec<Instr> {
    let mut out = Vec::with_capacity(code.len());
    let mut i = 0;
    while i < code.len() {
        if i + 2 < code.len() {
            if let (Instr::ConstNumber(a), Instr::ConstNumber(b), Instr::Binary(op)) =
                (&code[i], &code[i + 1], &code[i + 2])
            {
                let value = match op.as_str() {
                    "Plus" => Some(a + b),
                    "Minus" => Some(a - b),
                    "Star" => Some(a * b),
                    "Slash" if *b != 0.0 => Some(a / b),
                    "Percent" if *b != 0.0 => Some(a % b),
                    _ => None,
                };
                if let Some(v) = value {
                    out.push(Instr::ConstNumber(v));
                    i += 3;
                    continue;
                }
            }
        }
        out.push(code[i].clone());
        i += 1;
    }
    out
}
