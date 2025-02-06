use rpl_context::PatCtxt;
use rpl_mir::pat::Location;
use rpl_mir::{pat, CheckMirCtxt};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};
use std::ops::Not;

#[instrument(level = "info", skip_all)]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt::new(tcx, pcx);
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'pcx, 'tcx> CheckFnCtxt<'pcx, 'tcx> {
    fn new(tcx: TyCtxt<'tcx>, pcx: PatCtxt<'pcx>) -> Self {
        Self { tcx, pcx }
    }
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn(..) => {},
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

            let pattern = pattern_unchecked_ptr_offset(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let offset = matches[pattern.offset].span_no_inline(body);
                let reference = matches[pattern.reference].span_no_inline(body);
                debug!(?ptr, ?offset, ?reference);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::UncheckedPtrOffset { ptr, offset, reference });
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct PatternUncheckedPtrOffset<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    ptr: pat::Location,
    offset: pat::Location,
    reference: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_unchecked_ptr_offset(pcx: PatCtxt<'_>) -> PatternUncheckedPtrOffset<'_> {
    let ptr;
    let offset;
    let mut reference = Location::uninitialized();
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            #[export(offset)]
            let $offset: usize = _; // _?0 <-> _2 ?bb0[0] <-> _2
            let $offset_1: usize = copy $offset; // _?1 <-> _3 ?bb0[1] <-> bb0[0]
            #[export(ptr)]
            let $ptr_1: *const $T = _; // _?2 <-> _4 ?bb0[2] <-> bb3[0]
            let $offset_2: usize; // _?3 <-> _13
            let $flag: bool; // _?4 <-> _12
            let $ptr_3: *const $T; // _?5 <-> _14
            let $ptr_4: *const $T; // _?6 <-> _15
            let $reference: &$T; // _?7 <-> _0
            loop { // ?bb0[3] <-> bb3[1]
                $offset_2 = copy $offset_1; // ?bb1[0] <-> bb4[0]
                $flag = Gt(move $offset_2, const 0usize); // ?bb1[1] <-> bb4[0]
                switchInt(move $flag) { // ?bb1[2]
                    0usize => {
                        #[export(reference)]
                        $reference = &(*$ptr_1); // ?bb4[0]
                        break; // ?bb4[1]
                    }
                    _ => {
                        $offset_1 = Sub(copy $offset_1, const 1usize); // ?bb5[0] <-> bb5[0]
                        $ptr_4 = copy $ptr_1; // ?bb5[1] <-> bb5[1]
                        $ptr_3 = Offset(copy $ptr_4, _); // ?bb5[2] <-> bb5[3]
                        $ptr_1 = move $ptr_3; // ?bb5[3] <-> bb5[4]
                        // FIXME: without this, a basic block, where there is only one goto statement, is generated
                        continue; // ?bb5[4] <-> bb5[5]
                    }
                }
            }
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternUncheckedPtrOffset {
        pattern,
        fn_pat,
        ptr,
        offset,
        reference,
    }
}
