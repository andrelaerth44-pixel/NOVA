use crate::ir::{BasicBlock, Instr, Module};
use std::collections::VecDeque;

pub fn lower(program: &[crate::Stmt]) -> Module {
    crate::ir::IrBuilder::new().lower_program(program)
}

fn successors(blocks: &[BasicBlock], id: usize) -> Vec<usize> {
    match blocks[id].code.last() {
        Some(Instr::Jump(t)) => vec![*t],
        Some(Instr::JumpIfFalse(t)) => {
            let mut v=vec![*t];
            if id+1<blocks.len(){v.push(id+1);}
            v
        }
        Some(Instr::Return(_)) => vec![],
        _ => if id+1<blocks.len(){vec![id+1]}else{vec![]},
    }
}

fn stack_effect(ins:&Instr)->isize {
    match ins {
        Instr::ConstNumber(_)|Instr::ConstString(_)|Instr::ConstBool(_)|Instr::ConstNull|Instr::Load(_)=>1,
        Instr::Store(_)|Instr::Pop=>-1,
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
    }
}

fn verify_blocks(blocks:&[BasicBlock],label:&str)->Result<(),String>{
    if blocks.is_empty(){return Err(format!("{} has no basic blocks",label));}
    for (i,b) in blocks.iter().enumerate(){
        if b.id!=i{return Err(format!("{} has invalid block id {} (expected {})",label,b.id,i));}
        for ins in &b.code {
            if let Instr::Jump(t)|Instr::JumpIfFalse(t)=ins {
                if *t>=blocks.len(){return Err(format!("{} block {} jumps to invalid block {}",label,i,t));}
            }
        }
    }
    let mut incoming=vec![None;blocks.len()];
    incoming[0]=Some(0isize);
    let mut queue=VecDeque::from([0usize]);
    while let Some(id)=queue.pop_front(){
        let mut depth=incoming[id].unwrap();
        for (pc,ins) in blocks[id].code.iter().enumerate(){
            depth+=stack_effect(ins);
            if depth<0{return Err(format!("IR stack underflow in {} block {} at {}",label,id,pc));}
        }
        for succ in successors(blocks,id){
            match incoming[succ] {
                None=>{incoming[succ]=Some(depth);queue.push_back(succ);}
                Some(old) if old!=depth=>return Err(format!("IR stack depth mismatch at {} block {}: incoming {} and {}",label,succ,old,depth)),
                _=>{}
            }
        }
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
        for b in blocks{out.push_str(&format!("  block {} -> {:?}\n",b.id,successors(blocks,b.id)));}
    }
    let mut out=String::new();
    dump(&m.blocks,"module",&mut out);
    for f in &m.functions{dump(&f.blocks,&format!("fn {}",f.name),&mut out);}
    out
}
