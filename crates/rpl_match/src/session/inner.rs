use rpl_context::pat::{self, PatternItem};
use rustc_hir::def_id::LocalDefId;

use crate::session::collect::MatchCollectCtxt;
use crate::session::config::SessionConfig;
use crate::session::matching::SessionMatching;
use crate::session::slot::{CrateItemIndex, SessionResult, SlotCandidate, collect_slot_descs};

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
        // Item guards need ADT/impl bindings from the item-rooted matcher.
        if !rust_items.legacy_function_matching_enabled() {
            return Vec::new();
        }
        let (fn_slots, adt_slots) = collect_slot_descs(rust_items);

        if fn_slots.is_empty() && adt_slots.is_empty() {
            return Vec::new();
        }

        let mut results = SessionMatching::run(&self.collect, self.config, index, rust_items, &fn_slots, &adt_slots);
        results = Self::deduplicate_fn_slot_permutations(results);
        self.enrich_results(index, &mut results);
        Self::deduplicate_results(rust_items.attr.should_deduplicate(), results)
    }

    fn deduplicate_fn_slot_permutations(results: Vec<SessionResult<'tcx>>) -> Vec<SessionResult<'tcx>> {
        let (single_fn, multi_fn): (Vec<_>, Vec<_>) = results
            .into_iter()
            .partition(|result| Self::fn_assignment_signature(result).len() <= 1);

        let mut kept_multi = Vec::new();
        for result in multi_fn {
            let signature = Self::fn_assignment_signature(&result);
            if kept_multi
                .iter()
                .all(|existing| Self::fn_assignment_signature(existing) != signature)
            {
                kept_multi.push(result);
            }
        }

        let mut kept = single_fn;
        kept.extend(kept_multi);
        kept
    }

    fn fn_assignment_signature(result: &SessionResult<'tcx>) -> Vec<LocalDefId> {
        let mut defs: Vec<_> = result
            .assignments
            .iter()
            .filter_map(|a| match &a.candidate {
                SlotCandidate::Fn(c) => Some(c.def_id),
                SlotCandidate::Adt(_) => None,
            })
            .collect();
        defs.sort_by_key(|def_id| def_id.local_def_index);
        defs
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
                    c.normalized = c.normalized.clone().map(map);
                    c.snapshot = super::bindings::BindingSnapshot::from_normalized(&c.normalized);
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

    pub fn equivalent(&self, other: &Self) -> bool {
        self.bindings.equivalent_to(&other.bindings)
            && self.assignments.len() == other.assignments.len()
            && self.primary_fn_slot() == other.primary_fn_slot()
            && match (self.primary_fn_candidate(), other.primary_fn_candidate()) {
                (Some(a), Some(b)) => a.def_id == b.def_id && a.normalized == b.normalized,
                (None, None) => true,
                _ => false,
            }
    }
}
