use rpl_context::pat::{self, PatternItem};
use rustc_hir::def_id::LocalDefId;

use crate::session::collect::MatchCollectCtxt;
use crate::session::config::SessionConfig;
use crate::session::matching::SessionMatching;
use crate::session::slot::{CrateItemIndex, MatchSlot, SessionResult, SlotCandidate, collect_slot_descs};

/// Orchestrates candidate collection and multi-slot matching for one pattern item.
pub struct MatchSession<'a, 'pcx, 'tcx> {
    collect: MatchCollectCtxt<'a, 'pcx, 'tcx>,
    config: SessionConfig,
}

impl<'a, 'pcx, 'tcx> MatchSession<'a, 'pcx, 'tcx> {
    pub fn new(collect: MatchCollectCtxt<'a, 'pcx, 'tcx>, config: SessionConfig) -> Self {
        Self { collect, config }
    }

    pub fn with_defaults(collect: MatchCollectCtxt<'a, 'pcx, 'tcx>) -> Self {
        Self::new(collect, SessionConfig::default())
    }

    pub fn match_rust_items(
        &self,
        index: &CrateItemIndex,
        rust_items: &'pcx pat::RustItems<'pcx>,
    ) -> Vec<SessionResult<'tcx>> {
        let (fn_slots, adt_slots) = collect_slot_descs(rust_items);

        if fn_slots.is_empty() && adt_slots.is_empty() {
            return Vec::new();
        }

        let mut results = SessionMatching::run(&self.collect, self.config, index, rust_items, &fn_slots, &adt_slots);
        self.enrich_results(index, &mut results);
        Self::deduplicate_results(rust_items.attr.should_deduplicate(), results)
    }

    fn deduplicate_results(deduplicate: bool, results: Vec<SessionResult<'tcx>>) -> Vec<SessionResult<'tcx>> {
        if !deduplicate {
            return results;
        }
        let mut kept = Vec::new();
        for result in results {
            if kept
                .iter()
                .all(|existing: &SessionResult<'tcx>| !existing.equivalent(&result))
            {
                kept.push(result);
            }
        }
        kept
    }

    fn enrich_results(&self, index: &CrateItemIndex, results: &mut [SessionResult<'tcx>]) {
        for result in results.iter_mut() {
            if let Some(ctx) = &mut result.primary_fn {
                if let Some(item) = index.fns.iter().find(|i| i.def_id == ctx.def_id) {
                    ctx.fn_name = item.fn_name;
                    ctx.header = item.header;
                    ctx.has_self = item.has_self;
                }
                ctx.self_ty = index.self_ty(self.collect.tcx, ctx.def_id);
            }
        }
    }

    pub fn match_pattern_item(
        &self,
        index: &CrateItemIndex,
        pat_item: &'pcx PatternItem<'pcx>,
    ) -> Vec<SessionResult<'tcx>> {
        match pat_item {
            PatternItem::RustItems(items) => self.match_rust_items(index, items),
            PatternItem::RPLPatternOperation(op) => {
                let results = self.match_pattern_operation(index, op);
                Self::deduplicate_results(op.attr.should_deduplicate(), results)
            },
        }
    }

    fn match_pattern_operation(
        &self,
        index: &CrateItemIndex,
        op: &pat::PatternOperation<'pcx>,
    ) -> Vec<SessionResult<'tcx>> {
        let positive: Vec<_> = op
            .positive
            .iter()
            .flat_map(|(_, item, map)| {
                self.match_pattern_item(index, item)
                    .into_iter()
                    .map(|result| result.map_bindings(map))
            })
            .collect();

        let negative: Vec<_> = op
            .negative
            .iter()
            .flat_map(|(_, item, map)| {
                self.match_pattern_item(index, item)
                    .into_iter()
                    .map(|result| result.map_bindings(map))
            })
            .collect();

        positive
            .into_iter()
            .filter(|pos| {
                let Some((pos_def, pos_norm)) = pos.operation_match_key() else {
                    return true;
                };
                !negative.iter().any(|neg| {
                    neg.operation_match_key()
                        .is_some_and(|(neg_def, neg_norm)| pos_def == neg_def && pos_norm == neg_norm)
                })
            })
            .collect()
    }
}

impl SessionResult<'_> {
    fn map_bindings(self, map: &pat::MatchedMap) -> Self {
        let assignments = self
            .assignments
            .into_iter()
            .map(|mut a| {
                if let SlotCandidate::Fn(ref mut c) = a.candidate {
                    let adt_defs = c.snapshot.adt_defs.clone();
                    c.normalized = c.normalized.clone().map(map);
                    c.snapshot =
                        super::bindings::BindingSnapshot::from_normalized_with_adt_defs(&c.normalized, adt_defs);
                }
                a
            })
            .collect();
        Self {
            assignments,
            bindings: self.bindings,
            primary_fn: self.primary_fn,
        }
    }

    /// Slot → DefId assignment key (order-sensitive by slot identity, not DefId set).
    fn slot_def_signature(&self) -> Vec<(MatchSlot, LocalDefId)> {
        let mut sig: Vec<_> = self
            .assignments
            .iter()
            .filter_map(|a| match &a.candidate {
                SlotCandidate::Fn(c) => Some((a.slot, c.def_id)),
                SlotCandidate::Adt(c) => Some((a.slot, c.def_id)),
            })
            .collect();
        sig.sort_by_key(|(slot, def_id)| (*slot, def_id.local_def_index));
        sig
    }

    pub fn equivalent(&self, other: &Self) -> bool {
        self.slot_def_signature() == other.slot_def_signature() && self.bindings.equivalent_to(&other.bindings)
    }
}
