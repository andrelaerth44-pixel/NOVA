#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Any, I32, I64, F32, F64, Number, Bool, String,
    Array(Box<Type>), Struct(String), Enum(String), Generic(String, Vec<Type>),
    TypeParam(String), Null, Void,
    Function(Vec<Type>, Box<Type>), Unknown,
}
impl Type {
    pub fn name(&self)->String{
        match self {
            Type::Any=>"any".into(), Type::I32=>"i32".into(), Type::I64=>"i64".into(),
            Type::F32=>"f32".into(), Type::F64=>"f64".into(), Type::Number=>"number".into(),
            Type::Bool=>"bool".into(), Type::String=>"string".into(),
            Type::Array(t)=>format!("{}[]",t.name()), Type::Struct(n)=>n.clone(), Type::Enum(n)=>n.clone(),
            Type::Generic(n,args)=>format!("{}<{}>",n,args.iter().map(|x|x.name()).collect::<Vec<_>>().join(", ")),
            Type::TypeParam(n)=>n.clone(),
            Type::Null=>"null".into(), Type::Void=>"void".into(),
            Type::Function(a,r)=>format!("fn({}) -> {}",a.iter().map(|x|x.name()).collect::<Vec<_>>().join(", "),r.name()),
            Type::Unknown=>"unknown".into()
        }
    }
    pub fn numeric(&self)->bool { matches!(self,Type::I32|Type::I64|Type::F32|Type::F64|Type::Number) }
    pub fn compatible(&self,other:&Type)->bool {
        self==other
            || matches!(self,Type::Any|Type::Unknown|Type::TypeParam(_))
            || matches!(other,Type::Any|Type::Unknown|Type::TypeParam(_))
            || (self.numeric() && other.numeric())
            || match (self, other) {
                (Type::Array(a), Type::Array(b)) => a.compatible(b),
                (Type::Generic(an, aa), Type::Generic(bn, ba)) => an==bn && aa.len()==ba.len() && aa.iter().zip(ba).all(|(a,b)| a.compatible(b)),
                _ => false
            }
    }
}
