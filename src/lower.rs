pub fn lower(program: &[crate::Stmt]) -> crate::ir::Module {
    crate::ir::IrBuilder::new().lower_program(program)
}

pub fn verify(m: &crate::ir::Module) -> Result<(), String> {
    fn check(code: &[crate::ir::Instr], label: &str) -> Result<(), String> {
        let mut depth: isize = 0;
        for (i, ins) in code.iter().enumerate() {
            match ins {
                crate::ir::Instr::ConstNumber(_) | crate::ir::Instr::ConstString(_) | crate::ir::Instr::ConstBool(_) | crate::ir::Instr::Load(_) => depth += 1,
                crate::ir::Instr::Store(_) | crate::ir::Instr::Pop | crate::ir::Instr::JumpIfFalse(_) | crate::ir::Instr::Return => depth -= 1,
                crate::ir::Instr::Binary(_) => depth -= 1,
                crate::ir::Instr::Call(name, argc) => {
                    if *argc == 0 { depth += 1; } else if name == "print" || name == "for_each" { depth -= *argc as isize; } else { depth -= *argc as isize - 1; }
                }
                crate::ir::Instr::Jump(_) => {}
                if depth < 0 { return Err(format!("IR stack underflow in {} at {}", label, i)); }

                crate::ir::Instr::Jump(t) | crate::ir::Instr::JumpIfFalse(t) if *t >= code.len() => {
                    return Err(format!(
                        "IR error in {} at {}: jump target {} is out of bounds",
                        label, i, t
                    ));
                }
                _ => {}
            }
        }
        if depth != 0 { return Err(format!("IR stack imbalance in {}: final depth {}", label, depth)); }
        Ok(())
    }
    check(&m.code, "module")?;
    for f in &m.functions {
        check(&f.code, &format!("fn {}", f.name))?;
    }
    Ok(())
}
