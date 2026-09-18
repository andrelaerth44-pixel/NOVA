use crate::ir::{BasicBlock, Instr, Module};
use std::collections::VecDeque;

pub fn lower(program: &[crate::Stmt]) -> Module {
    crate::ir::IrBuilder::new().lower_program(program)
}

fn successors(blocks:&[BasicBlock],id:usize)->Vec<usize>{
    let code=&blocks[id].code;
    let mut out=Vec::new();
    match code.last(){
        Some(Instr::Jump(t))=>{if *t<code.len(){out.push(id);}},
        Some(Instr::JumpIfFalse(_))=>{if id+1<blocks.len(){out.push(id+1);}},
        Some(Instr::Return(_))=>{},
        _=>if id+1<blocks.len(){out.push(id+1)},
    }
    out
}

fn verify_code(code:&[Instr],label:&str)->Result<(),String>{
    let mut depth=0isize;
    for (i,ins) in code.iter().enumerate(){
        depth+=match ins{
            Instr::ConstNumber(_)|Instr::ConstString(_)|Instr::ConstBool(_)|Instr::ConstNull|Instr::Load(_)=>1,
            Instr::Store(_)|Instr::Pop=>-1,
            Instr::StructInit { fields, .. } => 1 - fields.len() as isize,
            Instr::FieldGet { .. } => 0,
            Instr::Unary{..}=>0,
            Instr::Binary{..}=>-1,
            Instr::Call{name,argc,result}=>{
                let mut d=-(*argc as isize);
                if !matches!(result,crate::ir::IrType::Null){d+=1;}
                if name=="for_each"{d-=1;}
                d
            }
            Instr::Jump(_)=>0,
            Instr::JumpIfFalse(_)=>-1,
            Instr::Return(_)=>-1,
        };
        if depth<0{return Err(format!("IR stack underflow in {} at {}",label,i));}
        if let Instr::Jump(t)|Instr::JumpIfFalse(t)=ins{
            if *t>code.len(){return Err(format!("IR error in {} at {}: jump target {} is out of bounds",label,i,t));}
        }
    }
    Ok(())
}

fn verify_blocks(blocks:&[BasicBlock],label:&str)->Result<(),String>{
    if blocks.is_empty(){return Err(format!("{} has no basic blocks",label));}
    for (i,b) in blocks.iter().enumerate(){
        if b.id!=i{return Err(format!("{} has invalid block id {} (expected {})",label,i,i));}
        verify_code(&b.code,&format!("{} block {}",label,i))?;
    }
    Ok(())
}

pub fn verify(m:&Module)->Result<(),String>{
    verify_blocks(&m.blocks,"module")?;
    for f in &m.functions{verify_blocks(&f.blocks,&format!("fn {}",f.name))?;}
    Ok(())
}

pub fn cfg(m:&Module)->String{
    fn dump(blocks:&[BasicBlock],label:&str,out:&mut String){
        out.push_str(&format!("{}\n",label));
        for b in blocks{
            let mut edges=Vec::new();
            if let Some(last)=b.code.last(){
                match last{
                    Instr::Jump(t)=>edges.push(format!("self@{}",t)),
                    Instr::JumpIfFalse(t)=>{edges.push(format!("self@{}",t));if b.id+1<blocks.len(){edges.push(format!("next@{}",b.id+1));}},
                    Instr::Return(_)=>{},
                    _=>if b.id+1<blocks.len(){edges.push(format!("next@{}",b.id+1));},
                }
            }
            out.push_str(&format!("  block {} -> [{}]\n",b.id,edges.join(", ")));
        }
    }
    let mut out=String::new();
    dump(&m.blocks,"module",&mut out);
    for f in &m.functions{dump(&f.blocks,&format!("fn {}",f.name),&mut out);}
    out
}
