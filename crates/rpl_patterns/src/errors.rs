use rustc_macros::{Diagnostic, Subdiagnostic};
use rustc_span::{Span, Symbol};

#[derive(Diagnostic)]
#[diag(rpl_patterns_unsound_as_bytes_trait)]
pub struct UnsoundAsBytesTrait {
    #[primary_span]
    pub span: Span,
    #[subdiagnostic]
    pub as_bytes: Vec<UnsoundAsBytesMethod>,
    #[suggestion(code = " unsafe")]
    pub unsafe_sugg: Span,
}

#[derive(Subdiagnostic)]
#[label(rpl_patterns_unsound_as_bytes_method)]
pub struct UnsoundAsBytesMethod {
    #[primary_span]
    pub span: Span,
    pub name: Symbol,
    pub ref_mutbly: &'static str,
}
