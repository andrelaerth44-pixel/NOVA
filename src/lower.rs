use crate::ir::{Instr, Module};

pub fn lower(program: &[crate::Stmt]) -> Module {
    crate::ir::IrBuilder::new().lower_program(program)
}

pub fn verify(m: &Module) -> Result<(), String> {
    fn check(code: &[Instr], label: &str) -> Result<(), String> {
        let mut depth: isize = 0;
        for (i, ins) in code.iter().enumerate() {
            match ins {
                Instr::ConstNumber(_) | Instr::ConstString(_) | Instr::ConstBool(_) | Instr::ConstNull | Instr::Load(_) => depth += 1,
                Instr::Store(_) | Instr::Pop => depth -= 1,
                Instr::Unary { .. } => {}
                Instr::Binary { .. } => depth -= 1,
                Instr::Call { name, argc, result } => {
                    depth -= *argc as isize;
                    if !matches!(result, crate::ir::IrType::Null) { depth += 1; }
                    if name == "for_each" { depth -= 1; }
                }
                Instr::JumpIfFalse(_) => depth -= 1,
                Instr::Jump(_) => {}
                Instr::Return(_) => depth -= 1,
            }
            if depth < 0 {
                return Err(format!("IR stack underflow in {} at {}", label, i));
            }
            match ins {
                Instr::Jump(t) | Instr::JumpIfFalse(t) if *t >= code.len() => {
                    return Err(format!("IR error in {} at {}: jump target {} is out of bounds", label, i, t));
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(format!("IR stack imbalance in {}: final depth {}", label, depth));
        }
        Ok(())
    }

    fn check_blocks(blocks: &[crate::ir::BasicBlock], label: &str) -> Result<(), String> {
        if blocks.is_empty() {
            return Err(format!("{} has no basic blocks", label));
        }
        for block in blocks {
            check(&block.code, &format!("{} block {}", label, block.id))?;
        }
        Ok(())
    }

    check_blocks(&m.blocks, "module")?;
    for f in &m.functions {
        check_blocks(&f.blocks, &format!("fn {}", f.name))?;
    }
    Ok(())
}
