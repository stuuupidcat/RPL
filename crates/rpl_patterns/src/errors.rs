use rustc_errors::IntoDiagArg;
use rustc_macros::Diagnostic;
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

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
