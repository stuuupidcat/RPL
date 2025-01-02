use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{sym, Span, Symbol};

use rpl_context::{pat, PatCtxt};
use rpl_mir::CheckMirCtxt;

#[instrument(level = "info", skip(tcx, pcx))]
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

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        intravisit::walk_item(self, item);
    }

    fn visit_mod(&mut self, _m: &'tcx hir::Mod<'tcx>, _span: Span, _id: hir::HirId) -> Self::Result {}

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
            #[allow(irrefutable_let_patterns)]
            if let pattern = pattern(self.pcx)
                && let Some(matches) =
                    CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check()
                && let Some(cstring_drop) = matches[pattern.cstring_drop]
                && let drop_span = cstring_drop.span_no_inline(body)
                && let Some(ptr_usage) = matches[pattern.ptr_usage]
                && let use_span = ptr_usage.span_no_inline(body)
            {
                self.tcx.dcx().emit_err(crate::errors::UseAfterDrop {
                    use_span,
                    drop_span,
                    ty: self.tcx.type_of(cstring_did).instantiate_identity(),
                });
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
        fn $ffi_call(i32, *const std::ffi::c_char) -> i32;
        fn $pattern(..) -> _ = mir! {
            type CString = alloc::ffi::c_str::CString;
            type CStr = core::ffi::c_str::CStr;
            type NonNullU8 = core::ptr::non_null::NonNull<[u8]>;

            let cstring: CString = _;
            let cstring_ref: &CString = &cstring;
            let non_null: NonNullU8 = copy ((((*cstring_ref).inner).0).pointer);
            let uslice_ptr: *const [u8] = copy non_null.pointer;
            let cstr_ptr: *const CStr = copy uslice_ptr as *const CStr (PtrToPtr);
            let cstr: &CStr = &(*cstr_ptr);
            let islice: *const [i8] = &raw const ((*cstr).inner);
            let iptr: *const i8 = move islice as *const i8 (PtrToPtr);
            let iptr_arg: *const i8;
            let s: i32;
            #[export(cstring_drop)]
            drop(cstring);

            s = _;
            iptr_arg = copy iptr;
            #[export(ptr_usage)]
            _ = $ffi_call(move s, move iptr_arg);
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
