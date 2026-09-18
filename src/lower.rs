pub fn lower(program: &[crate::Stmt]) -> crate::ir::Module {
    crate::ir::IrBuilder::new().lower_program(program)
}

pub fn verify(m: &crate::ir::Module) -> Result<(), String> {
    fn check(code: &[crate::ir::Instr], label: &str) -> Result<(), String> {
        for (i, ins) in code.iter().enumerate() {
            match ins {
                crate::ir::Instr::Jump(t) | crate::ir::Instr::JumpIfFalse(t) if *t >= code.len() => {
                    return Err(format!(
                        "IR error in {} at {}: jump target {} is out of bounds",
                        label, i, t
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
    check(&m.code, "module")?;
    for f in &m.functions {
        check(&f.code, &format!("fn {}", f.name))?;
    }
    Ok(())
}
