use crate::ir::{BasicBlock, Instr, IrType, Module};

pub fn optimize(mut module: Module) -> Module {
    for block in &mut module.blocks {
        block.code = fold(std::mem::take(&mut block.code));
    }
    for f in &mut module.functions {
        for block in &mut f.blocks {
            block.code = fold(std::mem::take(&mut block.code));
        }
    }
    module
}

fn fold(code: Vec<Instr>) -> Vec<Instr> {
    let mut out = Vec::with_capacity(code.len());
    let mut i = 0;
    while i < code.len() {
        if i + 2 < code.len() {
            if let (Instr::ConstNumber(a), Instr::ConstNumber(b), Instr::Binary { op, ty: IrType::Number }) =
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

#[allow(dead_code)]
fn _keep_types(_: &BasicBlock) {}
