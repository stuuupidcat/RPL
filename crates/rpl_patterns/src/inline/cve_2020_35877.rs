use crate::lints::UNCHECKED_POINTER_OFFSET;
use rpl_context::PatCtxt;
use rpl_mir::pat::Location;
use rpl_mir::{pat, CheckMirCtxt, Matched};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::mir::Body;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};
use std::collections::BTreeSet;
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
        if kind.header().is_none_or(|header| header.is_unsafe().not()) && self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            // There are two patterns for checked offsets, one for the specific case and one for the general
            // case

            let pattern = pattern_unchecked_ptr_offset(self.pcx);
            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let ptr = matches[pattern.ptr].span_no_inline(body);
                let offset = matches[pattern.offset].span_no_inline(body);
                let reference = matches[pattern.reference].span_no_inline(body);
                debug!(?ptr, ?offset, ?reference);
                self.tcx
                    .dcx()
                    .emit_err(crate::errors::DerefUncheckedPtrOffset { ptr, offset, reference });
            }

            // The pattern means: there exists a pointer `ptr` and an offset `offset` such that `ptr` is
            // offset by `offset`, but no check is performed on the offset.
            //
            // This is a more general pattern than the previous one, as it does not assume the pointer is offset
            // inside a loop.
            //
            // However, it may produce false positives, as the offset and the length may be constrained by a
            // compilation-time constant.
            let pattern_1 = pattern_unchecked_ptr_offset_(self.pcx);
            let pattern_2 = pattern_checked_ptr_offset_lt(self.pcx);
            let matches_2 = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_2.pattern, pattern_2.fn_pat).check();
            let pattern_3 = pattern_checked_ptr_offset_le(self.pcx);
            let matches_3 = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_3.pattern, pattern_3.fn_pat).check();
            let pattern_4 = pattern_checked_ptr_offset_rem(self.pcx);
            let matches_4 = CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_4.pattern, pattern_4.fn_pat).check();

            fn collect_matched(
                matched: &Matched<'_>,
                ptr: Location,
                offset: Location,
                body: &Body<'_>,
            ) -> (Span, Span) {
                let span_ptr = matched[ptr].span_no_inline(body);
                let span_offset = matched[offset].span_no_inline(body);
                trace!(?span_ptr, ?span_offset, "checked offset found");
                (span_ptr, span_offset)
            }
            let locations: BTreeSet<_> = matches_2
                .iter()
                .map(|matches| collect_matched(matches, pattern_2.ptr, pattern_2.offset, body))
                .chain(
                    matches_3
                        .iter()
                        .map(|matches| collect_matched(matches, pattern_3.ptr, pattern_3.offset, body)),
                )
                .chain(
                    matches_4
                        .iter()
                        .map(|matches| collect_matched(matches, pattern_4.ptr, pattern_4.offset, body)),
                )
                .collect();

            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_1.pattern, pattern_1.fn_pat).check() {
                let ptr = matches[pattern_1.ptr].span_no_inline(body);
                let offset = matches[pattern_1.offset].span_no_inline(body);
                if locations.contains(&(ptr, offset)) {
                    // The offset is checked, so don't emit an error
                    continue;
                }
                debug!(?ptr, ?offset);
                self.tcx.emit_node_span_lint(
                    UNCHECKED_POINTER_OFFSET,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    offset,
                    crate::errors::UncheckedPtrOffset { ptr, offset },
                );
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
                        // FIXME: we can't distinguish between the two assignments to `$ptr_1`, so we get two errors
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

struct PatternUncheckedPtrOffsetGeneral<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    ptr: pat::Location,
    offset: pat::Location,
}

#[rpl_macros::pattern_def]
fn pattern_unchecked_ptr_offset_(pcx: PatCtxt<'_>) -> PatternUncheckedPtrOffsetGeneral<'_> {
    let ptr;
    let offset;
    let pattern = rpl! {
        #[meta($T:ty)]
        fn $pattern(..) -> _ = mir! {
            #[export(ptr)]
            let $ptr: *const $T = _;
            let $ptr_1: *const $T = _;
            #[export(offset)]
            $ptr_1 = Offset(copy $ptr, _);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternUncheckedPtrOffsetGeneral {
        pattern,
        fn_pat,
        ptr,
        offset,
    }
}

#[rpl_macros::pattern_def]
fn pattern_checked_ptr_offset_lt(pcx: PatCtxt<'_>) -> PatternUncheckedPtrOffsetGeneral<'_> {
    let ptr;
    let offset;
    let pattern = rpl! {
        #[meta($T:ty, $U:ty)]
        fn $pattern(..) -> _ = mir! {
            let $index: $U = _; // _?0 <-> _2 ?bb0[0] <-> _2
            #[export(ptr)]
            let $ptr: *const $T = _; // _?1 <-> _3 ?bb0[1] <-> bb1[0]
            let $cmp: bool = Lt(copy $index, _); // _?2 <-> _5 ?bb0[2] <-> bb0[3]
            #[export(offset)]
            let $ptr_1: *const $T = Offset(copy $ptr, _); // _?3 <-> _7 ?bb0[3] <-> bb1[1]
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternUncheckedPtrOffsetGeneral {
        pattern,
        fn_pat,
        ptr,
        offset,
    }
}

#[rpl_macros::pattern_def]
fn pattern_checked_ptr_offset_le(pcx: PatCtxt<'_>) -> PatternUncheckedPtrOffsetGeneral<'_> {
    let ptr;
    let offset;
    let pattern = rpl! {
        #[meta($T:ty, $U:ty)]
        fn $pattern(..) -> _ = mir! {
            let $index: $U = _; // _?0 <-> _2 ?bb0[0] <-> _2
            #[export(ptr)]
            let $ptr: *const $T = _; // _?1 <-> _3 ?bb0[1] <-> bb1[0]
            let $cmp: bool = Le(copy $index, _); // _?2 <-> _5 ?bb0[2] <-> bb0[3]
            #[export(offset)]
            let $ptr_1: *const $T = Offset(copy $ptr, _); // _?3 <-> _7 ?bb0[3] <-> bb1[1]
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternUncheckedPtrOffsetGeneral {
        pattern,
        fn_pat,
        ptr,
        offset,
    }
}

#[rpl_macros::pattern_def]
fn pattern_checked_ptr_offset_rem(pcx: PatCtxt<'_>) -> PatternUncheckedPtrOffsetGeneral<'_> {
    let ptr;
    let offset;
    let pattern = rpl! {
        #[meta($T:ty, $U:ty)]
        fn $pattern(..) -> _ = mir! {
            #[export(ptr)]
            let $ptr: *const $T = _;
            let $index: $U = Rem(_, _);
            #[export(offset)]
            let $ptr_1: *const $T = Offset(copy $ptr, copy $index);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    PatternUncheckedPtrOffsetGeneral {
        pattern,
        fn_pat,
        ptr,
        offset,
    }
}
