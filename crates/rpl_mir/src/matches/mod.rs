use std::cell::Cell;
use std::fmt;
use std::ops::Index;

use itertools::Itertools;
use rpl_match::CountedMatch;
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

pub fn matches<'tcx>(cx: &CheckMirCtxt<'_, '_, 'tcx>) -> Option<Vec<Matches<'tcx>>> {
    let mut matching = MatchCtxt::new(cx);
    if matching.do_match() {
        // return matching.matches.try_into_matches().ok();
        let vec = matching
            .succeed
            .into_inner()
            .into_iter()
            .filter_map(|matches| matches.try_into_matches().ok())
            .collect();
        return Some(vec);
    }
    None
}

struct MatchFailed;

#[derive(Clone)]
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
    succeed: Cell<Vec<CheckingMatches<'tcx>>>,
    succeeded: Cell<bool>,
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
            succeed: Cell::new(Vec::new()),
            succeeded: Cell::new(false),
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
        if self.matches.has_empty_candidates(self.cx) {
            return false;
        }
        let locs_pat = self.get_locs_pat();
        self.match_candidates(&locs_pat, 0);
        self.succeeded.get()
    }
    // Enumerate all (stmt_pat, cand) pairs recursively,
    // then try to match(embed) graph
    #[instrument(level = "debug", skip(self, locs_pat))]
    fn match_candidates(&self, locs_pat: &Vec<pat::Location>, index: usize) {
        if index == locs_pat.len() {
            if self.match_graph() {
                info!("Code instance matched");
                self.matches.log_matches(self.cx.body);
                let mut vec = self.succeed.take();
                vec.push(self.matches.clone());
                self.succeed.set(vec);
                self.succeeded.set(true);
            }
            return;
        };
        let loc_pat = locs_pat[index];
        let stmt_pat = &self.matches[loc_pat];
        for cand in &stmt_pat.candidates {
            if self.match_stmt_locals(loc_pat, *cand) {
                // set status
                stmt_pat.matched.set(Some(*cand));
                // recursion
                self.match_candidates(locs_pat, index + 1);
                // backtrack, clear status
                stmt_pat.matched.set(None);
            }
            // backtrack, clear status
            self.unmatch_stmt_locals(loc_pat);
        }
    }

    fn match_graph(&self) -> bool {
        self.match_cfg() && self.match_ddg()
    }

    fn match_cfg(&self) -> bool {
        self.match_block(pat::BasicBlock::ZERO)
    }

    fn match_ddg(&self) -> bool {
        // dep_loc_pat -----> dep_loc
        //   ^                   ^
        //   | local_pat         | local
        //   |                   |
        // loc_pat -------> stmt_match
        let locs_pat = self.get_locs_pat();
        locs_pat.into_iter().all(|loc_pat| self.match_stmt_deps(loc_pat))
    }

    fn get_locs_pat(&self) -> Vec<pat::Location> {
        let mut vec: Vec<pat::Location> = self
            .matches
            .basic_blocks
            .iter_enumerated()
            .flat_map(|(bb, block)| (0..block.statements.len()).map(move |stmt| (bb, stmt).into_location()))
            .collect();
        vec.remove(vec.len() - 1);
        vec
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
                && self.match_stmt_deps_of(bb_pat, self.cx.pat_ddg[bb_pat].rdep_start())
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
                    // FIXME: handle move of return value
                    self.cx
                        .pat_ddg
                        .dep_end(bb_pat)
                        .all(|((bb, stmt), _)| self.match_stmt_deps((bb, stmt).into_location()))
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
    fn match_stmt_deps_of(&self, bb_pat: pat::BasicBlock, mut pat: impl Iterator<Item = (usize, pat::Local)>) -> bool {
        pat.all(|(stmt, _)| self.match_stmt_deps((bb_pat, stmt).into_location()))
    }
    fn match_stmt_deps(&self, loc_pat: pat::Location) -> bool {
        let loc_pat = loc_pat.into_location();
        let stmt_match = self.matches[loc_pat].matched.get().unwrap();

        match stmt_match {
            StatementMatch::Arg(_) => true,
            StatementMatch::Location(loc) => {
                let mut pat_dep_edges = self.cx.pat_ddg[loc_pat.block].deps(loc_pat.statement_index);
                pat_dep_edges.all(|(dep_loc_pat, local_pat)| {
                    let is_dep = |local| self.matches[local_pat].matched.get().is_some_and(|l| l == local);
                    let dep_loc = self.matches[loc_pat.block].statements[dep_loc_pat]
                        .matched
                        .get()
                        .unwrap();
                    match dep_loc {
                        StatementMatch::Arg(local) => is_dep(local),
                        StatementMatch::Location(dep_loc) => self
                            .cx
                            .mir_ddg
                            .get_dep(loc.block, loc.statement_index, dep_loc.block, dep_loc.statement_index)
                            .is_some_and(is_dep),
                    }
                })
            },
        }
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
        self.unmatch_stmt_adt_matches(loc_pat);
        for &(local_pat, _) in self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index) {
            self.unmatch_local(local_pat);
        }
    }
    fn unmatch_stmt_adt_matches(&self, loc_pat: pat::Location) {
        let Some(StatementMatch::Location(loc)) = self.matches[loc_pat].matched.get() else {
            return;
        };
        use mir::visit::Visitor;
        use pat::visitor::PatternVisitor;
        struct CollectPlaces<P> {
            places: Vec<P>,
        }
        impl<'pcx> PatternVisitor<'pcx> for CollectPlaces<pat::Place<'pcx>> {
            fn visit_place(&mut self, place: pat::Place<'pcx>, pcx: PlaceContext, loc: pat::Location) {
                self.places.push(place);
                self.super_place(place, pcx, loc);
            }
        }
        impl<'tcx> Visitor<'tcx> for CollectPlaces<mir::Place<'tcx>> {
            fn visit_place(&mut self, &place: &mir::Place<'tcx>, pcx: PlaceContext, loc: mir::Location) {
                self.places.push(place);
                self.super_place(&place, pcx, loc);
            }
        }
        let mut place_pats = CollectPlaces::<pat::Place<'_>> { places: Vec::new() };
        let mut places = CollectPlaces::<mir::Place<'_>> { places: Vec::new() };
        self.cx.mir_pat.stmt_at(loc_pat).either_with(
            &mut place_pats,
            |place_pats, statement| place_pats.visit_statement(statement, loc_pat),
            |place_pats, terminator| place_pats.visit_terminator(terminator, loc_pat),
        );
        self.cx.body.stmt_at(loc).either_with(
            &mut places,
            |places, statement| places.visit_statement(statement, loc),
            |places, terminator| places.visit_terminator(terminator, loc),
        );
        for (place_pat, place) in core::iter::zip(place_pats.places, places.places) {
            self.cx.unmatch_place(place_pat, place);
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
        let conflicted_local = self.matches[local_pat].matched.get().unwrap();
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
        let conflicted_ty = self.matches[ty_var].matched.get().unwrap();
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
    fn has_empty_candidates(&self, cx: &CheckMirCtxt<'_, '_, 'tcx>) -> bool {
        self.basic_blocks
            .iter_enumerated()
            .any(|(bb, matches)| matches.has_empty_candidates(cx, bb))
            || self.locals.iter().any(LocalMatches::has_empty_candidates)
        // may declare a type variable without using it.
        // || self.ty_vars.iter().any(TyVarMatches::has_empty_candidates)
    }

    #[instrument(level = "debug", skip(self))]
    fn log_candidates(&self) {
        debug!("pat block <-> mir candidate blocks"); // how to bold the text?
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            debug!("{bb:?}: {:?}", block.candidates);
            debug!("pat stmt <-> mir candidate statements");
            for (index, stmt) in block.statements.iter().enumerate() {
                debug!("    {bb:?}[{index}]: {:?}", stmt.candidates);
            }
        }
        debug!("pat local <-> mir candidate locals");
        for (local, matches) in self.locals.iter_enumerated() {
            debug!("{local:?}: {:?}", matches.candidates);
        }
        debug!("pat ty metavar <-> mir candidate types");
        for (ty_var, matches) in self.ty_vars.iter_enumerated() {
            debug!("{ty_var:?}: {:?}", matches.candidates);
        }
    }

    #[instrument(level = "info", skip_all)]
    fn log_matches(&self, body: &mir::Body<'tcx>) {
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            info!("{bb:?} <-> [{:?}, {:?}]", block.start.get(), block.end.get());
            for (index, stmt) in block.statements.iter().enumerate() {
                info!(
                    "{bb:?}[{index}] <-> {:?}",
                    stmt.matched.get().map(|matched| matched.debug_with(body))
                );
            }
        }
        for (local, matches) in self.locals.iter_enumerated() {
            info!("{local:?} <-> {:?}", matches.matched.get());
        }
        for (ty_var, matches) in self.ty_vars.iter_enumerated() {
            info!("{ty_var:?}: {:?}", matches.matched.get());
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

#[derive(Debug, Clone)]
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
    fn has_empty_candidates(&self, cx: &CheckMirCtxt<'_, '_, '_>, bb: pat::BasicBlock) -> bool {
        self.candidates.is_empty()
            || self
                .statements
                .iter()
                .position(StatementMatches::has_empty_candidates)
                .inspect(|&stmt| {
                    info!(
                        "Statement {bb:?}[{stmt}] has no candidates: {:?}",
                        cx.mir_pat[bb].debug_stmt_at(stmt)
                    )
                })
                .is_some()
    }

    fn into_matches(self) -> BlockMatches {
        BlockMatches {
            statements: self.statements.into_iter().map(StatementMatches::take).collect(),
            start: self.start.take(),
            end: self.end.take(),
        }
    }
}

#[derive(Default, Debug, Clone)]
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

#[derive(Debug, Clone)]
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
        self.matched.try_take().ok_or(MatchFailed)
    }
}

#[derive(Default, Debug, Clone)]
struct TyVarMatches<'tcx> {
    matched: CountedMatch<Ty<'tcx>>,
    candidates: Vec<Ty<'tcx>>,
}

impl<'tcx> TyVarMatches<'tcx> {
    fn new() -> Self {
        Self::default()
    }

    fn try_take(self) -> Result<Ty<'tcx>, MatchFailed> {
        self.matched.try_take().ok_or(MatchFailed)
    }

    /// Test if there are any empty candidates in the matches.
    #[allow(unused)]
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
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
