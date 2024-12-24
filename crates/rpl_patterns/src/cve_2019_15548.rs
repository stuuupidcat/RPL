use std::ops::Not;

use rpl_context::PatCtxt;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use crate::lints::{LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION, RUST_STRING_POINTER_AS_C_STRING_POINTER};

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

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn(..) => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip_all, fields(?def_id))]
    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        if self.tcx.visibility(def_id).is_public()
            && kind.header().is_none_or(|header| header.is_unsafe().not())
            && self.tcx.is_mir_available(def_id)
        {
            let body = self.tcx.optimized_mir(def_id);
            #[allow(irrefutable_let_patterns)]
            if let pattern_cast = pattern_rust_str_as_c_str(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_cast.fn_pat).check()
                && let Some(cast_from) = matches[pattern_cast.cast_from]
                && let cast_from = cast_from.span_no_inline(body)
                && let Some(cast_to) = matches[pattern_cast.cast_to]
                && let cast_to = cast_to.span_no_inline(body)
            {
                debug!(?cast_from, ?cast_to);
                self.tcx.emit_node_span_lint(
                    RUST_STRING_POINTER_AS_C_STRING_POINTER,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    cast_from,
                    crate::errors::RustStrAsCStr { cast_from, cast_to },
                );
            }
            #[allow(irrefutable_let_patterns)]
            if let pattern_ptr = pattern_pass_a_pointer_to_c(self.pcx)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &pattern_ptr.pattern).check()
                && let Some(ptr) = matches[pattern_ptr.ptr]
                && let ptr = ptr.span_no_inline(body)
            {
                debug!(?ptr);
                self.tcx.emit_node_span_lint(
                    LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    ptr,
                    crate::errors::LengthlessBufferPassedToExternFunction { ptr },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternCast<'pcx> {
    fn_pat: &'pcx pat::Fn<'pcx>,
    cast_from: pat::Location,
    cast_to: pat::Location,
}

// FIXME: this should work for functions other than `crate::ll::instr`.
// FIXME: this should work when `inline-mir` is on.
#[rpl_macros::pattern_def]
fn pattern_rust_str_as_c_str(pcx: PatCtxt<'_>) -> PatternCast<'_> {
    let cast_from;
    let cast_to;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern (..) -> _ = mir! {

            type c_char = libc::c_char;

            #[export(cast_from)]
            let src: &alloc::string::String = _;
            let bytes: &[u8] = alloc::string::String::as_bytes(move src);
            let ptr: *const u8 = slice::as_ptr(copy bytes);
            #[export(cast_to)]
            let dst: *const c_char = copy ptr as *const c_char (Transmute);
            let ret: $T = $crate::ll::instr(move dst);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternCast {
        fn_pat,
        cast_from,
        cast_to,
    }
}

struct PatternPointer<'pcx> {
    pattern: MirPattern<'pcx>,
    ptr: pat::Location,
}

// FIXME: this should work for functions other than `crate::ll::instr`.
#[rpl_macros::pattern_def]
fn pattern_pass_a_pointer_to_c(pcx: PatCtxt<'_>) -> PatternPointer<'_> {
    rpl! {
        fn $pattern (..) -> _ = mir! {
            type c_char = libc::c_char;

            let ptr: *const c_char = _;
            _ = $crate::ll::instr(move ptr);
        }
    }

    PatternPointer {
        pattern,
        ptr: ptr_stmt,
        // ty_var: c_char_ty,
    }
}
