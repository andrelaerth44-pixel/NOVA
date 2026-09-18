#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type { Any, Number, Bool, String, Array(Box<Type>), Null, Function(Vec<Type>,Box<Type>), Unknown }
impl Type {
    pub fn name(&self)->String{match self{Type::Any=>"any".into(),Type::Number=>"number".into(),Type::Bool=>"bool".into(),Type::String=>"string".into(),Type::Array(t)=>format!("array<{}>",t.name()),Type::Null=>"null".into(),Type::Function(a,r)=>format!("fn({}) -> {}",a.iter().map(|x|x.name()).collect::<Vec<_>>().join(", "),r.name()),Type::Unknown=>"unknown".into()}}
    pub fn compatible(&self,other:&Type)->bool{self==other||matches!(self,Type::Any|Type::Unknown)||matches!(other,Type::Any|Type::Unknown)}
}
