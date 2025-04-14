use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol, sym};

use rpl_context::{PatCtxt, pat};
use rpl_mir::CheckMirCtxt;

use crate::lints::USE_AFTER_DROP;

#[instrument(level = "info", skip_all)]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt { tcx, pcx };
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.is_mir_available(def_id)
            && let Some(cstring_did) = self.tcx.get_diagnostic_item(sym::cstring_type)
        {
            let body = self.tcx.optimized_mir(def_id);
            let pattern = pattern(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let use_span = matches[pattern.ptr_usage].span_no_inline(body);
                let drop_span = matches[pattern.cstring_drop].span_no_inline(body);
                self.tcx.emit_node_span_lint(
                    USE_AFTER_DROP,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    use_span,
                    crate::errors::UseAfterDrop {
                        use_span,
                        drop_span,
                        ty: self.tcx.type_of(cstring_did).instantiate_identity(),
                    },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    cstring_drop: pat::Location,
    ptr_usage: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern(pcx: PatCtxt<'_>) -> Pattern<'_> {
    let cstring_drop;
    let ptr_usage;
    let pattern = rpl! {
        // TODO: match the ABI of the function

        #[meta($SessT:ty)]
        fn $ffi_call(*mut $SessT, *const std::ffi::c_char) -> i32;

        #[meta($SessT:ty)]
        fn $pattern(..) -> _ = mir! {
            type CString = alloc::ffi::c_str::CString;
            type CStr = core::ffi::c_str::CStr;
            type NonNullU8 = core::ptr::non_null::NonNull<[u8]>;

            let $cstring: CString = _;
            let $cstring_ref: &CString = &$cstring;
            let $non_null: NonNullU8 = copy ((((*$cstring_ref).inner).0).pointer);
            let $cstr_ptr: *const CStr = copy $non_null as *const CStr (Transmute);
            let $cstr: &CStr = &(*$cstr_ptr);
            let $islice: *const [i8] = &raw const ((*$cstr).inner);
            let $iptr: *const i8 = move $islice as *const i8 (PtrToPtr);
            let $iptr_arg: *const i8;
            let $s: *mut $SessT;
            #[export(cstring_drop)]
            drop($cstring);

            $s = _;
            $iptr_arg = copy $iptr;
            #[export(ptr_usage)]
            _ = $ffi_call(move $s, move $iptr_arg);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    Pattern {
        pattern,
        fn_pat,
        cstring_drop,
        ptr_usage,
    }
}
