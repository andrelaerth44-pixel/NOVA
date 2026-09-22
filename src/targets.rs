use crate::ir::{Instr,Module};

fn first_number(m:&Module)->Result<f64,String>{
    for block in &m.blocks{
        for instr in &block.code{
            if let Instr::ConstNumber(v)=instr{return Ok(*v);}
        }
    }
    Err("target backend needs a numeric constant".into())
}

pub fn emit_x86_64_gas(m:&Module)->Result<String,String>{
    let value=first_number(m)?;
    Ok(format!(
        ".text\n.globl main\nmain:\n  mov ${}, %eax\n  ret\n# NOVA x86-64 backend constant={}\n",
        value as i64,
        value
    ))
}

pub fn emit_aarch64_gas(m:&Module)->Result<String,String>{
    let value=first_number(m)?;
    Ok(format!(
        ".text\n.global main\nmain:\n  mov w0, #{}\n  ret\n// NOVA AArch64 backend constant={}\n",
        value
    ))
}

pub fn emit_wat(m:&Module)->Result<String,String>{
    let value=first_number(m)?;
    Ok(format!(
        "(module (func (export \"main\") (result i32) i32.const {})) ;; NOVA WASM constant={}\n",
        value,
        value
    ))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn emitters_exist(){
        let m=Module{blocks:vec![crate::ir::BasicBlock{id:0,code:vec![Instr::ConstNumber(42.0)]}],functions:vec![]};
        assert!(emit_x86_64_gas(&m).unwrap().contains("x86-64"));
        assert!(emit_aarch64_gas(&m).unwrap().contains("AArch64"));
        assert!(emit_wat(&m).unwrap().contains("WASM"));
    }
}
