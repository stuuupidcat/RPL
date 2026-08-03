//! Graph validation: CFG and DDG constraint checking.
//!
//! After all variables (types, constants, places, locals) and statements have
//! been assigned, the [`GraphValidator`] checks whether the assignment satisfies
//! the structural constraints imposed by the pattern's control-flow and
//! data-dependency graphs.

use rpl_mir_graph::TerminatorEdges;
use rustc_data_structures::stack::ensure_sufficient_stack;
use rustc_middle::mir;

use super::{IntoLocation, Matching, StatementMatch};
use crate::mir::{MatchContext, pat};

/// Validates that a complete variable assignment satisfies
/// the control-flow and data-dependency constraints of the pattern graph.
pub(super) struct GraphValidator<'a, 'pcx, 'tcx> {
    cx: &'a MatchContext<'a, 'pcx, 'tcx>,
    matching: &'a Matching<'tcx>,
}

impl<'a, 'pcx, 'tcx> GraphValidator<'a, 'pcx, 'tcx> {
    pub(super) fn new(cx: &'a MatchContext<'a, 'pcx, 'tcx>, matching: &'a Matching<'tcx>) -> Self {
        Self { cx, matching }
    }

    #[instrument(level = "info", skip(self), ret)]
    pub(super) fn validate(&self) -> bool {
        for block in &self.matching.basic_blocks {
            block.start.take();
            block.end.take();
        }
        self.validate_cfg() && self.validate_ddg()
    }

    #[instrument(level = "info", skip(self), ret)]
    fn validate_cfg(&self) -> bool {
        self.validate_block(pat::BasicBlock::ZERO)
    }

    #[instrument(level = "info", skip(self), ret)]
    fn validate_ddg(&self) -> bool {
        self.matching.loc_pats().all(|loc_pat| {
            let StatementMatch::Location(loc) = self.matching[loc_pat].force_get_matched() else {
                return true;
            };
            let matched = self.validate_stmt_deps(
                self.cx.pat_ddg.deps(loc_pat.block, loc_pat.statement_index),
                |dep_loc, local| {
                    let dep_local =
                        self.cx
                            .mir_ddg
                            .get_dep(loc.block, loc.statement_index, dep_loc.block, dep_loc.statement_index);
                    trace!(?dep_loc, ?local, ?dep_local);
                    dep_local == Some(local)
                },
            );
            debug!(?loc_pat, ?loc, ?matched, "validate_stmt_deps");
            matched
        })
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn validate_block(&self, bb_pat: pat::BasicBlock) -> bool {
        if self.cx.mir_pat[bb_pat].has_pat_end() {
            return true;
        }
        let block = self.matching[bb_pat]
            .terminator()
            .force_get_matched()
            .expect_location()
            .block;
        self.validate_block_end(bb_pat, block)
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn validate_block_start(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        let matching = &self.matching[bb_pat];
        matching.start.get().is_some_and(|block| block == bb)
            || matching.start.get().is_none()
                && self.validate_stmt_deps(
                    self.cx.pat_ddg[bb_pat]
                        .rdep_start()
                        .map(|(stmt_pat, local_pat)| ((bb_pat, stmt_pat).into_location(), local_pat)),
                    |dep_loc, local| {
                        dep_loc.block == bb && self.cx.mir_ddg[bb].is_rdep_start(dep_loc.statement_index, local)
                            || dep_loc.block != bb && self.cx.mir_ddg[bb].is_rdep_start_end(local)
                    },
                )
                && {
                    matching.start.set(Some(bb));
                    // Since start and the end of a block in the pattern graph may match different blocks
                    // in the MIR graph, we don't use `bb` here.
                    ensure_sufficient_stack(|| self.validate_block(bb_pat))
                }
            || {
                matching.start.set(None);
                false
            }
    }

    // FIXME: possibly missing control dependency edges, and low efficiency.
    // For intrablock edges, we can directly test if it is an edge of DDG, but for interblock edges, we
    // need to recursively check if there is a path from the start of the block `bb` to location
    // `rdep_loc`, because we don't store the interblock edges from the start of blocks yet.
    #[instrument(level = "debug", skip(self), ret)]
    fn is_rdep_start(&self, bb: mir::BasicBlock, rdep_loc: mir::Location, local: mir::Local) -> bool {
        rdep_loc.block == bb && self.cx.mir_ddg[bb].is_rdep_start(rdep_loc.statement_index, local)
            || self.cx.mir_ddg[bb].is_rdep_start_end(local)
                && self.cx.mir_cfg[bb]
                    .successors()
                    .any(|bb| ensure_sufficient_stack(|| self.is_rdep_start(bb, rdep_loc, local)))
    }

    // FIXME: in pattern like CVE-2021-29941/2/pattern_uninitialized_slice_mut, when there is a
    // statement in the pattern block matching a terminator, like this
    // ```
    // // pattern
    // ?bb0: {
    //     let len: usize = _;
    //     let vec: Vec<u32> = Vec::with_capacity(len);
    // }
    // ?bb1: {
    //     let vec_ptr = vec.as_mut_ptr();
    // }
    // ?bb2: {
    //     let arr: &mut [u32] = std::slice::from_raw_parts_mut(vec_ptr, len);
    // }
    //
    // // code
    // bb0: {
    //     let vec = Vec::with_capacity(len);
    // }
    // bb1: {
    //     let vec_ptr = vec.as_mut_ptr();
    // }
    // bb2: {
    //     let len = bla.len();
    // }
    // bb3: {
    //     let arr: &mut [u32] = std::slice::from_raw_parts_mut(vec_ptr, len);
    // }
    // ```
    // where `Vec::with_capacity` happens in advance of `bla.len()`, since the current implementation
    // of `validate_block_end` only tries to match `?bb0` with `bb0`, it will fail to match the
    // `bla.len()` statement in `bb3` with `?bb2` due to no data dependency edge can be found from
    // `bla.len()` to the end of `bb0`.
    #[instrument(level = "debug", skip(self), ret)]
    fn validate_block_end(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        // FIXME: handle empty blocks
        if self.cx.mir_pat[bb_pat].statements.is_empty()
            && matches!(self.cx.mir_pat[bb_pat].terminator(), pat::TerminatorKind::Goto(_))
        {
            return true;
        }
        let matching = &self.matching[bb_pat];
        matching.end.get().is_some_and(|block| block == bb)
            || matching.end.get().is_none()
                // FIXME: handle move of return value
                && self.validate_stmt_deps(self.cx.pat_ddg.dep_end(bb_pat), |dep_loc, local| {
                    self.cx.mir_ddg.get_dep_end(bb, dep_loc.block, dep_loc.statement_index)
                        .map(|dep_end| dep_end == local).unwrap_or(true)
                })
                && {
                    matching.end.set(Some(bb));
                    // recursively check all the successor blocks
                    self.validate_block_successors(bb_pat, bb)
                }
            || {
                matching.end.set(None);
                false
            }
    }

    /// Validate DDG edges of a statement, or the start or end of a block.
    ///
    /// We iterate over all data dependencies of a statement (i.e. the iterator
    /// `pat_deps`), and for each dependency `dep_loc_pat` we try to test whether dependency
    /// edge (`local_pat`) of the pattern DDG matches that of the MIR DDG (`local`).
    ///
    /// ```text
    /// 1. dependencies of a statement
    /// dep_loc_pat -----> dep_loc
    ///   ^                   ^
    ///   | local_pat         | local
    ///   |                   |
    /// loc_pat -----------> loc
    ///
    /// 2. dependencies of the end of a block
    /// dep_loc_pat -----> dep_loc
    ///   ^                   ^
    ///   | local_pat         | local
    ///   |                   |
    /// block_end_pat ---> block_end
    ///
    /// 3. reversed dependencies of the start of a block
    /// block_start_pat ---> block_start
    ///   ^                   ^
    ///   | local_pat         | local
    ///   |                   |
    /// rdep_loc_pat -----> rdep_loc
    /// ```
    #[instrument(level = "trace", skip(self, pat_deps, match_dep_local), ret)]
    fn validate_stmt_deps(
        &self,
        mut pat_deps: impl Iterator<Item = (impl IntoLocation<Location = pat::Location>, pat::Local)>,
        mut match_dep_local: impl FnMut(mir::Location, mir::Local) -> bool,
    ) -> bool {
        pat_deps.all(|(dep_loc_pat, local_pat)| {
            let dep_loc_pat = dep_loc_pat.into_location();
            let local = self.matching.locals.force_get(local_pat);
            let dep_stmt = self.matching[dep_loc_pat].force_get_matched();
            let matched = match dep_stmt {
                StatementMatch::Arg(l) => l == local,
                StatementMatch::Location(dep_loc) => {
                    trace!(?dep_loc_pat, ?dep_loc, ?local_pat, ?local, "match_dep_local");
                    match_dep_local(dep_loc, local)
                },
            };
            debug!(
                matched,
                "validate_stmt_deps: {dep_loc_pat:?} <-> {dep_stmt:?}, {local_pat:?} <-> {local:?}",
            );
            matched
        })
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn validate_block_successors(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        use TerminatorEdges::{AssignOnReturn, Double, Single, SwitchInt};
        debug!(term_pat = ?self.cx.pat_cfg[bb_pat], term = ?self.cx.mir_cfg[bb]);
        match (&self.cx.pat_cfg[bb_pat], &self.cx.mir_cfg[bb]) {
            (TerminatorEdges::None, _) => true,
            (&Single(bb_pat), &Single(bb) | &Double(bb, _)) => self.validate_block_start(bb_pat, bb),
            (&Double(bb_pat, unwind_pat), &Double(bb, unwind)) => {
                self.validate_block_start(bb_pat, bb) && self.validate_block_start(unwind_pat, unwind)
            },
            (
                AssignOnReturn {
                    return_: box return_pat,
                    cleanup: cleanup_pat,
                },
                AssignOnReturn { box return_, cleanup },
            ) => {
                return_pat.len() == return_.len()
                    && core::iter::zip(return_pat, return_)
                        .chain(cleanup_pat.as_ref().zip(cleanup.as_ref()))
                        .all(|(&bb_pat, &bb)| self.validate_block_start(bb_pat, bb))
            },
            (SwitchInt(targets_pat), SwitchInt(targets)) => {
                targets_pat.targets.iter().all(|(&value_pat, &bb_pat)| {
                    targets
                        .targets
                        .get(&value_pat)
                        .is_some_and(|&bb| self.validate_block_start(bb_pat, bb))
                }) && match (targets_pat.otherwise, targets.otherwise) {
                    (None, None | Some(_)) => true,
                    (Some(bb_pat), Some(bb)) => self.validate_block_start(bb_pat, bb),
                    (Some(_), None) => false,
                }
            },
            _ => false,
        }
    }
}
