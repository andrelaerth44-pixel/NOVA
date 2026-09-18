use crate::ir::{IrType, SsaFunction, SsaInstr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiType {
    Void,
    I32,
    I64,
    F32,
    F64,
    Bool,
    ValuePtr,
    ClosurePtr,
}

impl AbiType {
    pub fn c_name(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I32 => "int32_t",
            Self::I64 => "int64_t",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Bool => "uint8_t",
            Self::ValuePtr => "NovaValue*",
            Self::ClosurePtr => "NovaClosure*",
        }
    }
}

pub fn type_to_abi(ty: &IrType) -> AbiType {
    match ty {
        IrType::I32 => AbiType::I32,
        IrType::I64 => AbiType::I64,
        IrType::F32 => AbiType::F32,
        IrType::F64 | IrType::Number => AbiType::F64,
        IrType::Bool => AbiType::Bool,
        IrType::Null => AbiType::Void,
        IrType::Function(_, _) => AbiType::ClosurePtr,
        IrType::String
        | IrType::Any
        | IrType::Array(_)
        | IrType::Struct(_)
        | IrType::Enum(_)
        | IrType::Generic(_, _)
        | IrType::TypeParam(_) => AbiType::ValuePtr,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionAbi {
    pub name: String,
    pub params: Vec<AbiType>,
    pub return_type: AbiType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureAbi {
    pub function: String,
    pub captures: Vec<(String, AbiType)>,
    pub params: Vec<AbiType>,
    pub return_type: AbiType,
}

pub fn function_abi(function: &SsaFunction) -> FunctionAbi {
    FunctionAbi {
        name: function.name.clone(),
        params: function.params.iter().map(|(_, ty, _)| type_to_abi(ty)).collect(),
        return_type: type_to_abi(&function.return_type),
    }
}

pub fn closures(function: &SsaFunction) -> Vec<ClosureAbi> {
    let mut result = Vec::new();
    for block in &function.blocks {
        for (_, instr) in &block.instrs {
            if let SsaInstr::Closure {
                function,
                params,
                captures,
                ty,
            } = instr
            {
                let (call_params, return_type) = match ty {
                    IrType::Function(args, ret) => (
                        args.iter().map(type_to_abi).collect(),
                        type_to_abi(ret),
                    ),
                    _ => (
                        params.iter().map(|_| AbiType::ValuePtr).collect(),
                        AbiType::ValuePtr,
                    ),
                };

                result.push(ClosureAbi {
                    function: function.clone(),
                    captures: captures
                        .iter()
                        .map(|(name, _)| (name.clone(), AbiType::ValuePtr))
                        .collect(),
                    params: call_params,
                    return_type,
                });
            }
        }
    }
    result
}

pub fn format_function(function: &FunctionAbi) -> String {
    let params = function.params.iter().map(AbiType::c_name).collect::<Vec<_>>().join(", ");
    format!(
        "fn {}({}) -> {}",
        function.name,
        params,
        function.return_type.c_name()
    )
}

pub fn format_closure(closure: &ClosureAbi) -> String {
    let params = closure.params.iter().map(AbiType::c_name).collect::<Vec<_>>().join(", ");
    let captures = closure.captures
        .iter()
        .map(|(name, ty)| format!("{}: {}", name, ty.c_name()))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "closure {}: env(NovaEnv*) captures [{}] invoke(NovaEnv*, {}) -> {}",
        closure.function,
        captures,
        params,
        closure.return_type.c_name()
    )
}

pub fn format_module(functions: &[SsaFunction]) -> String {
    let mut out = String::from(
        "NOVA ABI v1\n         runtime types: NovaValue*, NovaClosure*, NovaEnv*\n",
    );

    for function in functions {
        out.push_str(&format!("{}\n", format_function(&function_abi(function))));
    }

    for function in functions {
        for closure in closures(function) {
            out.push_str(&format!("{}\n", format_closure(&closure)));
        }
    }

    out
}
