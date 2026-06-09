use rustc_errors::{DiagArgValue, IntoDiagArg, MultiSpan};
use rustc_macros::{Diagnostic, Subdiagnostic};
use rustc_span::Span;

use crate::utils::DumpOrPrintDiagKind;

/*
#[derive(Diagnostic)]
#[diag(rpl_utils_abort_due_to_debugging)]
#[note]
#[note(rpl_utils_remove_note)]
pub(crate) struct AbortDueToDebugging {
    #[primary_span]
    pub span: MultiSpan,
    #[subdiagnostic]
    // A `Vec` of `Subdiagnostic` now means a set of suggestions
    pub suggs: Vec<AbortDueToDebuggingSugg>,
}

#[derive(Subdiagnostic)]
#[multipart_suggestion(rpl_utils_abort_due_to_debugging_sugg, applicability = "machine-applicable")]
pub(crate) struct AbortDueToDebuggingSugg {
    #[suggestion_part(code = "")]
    pub span: Span,
}
*/

#[derive(Diagnostic)]
#[diag("abort due to debugging")]
#[note("`#[rpl::dump_hir]`, `#[rpl::print_hir]` and `#[rpl::dump_mir]` are only used for debugging")]
#[note("this error is to remind you removing these attributes")]
pub(crate) struct ErrorDueToDebugging {
    #[primary_span]
    #[suggestion("remove this attribute", code = "", applicability = "machine-applicable")]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("{$message}")]
pub(crate) struct DumpOrPrintDiag {
    #[primary_span]
    pub span: Span,
    #[label(
        "{$kind ->
        [dump_hir] HIR dumped
        [print_hir] HIR printed
        *[other] {\"\"}
    } because of this attribute"
    )]
    pub attr_span: Span,
    pub message: String,
    pub kind: DumpOrPrintDiagKind,
}

#[derive(Diagnostic)]
#[diag("MIR of `{$def_id}`")]
pub(crate) struct DumpMir {
    #[primary_span]
    pub span: Span,
    #[label("MIR dumped because of this attribute")]
    pub attr_span: Span,
    pub def_id: DefId,
    #[subdiagnostic]
    pub files: Vec<DumpMirFile>,
    #[subdiagnostic]
    pub locals_and_source_scopes: DumpMirLocalsAndSourceScopes,
    #[subdiagnostic]
    pub blocks: Vec<DumpMirBlock>,
}

#[derive(Subdiagnostic)]
#[note("see `{$file}` for dumped {$content}")]
pub(crate) struct DumpMirFile {
    pub file: String,
    pub content: &'static str,
}

#[derive(Subdiagnostic)]
#[note("locals and scopes in this MIR")]
pub(crate) struct DumpMirLocalsAndSourceScopes {
    #[primary_span]
    pub multi_span: MultiSpan,
}

#[derive(Subdiagnostic)]
#[note("{$block}")]
pub(crate) struct DumpMirBlock {
    pub block: String,
    #[primary_span]
    pub multi_span: MultiSpan,
}

#[derive(Diagnostic)]
#[diag("MIR of `{$instance}` is not available")]
pub(crate) struct DumpMirNotAvailable<'tcx> {
    pub instance: Instance<'tcx>,
    #[primary_span]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("expect a function path")]
pub(crate) struct DumpMirNotFnPath(#[primary_span] pub Span);

#[derive(Diagnostic)]
#[diag("`#[rpl::dump_mir]` cannot be used here")]
pub(crate) struct DumpMirInvalid(#[primary_span] pub Span);

#[derive(Diagnostic)]
#[diag("expect an initialization")]
pub(crate) struct DumpMirExpectInit {
    #[primary_span]
    pub span: Span,
    #[suggestion(
        "try add an initialization",
        code = "= /* expr */",
        applicability = "has-placeholders"
    )]
    pub missing: Span,
}

pub(crate) struct DefId(pub(crate) rustc_span::def_id::DefId);

impl IntoDiagArg for DefId {
    fn into_diag_arg(self, path: &mut Option<std::path::PathBuf>) -> DiagArgValue {
        rustc_middle::ty::tls::with_context(|icx| icx.tcx.def_path_str(self.0)).into_diag_arg(path)
    }
}

impl From<rustc_span::def_id::DefId> for DefId {
    fn from(def_id: rustc_span::def_id::DefId) -> Self {
        DefId(def_id)
    }
}

pub(crate) struct Instance<'tcx>(pub(crate) rustc_middle::ty::Instance<'tcx>);

impl IntoDiagArg for Instance<'_> {
    fn into_diag_arg(self, path: &mut Option<std::path::PathBuf>) -> DiagArgValue {
        self.0.to_string().into_diag_arg(path)
    }
}

impl<'tcx> From<rustc_middle::ty::Instance<'tcx>> for Instance<'tcx> {
    fn from(instance: rustc_middle::ty::Instance<'tcx>) -> Self {
        Instance(instance)
    }
}
