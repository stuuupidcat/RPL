use std::cell::Cell;
use std::fmt;
use std::num::NonZero;
use std::ops::Index;

use itertools::Itertools;
use rpl_mir_graph::TerminatorEdges;
use rustc_index::bit_set::HybridBitSet;
use rustc_index::IndexVec;
use rustc_middle::mir::visit::{MutatingUseContext, PlaceContext};
use rustc_middle::mir::{self};
use rustc_middle::ty::Ty;
use rustc_span::Span;

use crate::{pat, CheckMirCtxt};

pub struct Matches<'tcx> {
    pub basic_blocks: IndexVec<pat::BasicBlock, BlockMatches>,
    pub locals: IndexVec<pat::Local, mir::Local>,
    pub ty_vars: IndexVec<pat::TyVarIdx, Ty<'tcx>>,
}

pub struct BlockMatches {
    pub statements: Vec<Option<StatementMatch>>,
    pub start: Option<mir::BasicBlock>,
    pub end: Option<mir::BasicBlock>,
}

impl Index<pat::BasicBlock> for Matches<'_> {
    type Output = BlockMatches;

    fn index(&self, bb: pat::BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

impl Index<pat::Location> for Matches<'_> {
    type Output = Option<StatementMatch>;

    fn index(&self, stmt: pat::Location) -> &Self::Output {
        &self.basic_blocks[stmt.block].statements[stmt.statement_index]
    }
}

impl Index<pat::Local> for Matches<'_> {
    type Output = mir::Local;

    fn index(&self, local: pat::Local) -> &Self::Output {
        &self.locals[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for Matches<'tcx> {
    type Output = Ty<'tcx>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars[ty_var]
    }
}

pub fn matches<'tcx>(cx: &CheckMirCtxt<'_, '_, 'tcx>) -> Option<Matches<'tcx>> {
    let mut matching = MatchCtxt::new(cx);
    if matching.do_match() {
        matching.matches.log_matches(cx.body);
        return matching.matches.try_into_matches().ok();
    }
    None
}

struct MatchFailed;

struct CheckingMatches<'tcx> {
    basic_blocks: IndexVec<pat::BasicBlock, CheckBlockMatches>,
    locals: IndexVec<pat::Local, LocalMatches>,
    ty_vars: IndexVec<pat::TyVarIdx, TyVarMatches<'tcx>>,
}

impl Index<pat::BasicBlock> for CheckingMatches<'_> {
    type Output = CheckBlockMatches;

    fn index(&self, bb: pat::BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

impl Index<pat::Location> for CheckingMatches<'_> {
    type Output = StatementMatches;

    fn index(&self, stmt: pat::Location) -> &Self::Output {
        &self.basic_blocks[stmt.block].statements[stmt.statement_index]
    }
}

impl Index<pat::Local> for CheckingMatches<'_> {
    type Output = LocalMatches;

    fn index(&self, local: pat::Local) -> &Self::Output {
        &self.locals[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for CheckingMatches<'tcx> {
    type Output = TyVarMatches<'tcx>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars[ty_var]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StatementMatch {
    Arg(mir::Local),
    Location(mir::Location),
}

impl From<mir::Local> for StatementMatch {
    fn from(local: mir::Local) -> Self {
        StatementMatch::Arg(local)
    }
}

impl From<mir::Location> for StatementMatch {
    fn from(loc: mir::Location) -> Self {
        StatementMatch::Location(loc)
    }
}

impl fmt::Debug for StatementMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatementMatch::Arg(local) => local.fmt(f),
            StatementMatch::Location(loc) => loc.fmt(f),
        }
    }
}

impl StatementMatch {
    // fn is_in_block(self, block: mir::BasicBlock) -> bool {
    //     match self {
    //         StatementMatch::Arg(_) => true,
    //         StatementMatch::Location(loc) => loc.block == block,
    //     }
    // }

    pub fn debug_with<'a, 'tcx>(self, body: &'a mir::Body<'tcx>) -> impl core::fmt::Debug + use<'a, 'tcx> {
        struct DebugStatementMatch<'a, 'tcx> {
            stmt_match: StatementMatch,
            body: &'a mir::Body<'tcx>,
        }
        impl core::fmt::Debug for DebugStatementMatch<'_, '_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.stmt_match {
                    StatementMatch::Arg(local) => write!(f, "let {local:?}: {:?}", self.body.local_decls[local].ty),
                    StatementMatch::Location(location) => self.body.stmt_at(location).either_with(
                        f,
                        |f, stmt| stmt.fmt(f),
                        |f, terminator| terminator.kind.fmt(f),
                    ),
                }
            }
        }
        DebugStatementMatch { stmt_match: self, body }
    }

    pub fn source_info<'a>(self, body: &'a mir::Body<'_>) -> &'a mir::SourceInfo {
        match self {
            StatementMatch::Arg(arg) => &body.local_decls[arg].source_info,
            StatementMatch::Location(loc) => body.source_info(loc),
        }
    }

    pub fn span_no_inline(self, body: &mir::Body<'_>) -> Span {
        let source_info = self.source_info(body);
        let mut scope = source_info.scope;
        while let Some(parent_scope) = body.source_scopes[scope].inlined_parent_scope {
            scope = parent_scope;
        }
        if let Some((_instance, span)) = body.source_scopes[scope].inlined {
            return span;
        }
        source_info.span
    }
}

struct MatchCtxt<'a, 'pcx, 'tcx> {
    cx: &'a CheckMirCtxt<'a, 'pcx, 'tcx>,
    matches: CheckingMatches<'tcx>,
    // succeeded: Cell<bool>,
}

impl<'a, 'pcx, 'tcx> MatchCtxt<'a, 'pcx, 'tcx> {
    fn new(cx: &'a CheckMirCtxt<'a, 'pcx, 'tcx>) -> Self {
        let num_blocks = cx.mir_pat.basic_blocks.len();
        let num_locals = cx.mir_pat.locals.len();
        Self {
            cx,
            matches: CheckingMatches {
                basic_blocks: IndexVec::from_fn_n(
                    |bb| CheckBlockMatches::new(&cx.body.basic_blocks, cx.mir_pat[bb].num_statements_and_terminator()),
                    num_blocks,
                ),
                locals: IndexVec::from_fn_n(|_| LocalMatches::new(num_locals), num_locals),
                ty_vars: IndexVec::from_fn_n(|_| TyVarMatches::new(), cx.fn_pat.meta.ty_vars.len()),
            },
            // succeeded: Cell::new(false),
        }
    }
    #[instrument(level = "debug", skip(self))]
    fn build_candidates(&mut self) {
        for (bb_pat, block_mat) in self.matches.basic_blocks.iter_enumerated_mut() {
            let _span = debug_span!("build_candidates", ?bb_pat).entered();
            let block_pat = &self.cx.mir_pat[bb_pat];
            for (bb, block) in self.cx.body.basic_blocks.iter_enumerated() {
                let _span = debug_span!("build_candidates", ?bb).entered();
                for (stmt_pat, matches) in block_mat.statements.iter_mut().enumerate() {
                    let loc_pat = pat::Location {
                        block: bb_pat,
                        statement_index: stmt_pat,
                    };
                    let _span = debug_span!(
                        "build_candidates",
                        ?loc_pat,
                        stmt_pat = ?self.cx.mir_pat[bb_pat].debug_stmt_at(stmt_pat),
                    )
                    .entered();
                    if loc_pat.statement_index < block_pat.statements.len()
                        && let pat::StatementKind::Assign(
                            pat::Place {
                                local: local_pat,
                                projection: [],
                            },
                            pat::Rvalue::Any,
                        ) = block_pat.statements[loc_pat.statement_index]
                    {
                        if self.cx.mir_pat.self_idx == Some(local_pat)
                            && let self_value = mir::Local::from_u32(1)
                            && self.cx.match_local(local_pat, self_value)
                        {
                            debug!("add candidate of self: {local_pat:?} <-> {self_value:?}");
                            matches.candidates.push(StatementMatch::Arg(self_value));
                        } else {
                            for arg in self.cx.body.args_iter() {
                                let _span = debug_span!("build_candidates", arg = ?StatementMatch::Arg(arg).debug_with(self.cx.body))
                                .entered();
                                if self.cx.match_local(local_pat, arg) {
                                    debug!("add candidate of arg: {local_pat:?} <-> {arg:?}");
                                    matches.candidates.push(StatementMatch::Arg(arg));
                                }
                            }
                        }
                    }
                    for stmt in 0..=block.statements.len() {
                        let loc = mir::Location {
                            block: bb,
                            statement_index: stmt,
                        };
                        let _span =
                            debug_span!("build_candidates", stmt = ?StatementMatch::Location(loc).debug_with(self.cx.body))
                                .entered();
                        if self.cx.match_statement_or_terminator(loc_pat, loc) {
                            if stmt == block.statements.len() && stmt_pat == block_pat.statements.len() {
                                block_mat.candidates.insert(bb);
                            }
                            debug!("add candidate of statement: {loc_pat:?} <-> {loc:?}");
                            matches.candidates.push(StatementMatch::Location(loc));
                        }
                    }
                }
            }
        }
        for (candidates, matches) in core::iter::zip(&self.cx.locals, &mut self.matches.locals) {
            matches.candidates = std::mem::replace(
                &mut *candidates.borrow_mut(),
                HybridBitSet::new_empty(self.cx.locals.len()),
            );
        }
        for (candidates, matches) in core::iter::zip(&self.cx.ty.ty_vars, &mut self.matches.ty_vars) {
            matches.candidates = std::mem::take(&mut *candidates.borrow_mut());
        }
    }
    #[instrument(level = "info", skip(self), ret)]
    fn do_match(&mut self) -> bool {
        self.build_candidates();
        self.matches.log_candidates();
        !self.matches.has_empty_candidates() && self.match_only_candidates() && self.match_block(pat::BasicBlock::ZERO)
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_block(&self, bb_pat: pat::BasicBlock) -> bool {
        self.matches[bb_pat]
            .candidates
            .iter()
            .any(|bb| self.match_block_ends_with(bb_pat, bb))
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_block_starts_with(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        let matches = &self.matches[bb_pat];
        matches.start.get().is_some_and(|block| block == bb)
            || matches.start.get().is_none()
                && self.match_stmt_deps_of(bb_pat, bb, self.cx.pat_ddg[bb_pat].rdep_start(), |stmt| {
                    self.cx.mir_ddg[bb].get_rdep_start(stmt).next().is_some()
                })
                && {
                    matches.start.set(Some(bb));
                    self.match_block(bb_pat)
                }
            || {
                matches.start.set(None);
                false
            }
    }
    #[instrument(level = "info", skip(self), ret)]
    fn match_block_ends_with(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        // FIXME: handle empty blocks
        if self.cx.mir_pat[bb_pat].statements.is_empty()
            && matches!(self.cx.mir_pat[bb_pat].terminator(), pat::TerminatorKind::Goto(_))
        {
            return true;
        }
        let matches = &self.matches[bb_pat];
        matches.end.get().is_some_and(|block| block == bb)
            || matches.end.get().is_none()
                && {
                    // check the terminators
                    // self.match_stmt_and_deps(self.cx.patterns.terminator_loc(bb_pat),
                    // self.cx.body.terminator_loc(bb))
                    // check the statements in order of reversed dependencies
                    self.match_stmt_deps(&mut self.cx.pat_ddg.dep_end(bb_pat), &mut |loc| {
                        self.cx
                            .mir_ddg
                            .get_dep_end(bb, loc.block, loc.statement_index)
                            .is_some()
                            || self.cx.body[loc.block].terminator().kind == mir::TerminatorKind::Return
                        // FIXME: handle move of return value
                    })
                }
                && {
                    matches.end.set(Some(bb));
                    // recursively check all the succesor blocks
                    self.match_successor_blocks(bb_pat, bb)
                }
            || {
                matches.end.set(None);
                false
            }
    }
    fn match_stmt_deps_of(
        &self,
        bb_pat: pat::BasicBlock,
        bb: mir::BasicBlock,
        pat: impl Iterator<Item = (usize, pat::Local)>,
        // mut mir: impl FnMut() -> MirIter,
        mut is_dep: impl FnMut(usize) -> bool,
    ) -> bool {
        self.match_stmt_deps(
            &mut pat.map(|(stmt, local_pat)| ((bb_pat, stmt), local_pat)),
            // || mir().map(|(stmt, local)| ((bb, stmt), local)),
            &mut |loc| loc.block == bb && is_dep(loc.statement_index),
        )
    }
    fn match_stmt_deps(
        &self,
        pat: &mut impl Iterator<Item = (impl IntoLocation<Location = pat::Location>, pat::Local)>,
        // mut mir: impl FnMut() -> MirIter,
        is_dep: &mut impl FnMut(mir::Location) -> bool,
    ) -> bool {
        let Some((loc_pat, local_pat)) = pat.next() else {
            return true;
        };
        let loc_pat = loc_pat.into_location();
        let matched = self.matches[loc_pat]
            .candidates
            .iter()
            .any(|&stmt_match| match stmt_match {
                StatementMatch::Arg(local) => self.match_local(local_pat, local),
                StatementMatch::Location(loc) => {
                    is_dep(loc)
                        && {
                            let matched = &self.matches[loc_pat].matched;
                            if matched.get().is_none() {
                                matched.set(Some(stmt_match));
                                self.log_stmt_matched(loc_pat, stmt_match);
                            }
                            self.match_stmt_locals(loc_pat, stmt_match)
                        }
                        && self.match_stmt_and_deps(loc_pat, loc)
                        && self.match_stmt_deps(&mut *pat, &mut *is_dep)
                        || {
                            self.matches[loc_pat].matched.set(None);
                            self.unmatch_stmt_locals(loc_pat);
                            false
                        }
                },
            });
        /*
        let matched = mir()
            .map(|(loc, local)| (loc.into_location(), local))
            .filter(|&(loc, local)| {
                self.matches[local_pat].candidates.contains(local)
                    && self.matches[loc_pat].candidates.contains(&loc.into())
            })
            .any(|(loc, _local)| self.match_stmt_and_deps(loc_pat, loc));
        */
        if !matched {
            info!(
                "statement not matched: {loc_pat:?} {pat:?}",
                pat = self.cx.mir_pat[loc_pat.block].debug_stmt_at(loc_pat.statement_index),
            );
        }
        matched
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_successor_blocks(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        use TerminatorEdges::{AssignOnReturn, Double, Single, SwitchInt};
        debug!(term_pat = ?self.cx.pat_cfg[bb_pat], term = ?self.cx.mir_cfg[bb]);
        match (&self.cx.pat_cfg[bb_pat], &self.cx.mir_cfg[bb]) {
            (TerminatorEdges::None, _) => true,
            (&Single(bb_pat), &Single(bb) | &Double(bb, _)) => self.match_block_starts_with(bb_pat, bb),
            (&Double(bb_pat, unwind_pat), &Double(bb, unwind)) => {
                self.match_block_starts_with(bb_pat, bb) && self.match_block_starts_with(unwind_pat, unwind)
            },
            (
                AssignOnReturn {
                    return_: box ref return_pat,
                    cleanup: cleanup_pat,
                },
                AssignOnReturn {
                    box ref return_,
                    cleanup,
                },
            ) => {
                return_pat.len() == return_.len()
                    && core::iter::zip(return_pat, return_)
                        .chain(cleanup_pat.as_ref().zip(cleanup.as_ref()))
                        .all(|(&bb_pat, &bb)| self.match_block_starts_with(bb_pat, bb))
            },
            (SwitchInt(targets_pat), SwitchInt(targets)) => {
                targets_pat.targets.iter().all(|(&value_pat, &bb_pat)| {
                    targets
                        .targets
                        .get(&value_pat)
                        .is_some_and(|&bb| self.match_block_starts_with(bb_pat, bb))
                }) && match (targets_pat.otherwise, targets.otherwise) {
                    (None, None | Some(_)) => true,
                    (Some(bb_pat), Some(bb)) => self.match_block_starts_with(bb_pat, bb),
                    (Some(_), None) => false,
                }
            },
            _ => false,
        }
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_with(&self, loc_pat: pat::Location, loc: mir::Location) -> bool {
        let matches = &self.matches[loc_pat];
        matches.matched.get().is_some()
            || matches.candidates.iter().any(|&stmt_match| match stmt_match {
                StatementMatch::Arg(_local) => self.match_stmt_locals(loc_pat, stmt_match),
                StatementMatch::Location(loc) => self.match_stmt_and_deps(loc_pat, loc),
            })
        // || self.match_stmt_in_predecessors(loc_pat, bb)
    }
    // fn direct_predecessors(&self, bb: mir::BasicBlock) -> impl Iterator<Item = mir::BasicBlock> +
    // use<'tcx, '_> {     self.cx.body.basic_blocks.predecessors()[bb]
    //         .iter()
    //         .copied()
    //         .filter(move |&pred| self.cx.mir_cfg[pred].successors().any(|target| target == bb))
    // }
    // #[instrument(level = "debug", skip(self), ret)]
    // fn match_stmt_in_predecessors(&self, loc_pat: pat::Location, bb: mir::BasicBlock) -> bool {
    //     self.direct_predecessors(bb)
    //         .any(|pred| self.match_stmt_in(loc_pat, pred))
    // }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_and_deps(&self, loc_pat: pat::Location, loc: mir::Location) -> bool {
        // let only_candidate = self.matches[loc_pat].candidates.len() == 1;
        let loc_pat = loc_pat.into_location();
        let loc = loc.into_location();
        let matched = &self.matches[loc_pat].matched;
        let stmt_match = StatementMatch::Location(loc);
        matched.get().is_none_or(|m| m == stmt_match)
            // && {
            //     if matched.get().is_none() {
            //         matched.set(Some(stmt_match));
            //         self.log_stmt_matched(loc_pat, stmt_match);
            //     }
            //     self.match_stmt_locals(loc_pat, stmt_match)
            // }
            && {
                self.match_stmt_deps(
                    &mut self.cx.pat_ddg.deps(loc_pat.block, loc_pat.statement_index),
                    &mut |dep_loc| {
                        self.cx
                            .mir_ddg
                            .get_dep(loc.block, loc.statement_index, dep_loc.block, dep_loc.statement_index)
                            .is_some()
                    },
                ) // && self.match_stmt_dep_start(loc_pat, loc.block)
            }
        // || !only_candidate && {
        //     matched.set(None);
        //     self.unmatch_stmt_locals(loc_pat);
        //     false
        // }
    }
    // #[instrument(level = "debug", skip(self), ret)]
    // fn match_stmt_dep_start(&self, loc_pat: pat::Location, bb: mir::BasicBlock) -> bool {
    //     self.cx.pat_ddg[loc_pat.block]
    //         .get_rdep_start(loc_pat.statement_index)
    //         .next()
    //         .is_none()
    //         || self.matches[loc_pat.block].start.get().is_none_or(|block| block == bb) && {
    //             self.matches[loc_pat.block].start.set(Some(bb));
    //             self.match_block_starts_with(loc_pat.block, bb)
    //         }
    //     // || self
    //     //     .direct_predecessors(bb)
    //     //     .any(|pred| self.match_stmt_dep_start(loc_pat, pred))
    // }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_locals(&self, loc_pat: pat::Location, stmt_match: StatementMatch) -> bool {
        let accesses_pat = self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index);
        let accesses = match stmt_match {
            StatementMatch::Arg(local) => &[(local, PlaceContext::MutatingUse(MutatingUseContext::Store))],
            StatementMatch::Location(loc) => self.cx.mir_ddg[loc.block].accesses(loc.statement_index),
        };
        if loc_pat.statement_index < self.cx.mir_pat[loc_pat.block].statements.len()
            && let pat::StatementKind::Assign(
                pat::Place {
                    local: local_pat,
                    projection: [],
                },
                pat::Rvalue::Any,
            ) = self.cx.mir_pat[loc_pat.block].statements[loc_pat.statement_index]
        {
            return accesses
                .iter()
                .find(|&&(_, access)| access.is_place_assignment())
                .is_some_and(|&(local, _)| self.match_local(local_pat, local));
        }
        let mut iter = accesses.iter();
        accesses_pat.iter().all(|&(local_pat, access_pat)| {
            debug!(?local_pat, ?access_pat);
            iter.by_ref()
                .inspect(|&&(local, access)| debug!(?local, ?access))
                .find(|&&(_, access)| access == access_pat)
                .is_some_and(|&(local, _)| self.match_local(local_pat, local))
        })
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_local(&self, local_pat: pat::Local, local: mir::Local) -> bool {
        if self.matches[local_pat].matched.r#match(local) {
            self.log_local_matched(local_pat, local);
        } else {
            self.log_local_conflicted(local_pat, local);
            return false;
        }
        self.match_local_ty(self.cx.mir_pat.locals[local_pat], self.cx.body.local_decls[local].ty)
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_local_ty(&self, ty_pat: pat::Ty<'pcx>, ty: Ty<'tcx>) -> bool {
        self.cx.ty.match_ty(ty_pat, ty)
            && self.cx.ty.ty_vars.iter_enumerated().all(|(ty_var, tys)| {
                let ty = match &core::mem::take(&mut *tys.borrow_mut())[..] {
                    [] => return true,
                    &[ty] => ty,
                    [..] => return false,
                };
                self.match_ty_var(ty_var, ty)
            })
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_ty_var(&self, ty_var: pat::TyVarIdx, ty: Ty<'tcx>) -> bool {
        if self.matches[ty_var].matched.r#match(ty) {
            self.log_ty_var_matched(ty_var, ty);
        } else {
            self.log_ty_var_conflicted(ty_var, ty);
            return false;
        }
        true
    }
    #[instrument(level = "debug", skip(self))]
    fn unmatch_stmt_locals(&self, loc_pat: pat::Location) {
        for &(local_pat, _) in self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index) {
            self.unmatch_local(local_pat);
        }
    }
    #[instrument(level = "debug", skip(self))]
    fn unmatch_local(&self, local_pat: pat::Local) {
        self.matches[local_pat].matched.unmatch();
        if let &pat::TyKind::TyVar(ty_var) = self.cx.mir_pat.locals[local_pat].kind() {
            self.matches[ty_var.idx].matched.unmatch();
        }
    }

    fn log_stmt_matched(&self, loc_pat: impl IntoLocation<Location = pat::Location>, stmt_match: StatementMatch) {
        let loc_pat = loc_pat.into_location();
        info!(
            "statement matched {loc_pat:?} {pat:?} <-> {stmt_match:?} {statement:?}",
            pat = self.cx.mir_pat[loc_pat.block].debug_stmt_at(loc_pat.statement_index),
            statement = stmt_match.debug_with(self.cx.body),
        );
    }
    fn log_local_conflicted(&self, local_pat: pat::Local, local: mir::Local) {
        let conflicted_local = self.matches[local_pat].matched.get().unwrap().into_inner();
        info!(
            "local conflicted: {local_pat:?}: {ty_pat:?} !! {local:?} / {conflicted_local:?}: {ty:?}",
            ty_pat = self.cx.mir_pat.locals[local_pat],
            ty = self.cx.body.local_decls[conflicted_local].ty,
        );
    }
    fn log_local_matched(&self, local_pat: pat::Local, local: mir::Local) {
        debug!(
            "local matched: {local_pat:?}: {ty_pat:?} <-> {local:?}: {ty:?}",
            ty_pat = self.cx.mir_pat.locals[local_pat],
            ty = self.cx.body.local_decls[local].ty,
        );
    }
    fn log_ty_var_conflicted(&self, ty_var: pat::TyVarIdx, ty: Ty<'tcx>) {
        let conflicted_ty = self.matches[ty_var].matched.get().unwrap().into_inner();
        info!("type variable conflicted, {ty_var:?}: {ty:?} !! {conflicted_ty:?}");
    }
    fn log_ty_var_matched(&self, ty_var: pat::TyVarIdx, ty: Ty<'tcx>) {
        debug!("type variable matched, {ty_var:?} <-> {ty:?}");
    }

    /// If there is only one candidate, set the matched value.
    #[instrument(level = "debug", skip(self))]
    fn match_only_candidates(&self) -> bool {
        self.matches.basic_blocks.iter_enumerated().all(|(bb_pat, block_pat)| {
            block_pat.statements.iter().enumerate().all(|(stmt_pat, matches)| {
                if let &[stmt_match] = &matches.candidates[..] {
                    let loc_pat = (bb_pat, stmt_pat).into_location();
                    matches.matched.set(Some(stmt_match));
                    self.log_stmt_matched(loc_pat, stmt_match);
                    return self.match_stmt_locals(loc_pat, stmt_match);
                }
                true
            }) && self.matches.locals.iter_enumerated().all(|(local_pat, matches)| {
                if let Ok(local) = matches.candidates.iter().exactly_one() {
                    return self.match_local(local_pat, local);
                }
                true
            }) && self.matches.ty_vars.iter_enumerated().all(|(ty_var_idx, matches)| {
                if let &[ty] = &matches.candidates[..] {
                    return self.match_ty_var(ty_var_idx, ty);
                }
                true
            })
        })
    }
}

impl<'tcx> CheckingMatches<'tcx> {
    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self) -> bool {
        self.basic_blocks.iter().any(CheckBlockMatches::has_empty_candidates)
            || self.locals.iter().any(LocalMatches::has_empty_candidates)
            || self.ty_vars.iter().any(TyVarMatches::has_empty_candidates)
    }

    #[instrument(level = "debug", skip(self))]
    fn log_candidates(&self) {
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            debug!("{bb:?}: {:?}", block.candidates);
            for (index, stmt) in block.statements.iter().enumerate() {
                debug!("{bb:?}[{index}]: {:?}", stmt.candidates);
            }
        }
        for (local, matches) in self.locals.iter_enumerated() {
            debug!("{local:?}: {:?}", matches.candidates);
        }
        for (ty_var, matches) in self.ty_vars.iter_enumerated() {
            debug!("{ty_var:?}: {:?}", matches.candidates);
        }
    }

    #[instrument(level = "debug", skip_all)]
    fn log_matches(&self, body: &mir::Body<'tcx>) {
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            debug!("{bb:?} <-> [{:?}, {:?}]", block.start.get(), block.end.get());
            for (index, stmt) in block.statements.iter().enumerate() {
                debug!(
                    "{bb:?}[{index}] <-> {:?}",
                    stmt.matched.get().map(|matched| matched.debug_with(body))
                );
            }
        }
        for (local, matches) in self.locals.iter_enumerated() {
            debug!("{local:?} <-> {:?}", matches.matched.get());
        }
        for (ty_var, matches) in self.ty_vars.iter_enumerated() {
            debug!("{ty_var:?}: {:?}", matches.matched.get());
        }
    }

    fn try_into_matches(self) -> Result<Matches<'tcx>, MatchFailed> {
        let basic_blocks = self
            .basic_blocks
            .into_iter()
            .map(CheckBlockMatches::into_matches)
            .collect();
        let locals = self.locals.into_iter().map(LocalMatches::try_take).try_collect()?;
        let ty_vars = self.ty_vars.into_iter().map(TyVarMatches::try_take).try_collect()?;
        Ok(Matches {
            basic_blocks,
            locals,
            ty_vars,
        })
    }
}

#[derive(Debug)]
struct CheckBlockMatches {
    candidates: HybridBitSet<mir::BasicBlock>,
    statements: Vec<StatementMatches>,
    start: Cell<Option<mir::BasicBlock>>,
    end: Cell<Option<mir::BasicBlock>>,
}

impl CheckBlockMatches {
    fn new(blocks: &mir::BasicBlocks<'_>, num_stmts: usize) -> Self {
        Self {
            candidates: HybridBitSet::new_empty(blocks.len()),
            statements: core::iter::repeat_with(Default::default).take(num_stmts).collect(),
            start: Cell::new(None),
            end: Cell::new(None),
        }
    }
    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty() || self.statements.iter().any(StatementMatches::has_empty_candidates)
    }

    fn into_matches(self) -> BlockMatches {
        BlockMatches {
            statements: self.statements.into_iter().map(StatementMatches::take).collect(),
            start: self.start.take(),
            end: self.end.take(),
        }
    }
}

#[derive(Default, Debug)]
struct StatementMatches {
    matched: Cell<Option<StatementMatch>>,
    candidates: Vec<StatementMatch>,
}

impl StatementMatches {
    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self) -> bool {
        if let &[m] = &self.candidates[..] {
            self.matched.set(Some(m));
        }
        self.candidates.is_empty()
    }

    fn take(self) -> Option<StatementMatch> {
        self.matched.take()
    }
}

#[derive(Debug)]
struct LocalMatches {
    matched: CountedMatch<mir::Local>,
    candidates: HybridBitSet<mir::Local>,
}

impl LocalMatches {
    fn new(num_locals: usize) -> Self {
        Self {
            matched: CountedMatch::default(),
            candidates: HybridBitSet::new_empty(num_locals),
        }
    }

    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }

    fn try_take(self) -> Result<mir::Local, MatchFailed> {
        self.matched.try_take()
    }
}

#[derive(Default, Debug)]
struct TyVarMatches<'tcx> {
    matched: CountedMatch<Ty<'tcx>>,
    candidates: Vec<Ty<'tcx>>,
}

impl<'tcx> TyVarMatches<'tcx> {
    fn new() -> Self {
        Self::default()
    }

    fn try_take(self) -> Result<Ty<'tcx>, MatchFailed> {
        self.matched.try_take()
    }

    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }
}

struct CountedMatch<T>(Cell<Option<Counted<T>>>);

impl<T> Default for CountedMatch<T> {
    fn default() -> Self {
        Self(Cell::new(None))
    }
}

impl<T: Copy + PartialEq> CountedMatch<T> {
    fn get(&self) -> Option<Counted<T>> {
        self.0.get()
    }
    fn r#match(&self, value: T) -> bool {
        match self.0.get() {
            None => self.0.set(Some(Counted::new(value))),
            Some(l) if l.value == value => self.0.set(Some(l.inc())),
            Some(_) => return false,
        }
        true
    }
    fn unmatch(&self) {
        self.0.update(|m| m.and_then(Counted::dec));
    }
    fn try_take(self) -> Result<T, MatchFailed> {
        self.0.take().map(Counted::into_inner).ok_or(MatchFailed)
    }
}

#[derive(Clone, Copy)]
struct Counted<T> {
    value: T,
    count: NonZero<u32>,
}

impl<T> Counted<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            count: NonZero::<u32>::MIN,
        }
    }
    fn into_inner(self) -> T {
        self.value
    }
    fn inc(self) -> Self {
        Self {
            count: self.count.checked_add(1).unwrap(),
            ..self
        }
    }
    fn dec(self) -> Option<Self> {
        Some(Self {
            count: NonZero::new(self.count.get().wrapping_sub(1))?,
            ..self
        })
    }
}

impl<T: fmt::Debug> fmt::Debug for Counted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {{{}}}", self.value, self.count)
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for CountedMatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.get().fmt(f)
    }
}

trait IntoLocation: Copy {
    type Location;
    fn into_location(self) -> Self::Location;
}

impl IntoLocation for pat::Location {
    type Location = pat::Location;

    fn into_location(self) -> Self::Location {
        self
    }
}

impl IntoLocation for (pat::BasicBlock, usize) {
    type Location = pat::Location;

    fn into_location(self) -> Self::Location {
        pat::Location {
            block: self.0,
            statement_index: self.1,
        }
    }
}

impl IntoLocation for mir::Location {
    type Location = mir::Location;

    fn into_location(self) -> Self::Location {
        self
    }
}

impl IntoLocation for (mir::BasicBlock, usize) {
    type Location = mir::Location;

    fn into_location(self) -> Self::Location {
        mir::Location {
            block: self.0,
            statement_index: self.1,
        }
    }
}
