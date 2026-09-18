#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span { pub start: usize, pub end: usize, pub line: usize, pub column: usize }
impl Span { pub fn new(start:usize,end:usize,line:usize,column:usize)->Self{Self{start,end,line,column}} }
#[derive(Clone, Debug)]
pub struct Diagnostic { pub message:String, pub span:Option<Span> }
impl Diagnostic { pub fn new(message:impl Into<String>,span:Option<Span>)->Self{Self{message:message.into(),span}} pub fn render(&self)->String{match self.span{Some(s)=>format!("{}:{}: {}",s.line,s.column,self.message),None=>self.message.clone()}} }
