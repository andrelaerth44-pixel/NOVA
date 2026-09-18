use crate::span::{Diagnostic,Span};
pub fn error(message:impl Into<String>,span:Option<Span>)->Diagnostic{Diagnostic::new(message,span)}
