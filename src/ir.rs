#[derive(Clone, Debug)]
pub enum Instr { ConstNumber(f64), ConstString(String), ConstBool(bool), Load(String), Store(String), Binary(String), Call(String,usize), Return, Pop }
#[derive(Clone, Debug, Default)]
pub struct Module { pub code:Vec<Instr> }
impl Module { pub fn push(&mut self,i:Instr){self.code.push(i)} }
