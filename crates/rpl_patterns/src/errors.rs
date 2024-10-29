use rustc_errors::IntoDiagArg;
use rustc_macros::Diagnostic;
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

pub struct Mutability(ty::Mutability);

impl From<ty::Mutability> for Mutability {
    fn from(mutability: ty::Mutability) -> Self {
        Self(mutability)
    }
}

impl IntoDiagArg for Mutability {
    fn into_diag_arg(self) -> rustc_errors::DiagArgValue {
        self.0.prefix_str().into_diag_arg()
    }
}

#[derive(Diagnostic)]
#[diag(rpl_patterns_unsound_slice_cast)]
pub struct UnsoundSliceCast<'tcx> {
    #[note]
    pub cast_from: Span,
    #[primary_span]
    pub cast_to: Span,
    pub ty: Ty<'tcx>,
    pub mutability: Mutability,
}

#[derive(Diagnostic)]
#[diag(rpl_patterns_use_after_drop)]
pub struct UseAfterDrop<'tcx> {
    #[note]
    pub drop_span: Span,
    #[primary_span]
    pub use_span: Span,
    pub ty: Ty<'tcx>,
}

#[derive(Diagnostic)]
#[diag(rpl_patterns_offset_by_one)]
pub struct OffsetByOne {
    #[primary_span]
    #[label(rpl_patterns_read_label)]
    pub read: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
    #[help]
    #[suggestion(code = "({len_local} - 1)")]
    pub len: Span,
    pub len_local: String,
}

// for cve_2018_21000
#[derive(Diagnostic)]
#[diag(rpl_patterns_misordered_parameters)]
pub struct MisorderedParameters {
    #[help]
    #[primary_span]
    pub span: Span,
}
