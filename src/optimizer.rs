use crate::ir::{BasicBlock, Instr, IrType, Module, SsaFunction};

pub fn optimize(mut module: Module) -> Module {
    for block in &mut module.blocks { block.code = optimize_block(std::mem::take(&mut block.code)); }
    for f in &mut module.functions {
        for block in &mut f.blocks { block.code = optimize_block(std::mem::take(&mut block.code)); }
    }
    module
}

fn optimize_block(code: Vec<Instr>) -> Vec<Instr> {
    let mut rewritten=Vec::with_capacity(code.len());
    let mut old_to_new=vec![0usize;code.len()+1];
    let mut i=0usize;
    let mut terminated=false;

    while i<code.len() {
        old_to_new[i]=rewritten.len();
        if terminated { i+=1; continue; }

        if i+2<code.len() {
            if let (Instr::ConstNumber(a),Instr::ConstNumber(b),Instr::Binary{op,ty:IrType::Number})=(&code[i],&code[i+1],&code[i+2]) {
                if let Some(v)=fold_numeric(op,*a,*b) {
                    rewritten.push(Instr::ConstNumber(v));
                    old_to_new[i+1]=rewritten.len();
                    old_to_new[i+2]=rewritten.len();
                    i+=3;
                    continue;
                }
            }
        }

        if i+1<code.len() {
            if let (Instr::ConstBool(value),Instr::JumpIfFalse(target))=(&code[i],&code[i+1]) {
                if *value {
                    old_to_new[i+1]=rewritten.len();
                    i+=2;
                    continue;
                } else {
                    rewritten.push(Instr::Jump(*target));
                    old_to_new[i+1]=rewritten.len();
                    i+=2;
                    terminated=true;
                    continue;
                }
            }
        }

        match &code[i] {
            Instr::Jump(_) | Instr::Return(_) => terminated=true,
            _=>{}
        }
        rewritten.push(code[i].clone());
        i+=1;
    }
    old_to_new[code.len()]=rewritten.len();

    for ins in &mut rewritten {
        match ins {
            Instr::Jump(t)|Instr::JumpIfFalse(t) => {
                *t=if *t<=code.len(){old_to_new[*t]}else{rewritten.len()};
            }
            _=>{}
        }
    }
    rewritten
}

pub fn validate_ssa(functions: &[SsaFunction]) -> Result<(), String> {
    for function in functions {
        function.verify_operands()?;
    }
    Ok(())
}

fn fold_numeric(op:&str,a:f64,b:f64)->Option<f64>{
    match op {
        "Plus"=>Some(a+b),
        "Minus"=>Some(a-b),
        "Star"=>Some(a*b),
        "Slash" if b!=0.0=>Some(a/b),
        "Percent" if b!=0.0=>Some(a%b),
        _=>None,
    }
}

#[allow(dead_code)]
fn _keep_types(_: &BasicBlock) {}
