use crate::{lex, Parser, Stmt};
use crate::ir::{Module, SsaFunction};

/// Result of the front-end/compiler pipeline. Keeping these artifacts together
/// makes the compiler usable as a library and gives the future self-hosted
/// compiler a stable boundary between parsing, checking, lowering and codegen.
#[derive(Debug)]
pub struct Compilation {
    pub program: Vec<Stmt>,
    pub ir: Module,
    pub ssa: SsaFunction,
}

pub fn parse_source(source: &str) -> Result<Vec<Stmt>, String> {
    let tokens = lex(source)?;
    Parser::new(tokens).program()
}

pub fn compile_program(program: Vec<Stmt>) -> Result<Compilation, String> {
    let mut checker = crate::semantic::Checker::new();
    checker.check(&program).map_err(|errors| errors.join("\\n"))?;

    let ir = crate::optimizer::optimize(crate::lower::lower(&program));
    crate::lower::verify(&ir)?;

    let ssa = crate::ssa_lower::lower_program(&program);
    ssa.validate()?;
    ssa.verify_operands()?;

    Ok(Compilation { program, ir, ssa })
}

pub fn compile_source(source: &str) -> Result<Compilation, String> {
    compile_program(parse_source(source)?)
}

/// Stable textual representation for tools, tests and the future self-hosted
/// compiler bootstrap. It intentionally contains no addresses or hash order.
pub fn format_ssa(function: &SsaFunction) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn {} -> {:?}\\n", function.name, function.return_type));
    for block in &function.blocks {
        out.push_str(&format!("block {}", block.id));
        if !block.params.is_empty() {
            out.push_str(&format!(" (params {:?})", block.params));
        }
        out.push('\\n');
        for (value, instruction) in &block.instrs {
            out.push_str(&format!("  %{} = {:?}\\n", value, instruction));
        }
        if let Some(term) = &block.terminator {
            out.push_str(&format!("  {:?}\\n", term));
        }
    }
    out
}
