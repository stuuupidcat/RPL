use rustc_errors::IntoDiagArg;
use rustc_macros::{Diagnostic, LintDiagnostic};
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

// for cve_2020_35881
#[derive(Diagnostic)]
#[diag(rpl_patterns_wrong_assumption_of_fat_pointer_layout)]
#[help]
pub struct WrongAssumptionOfFatPointerLayout {
    #[primary_span]
    #[label(rpl_patterns_ptr_transmute_label)]
    pub ptr_transmute: Span,
    #[label(rpl_patterns_get_data_ptr_label)]
    pub data_ptr_get: Span,
}

// for cve_2019_15548
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_rust_str_as_c_str)]
#[help]
pub struct RustStrAsCStr {
    #[label(rpl_patterns_label)]
    pub cast_from: Span,
    #[note]
    pub cast_to: Span,
}

// another pattern for cve_2019_15548
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_lengthless_buffer_passed_to_extern_function)]
pub struct LengthlessBufferPassedToExternFunction {
    #[label(rpl_patterns_label)]
    pub ptr: Span,
}

// for cve_2021_27376
#[derive(Diagnostic)]
#[diag(rpl_patterns_wrong_assumption_of_layout_compatibility)]
#[help]
pub struct WrongAssumptionOfLayoutCompatibility {
    #[primary_span]
    pub cast_to: Span,
    #[note]
    pub cast_from: Span,
    pub type_to: &'static str,
    pub type_from: &'static str,
}

// for cve_2021_27376
#[derive(Diagnostic)]
#[diag(rpl_patterns_trust_exact_size_iterator)]
#[help]
pub struct TrustExactSizeIterator {
    #[primary_span]
    pub set_len: Span,
    #[label(rpl_patterns_len_label)]
    pub len: Span,
}

// for cve_2021_27376
#[derive(Diagnostic)]
#[diag(rpl_patterns_slice_from_raw_parts_uninitialized)]
#[help]
pub struct SliceFromRawPartsUninitialized {
    #[primary_span]
    pub slice: Span,
    #[label(rpl_patterns_len_label)]
    pub len: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
    #[label(rpl_patterns_vec_label)]
    pub vec: Span,
    pub fn_name: &'static str,
}

// for cve_2018_20992
// use `Vec::set_len` to extend the length of a `Vec` without initializing the new elements
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_vec_set_len_to_extend)]
#[note]
pub struct VecSetLenToExtend {
    #[label(rpl_patterns_set_len_label)]
    pub set_len: Span,
    #[label(rpl_patterns_vec_label)]
    pub vec: Span,
}

// for cve_2018_20992
// a workaround for without `#without!`
#[derive(LintDiagnostic)]
// use `Vec::set_len` to truncate the length of a `Vec`
#[diag(rpl_patterns_vec_set_len_to_truncate)]
pub struct VecSetLenToTruncate {
    #[label(rpl_patterns_set_len_label)]
    #[help]
    pub span: Span,
}

// for cve_2019_16138
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_set_len_uninitialized)]
#[help]
pub struct SetLenUninitialized {
    #[label(rpl_patterns_set_len_label)]
    pub set_len: Span,
    #[label(rpl_patterns_vec_label)]
    pub vec: Span,
}

// for cve_2020_35898_9
#[derive(Diagnostic)]
#[diag(rpl_patterns_get_mut_in_rc_unsafecell)]
#[help]
pub struct GetMutInRcUnsafeCell {
    #[primary_span]
    #[note]
    #[help]
    pub get_mut: Span,
}

// for cve_2020_35888
#[derive(Diagnostic)]
#[diag(rpl_patterns_drop_uninit_value)]
pub struct DropUninitValue {
    #[primary_span]
    pub drop: Span,
}

// for cve_2020_35907
#[derive(Diagnostic)]
#[diag(rpl_patterns_thread_local_static_ref)]
#[help(rpl_patterns_sync_help)]
#[help]
pub struct ThreadLocalStaticRef<'tcx> {
    #[primary_span]
    pub span: Span,
    #[label]
    pub thread_local: Span,
    #[label(rpl_patterns_ret_label)]
    pub ret: Span,
    pub ty: Ty<'tcx>,
}

// for cve_2021_25904
// FIXME: add a span for `#[help]` containing the function header
#[derive(Diagnostic)]
#[diag(rpl_patterns_from_raw_parts)]
#[help]
pub struct UnvalidatedSliceFromRawParts {
    #[primary_span]
    pub src: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
    #[label(rpl_patterns_slice_label)]
    pub slice: Span,
}

// for cve_2022_23639
#[derive(Diagnostic)]
#[diag(rpl_patterns_unsound_cast_between_u64_and_atomic_u64)]
#[note]
pub struct UnsoundCastBetweenU64AndAtomicU64 {
    #[primary_span]
    pub transmute: Span,
    #[label(rpl_patterns_src_label)]
    pub src: Span,
}

// for cve_2020_35860
#[derive(Diagnostic)]
#[diag(rpl_patterns_deref_null_pointer)]
#[note]
pub struct DerefNullPointer {
    #[primary_span]
    #[label(rpl_patterns_deref_label)]
    pub deref: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
}

// for cve_2020_35877
#[derive(Diagnostic)]
#[diag(rpl_patterns_deref_unchecked_ptr_offset)]
#[help]
pub struct DerefUncheckedPtrOffset {
    #[primary_span]
    #[label(rpl_patterns_reference_label)]
    pub reference: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
    #[label(rpl_patterns_offset_label)]
    pub offset: Span,
}

// for cve_2020_35901
#[derive(Diagnostic)]
#[diag(rpl_patterns_unsound_pin_project)]
#[note]
pub struct UnsoundPinNewUnchecked<'tcx> {
    #[primary_span]
    pub span: Span,
    #[label]
    pub mut_self: Span,
    pub ty: Ty<'tcx>,
}

// for cve_2020_35877
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_unchecked_ptr_offset)]
#[help]
#[note]
pub struct UncheckedPtrOffset {
    #[label(rpl_patterns_offset_label)]
    pub offset: Span,
    #[label(rpl_patterns_ptr_label)]
    pub ptr: Span,
}

// for cve_2024_27284
#[derive(LintDiagnostic)]
#[diag(rpl_patterns_cassandra_iter_next_ptr_passed_to_cass_iter_get)]
#[help]
pub struct CassandraIterNextPtrPassedToCassIterGet {
    #[label(rpl_patterns_cass_iter_next_label)]
    pub cass_iter_next: Span,
}
