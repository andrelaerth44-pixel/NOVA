use crate::{lex, Parser, Stmt};
use crate::ir::{Module, SsaFunction};

/// Result of the front-end/compiler pipeline.
///
/// The legacy typed stack IR is retained for the existing C backend while SSA
/// is now generated for the whole top-level program: the synthetic <main>
/// function plus every declared NOVA function.
#[derive(Debug)]
pub struct Compilation {
    pub program: Vec<Stmt>,
    pub ir: Module,
    pub ssa: SsaFunction,
    pub ssa_functions: Vec<SsaFunction>,
}

pub fn parse_source(source: &str) -> Result<Vec<Stmt>, String> {
    let tokens = lex(source)?;
    Parser::new(tokens).program()
}

fn verify_ssa(function: &SsaFunction) -> Result<(), String> {
    function.validate()?;
    function.verify_operands()
}

pub fn compile_program(program: Vec<Stmt>) -> Result<Compilation, String> {
    let mut checker = crate::semantic::Checker::new();
    checker.check(&program).map_err(|errors| errors.join("\n"))?;

    let ir = crate::optimizer::optimize(crate::lower::lower(&program));
    crate::lower::verify(&ir)?;

    let main_ssa = crate::ssa_lower::lower_program(&program);
    verify_ssa(&main_ssa)?;

    let mut ssa_functions = Vec::new();
    ssa_functions.push(main_ssa.clone());

    for stmt in &program {
        if let Stmt::Fn(name, _generics, args, ret, body) = stmt {
            let function = crate::ssa_lower::lower_function(name, args, ret, body);
            verify_ssa(&function)?;
            ssa_functions.push(function);
        }
    }

    Ok(Compilation {
        program,
        ir,
        ssa: main_ssa,
        ssa_functions,
    })
}

pub fn compile_source(source: &str) -> Result<Compilation, String> {
    compile_program(parse_source(source)?)
}

/// Stable textual representation for tools, tests and the future self-hosted
/// compiler bootstrap. It intentionally contains no addresses or hash order.
pub fn format_ssa(function: &SsaFunction) -> String {
    let mut out = String::new();
    let params = function.params
        .iter()
        .map(|(name, ty, _)| format!("{}: {:?}", name, ty))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("fn {}({}) -> {:?}\n", function.name, params, function.return_type));
    for block in &function.blocks {
        out.push_str(&format!("block {}", block.id));
        if !block.params.is_empty() {
            out.push_str(&format!(" (params {:?})", block.params));
        }
        out.push('\n');
        for (value, instruction) in &block.instrs {
            out.push_str(&format!("  %{} = {:?}\n", value, instruction));
        }
        if let Some(term) = &block.terminator {
            out.push_str(&format!("  {:?}\n", term));
        }
    }
    out
}

pub fn format_ssa_module(functions: &[SsaFunction]) -> String {
    let mut out = String::new();
    for (index, function) in functions.iter().enumerate() {
        if index != 0 {
            out.push('\n');
        }
        out.push_str(&format_ssa(function));
    }
    out
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_preserves_function_types_in_ssa() {
        let source = r#"
            fn identity<T>(value: T) -> T {
                return value
            }

            fn sum(a: i64, b: i64) -> i64 {
                return a + b
            }

            print sum(20, 22)
        "#;

        let compilation = compile_source(source).expect("source should compile");
        assert_eq!(compilation.ssa_functions.len(), 3);
        assert!(matches!(
            compilation.ssa_functions[1].params[0].1,
            crate::ir::IrType::TypeParam(ref name) if name == "T"
        ));
        assert!(matches!(
            compilation.ssa_functions[1].return_type,
            crate::ir::IrType::TypeParam(ref name) if name == "T"
        ));
        assert!(matches!(
            compilation.ssa_functions[2].params[0].1,
            crate::ir::IrType::I64
        ));
        assert!(matches!(
            compilation.ssa_functions[2].return_type,
            crate::ir::IrType::I64
        ));
    }
}



    #[test]
    fn lower_option_result_and_try_as_ssa_instructions() {
        let source = r#"
            fn maybe(flag: bool) -> Option<i64> {
                if flag {
                    return Some(42)
                }
                return None
            }

            fn compute(flag: bool) -> Option<i64> {
                value = maybe(flag)?
                return Some(value + 8)
            }

            fn parse(flag: bool) -> Result<string, string> {
                if flag {
                    return Ok("NOVA")
                }
                return Err("failed")
            }

            result = maybe(true)
            match result {
                Some(value) {
                    print value
                },
                None {
                    print 0
                }
            }

            print compute(true)
            print parse(false)?
        "#;

        let compilation = compile_source(source).expect("Option/Result source should compile");
        let all = compilation
            .ssa_functions
            .iter()
            .flat_map(|f| f.blocks.iter())
            .flat_map(|b| b.instrs.iter());

        assert!(all.clone().any(|(_, instr)| matches!(instr, crate::ir::SsaInstr::EnumInit { name, variant, .. } if name == "Option" && variant == "Some")));
        assert!(all.clone().any(|(_, instr)| matches!(instr, crate::ir::SsaInstr::EnumInit { name, variant, .. } if name == "Result" && variant == "Err")));
        assert!(all.clone().any(|(_, instr)| matches!(instr, crate::ir::SsaInstr::EnumTest { variant, .. } if variant == "Some")));
        assert!(all.clone().any(|(_, instr)| matches!(instr, crate::ir::SsaInstr::Try { .. })));
    }


#[cfg(test)]
mod closure_ssa_tests {
    use super::*;

    #[test]
    fn closure_capture_becomes_explicit_ssa_operand() {
        let source = r#"
            fn make_counter() {
                value = 0
                return fn() {
                    value = value + 1
                    return value
                }
            }

            counter = make_counter()
            print counter()
        "#;

        let compilation = compile_source(source).expect("closure source should compile");
        let closure = compilation.ssa_functions[1]
            .blocks
            .iter()
            .flat_map(|block| block.instrs.iter())
            .find_map(|(_, instr)| match instr {
                crate::ir::SsaInstr::Closure { captures, .. } => Some(captures),
                _ => None,
            })
            .expect("closure instruction missing");

        assert_eq!(closure.len(), 1);
        assert_eq!(closure[0].0, "value");
    }
}
