use std::ops::Not;

use rpl_context::PatCtxt;
use rpl_mir::pat::MirPatternBuilder;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

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
            if let mut patterns_cast = MirPatternBuilder::new(self.pcx)
                && let pattern_cast = pattern_rust_str_as_c_str(&mut patterns_cast)
                && let Some(matches) = CheckMirCtxt::new(self.tcx, body, &patterns_cast.build()).check()
                && let Some(cast_from) = matches[pattern_cast.cast_from]
                && let cast_from = cast_from.span_no_inline(body)
                && let Some(cast_to) = matches[pattern_cast.cast_to]
                && let cast_to = cast_to.span_no_inline(body)
            {
                debug!(?cast_from, ?cast_to);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::RustStrAsCStr { cast_from, cast_to });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternCast {
    cast_from: pat::Location,
    cast_to: pat::Location,
}

// FIXME: this does not work due to the lack of name resolution.
// FIXME: this should work for `libc::char` but not hard encoded `i8`.
// FIXME: this should work for functions other than `crate::ll::instr`.
#[rpl_macros::mir_pattern]
fn pattern_rust_str_as_c_str(patterns: &mut pat::MirPatternBuilder<'_>) -> PatternCast {
    mir! {
        meta!($T:ty);

        // type c_char = libc::c_char;
        type c_char = i8;

        let src: &alloc::string::String = _;
        let bytes: &[u8] = alloc::string::String::as_bytes(move src);
        let ptr: *const u8 = core::slice::as_ptr(copy bytes);
        let dst: *const c_char = copy ptr as *const c_char (Transmute);
        let ret: $T = $crate::ll::instr(move dst);
    }

    PatternCast {
        cast_from: src_stmt,
        cast_to: dst_stmt,
    }
}
