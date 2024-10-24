use std::cell::Cell;
use std::ops::Index;

use rpl_mir_graph::TerminatorEdges;
use rustc_index::bit_set::HybridBitSet;
use rustc_index::IndexVec;
use rustc_middle::mir;
use rustc_middle::mir::visit::{MutatingUseContext, PlaceContext};
use rustc_middle::ty::Ty;
use rustc_span::Span;

use crate::{pat, CheckMirCtxt};

pub struct Matches<'tcx> {
    basic_blocks: IndexVec<pat::BasicBlock, BlockMatches>,
    locals: IndexVec<pat::LocalIdx, mir::Local>,
    ty_vars: IndexVec<pat::TyVarIdx, Ty<'tcx>>,
}

pub struct BlockMatches {
    statements: Vec<Option<StatementMatch>>,
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

impl Index<pat::LocalIdx> for Matches<'_> {
    type Output = mir::Local;

    fn index(&self, local: pat::LocalIdx) -> &Self::Output {
        &self.locals[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for Matches<'tcx> {
    type Output = Ty<'tcx>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars[ty_var]
    }
}

pub fn matches<'tcx>(cx: &CheckMirCtxt<'_, 'tcx>) -> Option<Matches<'tcx>> {
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
    locals: IndexVec<pat::LocalIdx, LocalMatches>,
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

impl Index<pat::LocalIdx> for CheckingMatches<'_> {
    type Output = LocalMatches;

    fn index(&self, local: pat::LocalIdx) -> &Self::Output {
        &self.locals[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for CheckingMatches<'tcx> {
    type Output = TyVarMatches<'tcx>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars[ty_var]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementMatch {
    Arg(mir::Local),
    Location(mir::Location),
}

impl StatementMatch {
    fn is_in_block(self, block: mir::BasicBlock) -> bool {
        match self {
            StatementMatch::Arg(_) => true,
            StatementMatch::Location(loc) => loc.block == block,
        }
    }

    fn debug_with<'a, 'tcx>(self, body: &'a mir::Body<'tcx>) -> impl core::fmt::Debug + use<'a, 'tcx> {
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
        if let Some(parent_scope) = body.source_scopes[scope].inlined_parent_scope {
            scope = parent_scope;
        }
        if let Some((_instance, span)) = body.source_scopes[scope].inlined {
            return span;
        }
        source_info.span
    }
}

struct MatchCtxt<'a, 'tcx> {
    cx: &'a CheckMirCtxt<'a, 'tcx>,
    matches: CheckingMatches<'tcx>,
    // succeeded: Cell<bool>,
}

impl<'a, 'tcx> MatchCtxt<'a, 'tcx> {
    fn new(cx: &'a CheckMirCtxt<'a, 'tcx>) -> Self {
        let num_blocks = cx.patterns.basic_blocks.len();
        let num_locals = cx.patterns.locals.len();
        Self {
            cx,
            matches: CheckingMatches {
                basic_blocks: IndexVec::from_fn_n(
                    |bb| CheckBlockMatches::new(&cx.body.basic_blocks, cx.patterns[bb].num_statements_and_terminator()),
                    num_blocks,
                ),
                locals: IndexVec::from_fn_n(|_| LocalMatches::new(num_locals), num_locals),
                ty_vars: IndexVec::from_fn_n(|_| TyVarMatches::new(), cx.patterns.ty_vars.len()),
            },
            // succeeded: Cell::new(false),
        }
    }
    #[instrument(level = "info", skip(self))]
    fn build_candidates(&mut self) {
        for (bb_pat, block_mat) in self.matches.basic_blocks.iter_enumerated_mut() {
            let block_pat = &self.cx.patterns[bb_pat];
            for (bb, block) in self.cx.body.basic_blocks.iter_enumerated() {
                for (stmt_pat, matches) in block_mat.statements.iter_mut().enumerate() {
                    let loc_pat = pat::Location {
                        block: bb_pat,
                        statement_index: stmt_pat,
                    };
                    if loc_pat.statement_index < block_pat.statements.len()
                        && let pat::StatementKind::Init(pat::Place {
                            local: local_pat,
                            projection: [],
                        }) = block_pat.statements[loc_pat.statement_index]
                    {
                        if self.cx.patterns.self_idx == Some(local_pat)
                            && let self_value = mir::Local::from_u32(1)
                            && self.cx.match_local(local_pat, self_value)
                        {
                            debug!("add candidate of self: {local_pat:?} <-> {self_value:?}");
                            matches.candidates.push(StatementMatch::Arg(self_value));
                        } else {
                            for arg in self.cx.body.args_iter() {
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
        for (candidates, matches) in core::iter::zip(&self.cx.ty_vars, &mut self.matches.ty_vars) {
            matches.candidates = std::mem::take(&mut *candidates.borrow_mut());
        }
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn do_match(&mut self) -> bool {
        self.build_candidates();
        self.matches.log_candidates();
        !self.matches.has_empty_candidates() && self.match_block(pat::BasicBlock::ZERO)
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
        matches.visited.get()
            || {
                matches.visited.set(true);
                matches.start.set(Some(bb));
                self.match_block(bb_pat)
            }
            || {
                matches.visited.set(false);
                matches.start.set(None);
                false
            }
    }
    #[instrument(level = "info", skip(self), ret)]
    fn match_block_ends_with(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        let matches = &self.matches[bb_pat];
        matches.visited.get()
            || {
                matches.visited.set(true);

                // check the statements in order of reversed dependencies
                self.cx.pat_ddg[bb_pat].dep_end().all(|statement_index| {
                    let loc_pat = pat::Location {
                        block: bb_pat,
                        statement_index,
                    };
                    self.match_stmt_in(loc_pat, bb)
                })
            } && {
                matches.end.set(Some(bb));
                // recursively check all the succesor blocks
                self.match_successor_blocks(bb_pat, bb)
            }
            || {
                matches.visited.set(false);
                matches.end.set(None);
                false
            }
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_successor_blocks(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        use TerminatorEdges::{AssignOnReturn, Double, Single, SwitchInt};
        match (&self.cx.pat_cfg[bb_pat], &self.cx.mir_cfg[bb]) {
            (TerminatorEdges::None, TerminatorEdges::None) => true,
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
    fn match_stmt_in(&self, loc_pat: pat::Location, bb: mir::BasicBlock) -> bool {
        let matches = &self.matches[loc_pat];
        matches.matched.get().is_some()
            || matches
                .candidates
                .iter()
                .filter(|stmt_match| stmt_match.is_in_block(bb))
                .any(|&stmt_match| self.match_stmt_and_deps(loc_pat, bb, stmt_match))
            || self.match_stmt_in_predecessors(loc_pat, bb)
    }
    fn direct_predecessors(&self, bb: mir::BasicBlock) -> impl Iterator<Item = mir::BasicBlock> + use<'tcx, '_> {
        self.cx.body.basic_blocks.predecessors()[bb]
            .iter()
            .copied()
            .filter(move |&pred| {
                matches!(
                    self.cx.mir_cfg[pred],
                    TerminatorEdges::Single(target) | TerminatorEdges::Double(target, _)
                    | TerminatorEdges::AssignOnReturn { return_: box [target], .. } if target == bb
                )
            })
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_in_predecessors(&self, loc_pat: pat::Location, bb: mir::BasicBlock) -> bool {
        self.direct_predecessors(bb)
            .any(|pred| self.match_stmt_in(loc_pat, pred))
    }
    #[instrument(level = "info", skip(self), ret)]
    fn match_stmt_and_deps(&self, loc_pat: pat::Location, bb: mir::BasicBlock, stmt_match: StatementMatch) -> bool {
        let matched = &self.matches[loc_pat].matched;
        matched.get().is_none_or(|m| m == stmt_match)
            && {
                matched.set(Some(stmt_match));
                info!(
                    pat = ?self.cx.patterns[loc_pat.block].debug_stmt_at(loc_pat.statement_index),
                    statement = ?stmt_match.debug_with(self.cx.body),
                    "statement matched",
                );
                self.match_stmt_locals(loc_pat, stmt_match)
            }
            && self.cx.pat_ddg[loc_pat.block]
                .deps(loc_pat.statement_index)
                .all(|statement_index| {
                    let loc_pat = pat::Location {
                        statement_index,
                        ..loc_pat
                    };
                    self.match_stmt_in(loc_pat, bb)
                })
            && self.match_stmt_dep_start(loc_pat, bb)
            || {
                matched.set(None);
                self.unmatch_stmt_locals(loc_pat);
                false
            }
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_dep_start(&self, loc_pat: pat::Location, bb: mir::BasicBlock) -> bool {
        !self.cx.pat_ddg[loc_pat.block].is_rdep_start(loc_pat.statement_index)
            || self.matches[loc_pat.block].start.get() == Some(bb)
            || self
                .direct_predecessors(bb)
                .any(|pred| self.match_stmt_dep_start(loc_pat, pred))
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_locals(&self, loc_pat: pat::Location, stmt_match: StatementMatch) -> bool {
        let accesses_pat = self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index);
        let accesses = match stmt_match {
            StatementMatch::Arg(local) => &[(local, PlaceContext::MutatingUse(MutatingUseContext::Store))],
            StatementMatch::Location(loc) => self.cx.mir_ddg[loc.block].accesses(loc.statement_index),
        };
        if loc_pat.statement_index < self.cx.patterns[loc_pat.block].statements.len()
            && let pat::StatementKind::Init(pat::Place {
                local: local_pat,
                projection: [],
            }) = self.cx.patterns[loc_pat.block].statements[loc_pat.statement_index]
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
    fn match_local(&self, local_pat: pat::LocalIdx, local: mir::Local) -> bool {
        self.matches[local_pat].matched.get().is_none_or(|l| l == local) && {
            self.matches[local_pat].matched.set(Some(local));
            debug!(
                ?local_pat, ?local,
                ty_pat = ?self.cx.patterns.locals[local_pat],
                ty = ?self.cx.body.local_decls[local].ty,
                "local matched",
            );
            self.match_local_ty(self.cx.patterns.locals[local_pat], self.cx.body.local_decls[local].ty)
        }
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_local_ty(&self, ty_pat: pat::Ty<'tcx>, ty: Ty<'tcx>) -> bool {
        self.cx.match_ty(ty_pat, ty) && {
            self.cx.ty_vars.iter_enumerated().all(|(ty_var, tys)| {
                let ty = match &core::mem::take(&mut *tys.borrow_mut())[..] {
                    [] => return true,
                    &[ty] => ty,
                    [..] => return false,
                };
                debug!(?ty_var, ?ty, "type variable matched");
                self.matches[ty_var].matched.set(Some(ty));
                true
            })
        }
    }
    #[instrument(level = "debug", skip(self))]
    fn unmatch_stmt_locals(&self, loc_pat: pat::Location) {
        for &(local_pat, _) in self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index) {
            self.unmatch_local(local_pat);
        }
    }
    #[instrument(level = "debug", skip(self))]
    fn unmatch_local(&self, local_pat: pat::LocalIdx) {
        self.matches[local_pat].matched.set(None);
        if let &pat::TyKind::TyVar(ty_var) = self.cx.patterns.locals[local_pat].kind() {
            self.matches[ty_var].matched.set(None);
        }
    }
}

impl<'tcx> CheckingMatches<'tcx> {
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
    visited: Cell<bool>,
    start: Cell<Option<mir::BasicBlock>>,
    end: Cell<Option<mir::BasicBlock>>,
}

impl CheckBlockMatches {
    fn new(blocks: &mir::BasicBlocks<'_>, num_stmts: usize) -> Self {
        Self {
            candidates: HybridBitSet::new_empty(blocks.len()),
            statements: core::iter::repeat_with(Default::default).take(num_stmts).collect(),
            visited: Cell::new(false),
            start: Cell::new(None),
            end: Cell::new(None),
        }
    }
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
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }

    fn take(self) -> Option<StatementMatch> {
        self.matched.take()
    }
}

#[derive(Debug)]
struct LocalMatches {
    matched: Cell<Option<mir::Local>>,
    candidates: HybridBitSet<mir::Local>,
}

impl LocalMatches {
    fn new(num_locals: usize) -> Self {
        Self {
            matched: Cell::new(None),
            candidates: HybridBitSet::new_empty(num_locals),
        }
    }
    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }

    fn try_take(self) -> Result<mir::Local, MatchFailed> {
        self.matched.take().ok_or(MatchFailed)
    }
}

#[derive(Default, Debug)]
struct TyVarMatches<'tcx> {
    matched: Cell<Option<Ty<'tcx>>>,
    candidates: Vec<Ty<'tcx>>,
}

impl<'tcx> TyVarMatches<'tcx> {
    fn new() -> Self {
        Self::default()
    }

    fn try_take(self) -> Result<Ty<'tcx>, MatchFailed> {
        self.matched.take().ok_or(MatchFailed)
    }

    fn has_empty_candidates(&self) -> bool {
        self.candidates.is_empty()
    }
}
