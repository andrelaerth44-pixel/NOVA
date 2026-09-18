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
    out.push_str(&format!("fn {} -> {:?}\n", function.name, function.return_type));
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
