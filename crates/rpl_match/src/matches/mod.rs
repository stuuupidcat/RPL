use std::cell::Cell;
use std::fmt;
use std::ops::Index;

use rpl_constraints::Const;
use rpl_constraints::attributes::ExtraSpan;
use rpl_context::pat::{LabelMap, Spanned};
use rpl_mir_graph::TerminatorEdges;
use rustc_data_structures::stack::ensure_sufficient_stack;
use rustc_hir::FnDecl;
use rustc_index::bit_set::MixedBitSet;
use rustc_index::IndexVec;
use rustc_middle::mir::visit::PlaceContext;
use rustc_middle::mir::{self, PlaceRef};
use rustc_middle::ty::Ty;
use rustc_span::{Span, Symbol};

use crate::CountedMatch;
use crate::mir::{CheckMirCtxt, pat};
use crate::solver::variable::{VarDomain, VarSlot};
use crate::statement::MatchStatement as _;
use crate::ty::MatchTy as _;

pub mod artifact;
mod color;

#[derive(Debug)]
pub struct Matched<'tcx> {
    pub basic_blocks: IndexVec<pat::BasicBlock, MatchedBlock>,
    pub locals: IndexVec<pat::Local, mir::Local>,
    pub ty_vars: IndexVec<pat::TyVarIdx, Ty<'tcx>>,
    pub const_vars: IndexVec<pat::ConstVarIdx, Const<'tcx>>,
    pub place_vars: IndexVec<pat::PlaceVarIdx, PlaceRef<'tcx>>,
}

impl Matched<'_> {
    pub(crate) fn log_matched(&self) {
        use tracing::debug as info;
        info!("pat block <-> mir candidate blocks");
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            info!("pat stmt <-> mir candidate statements");
            for (index, stmt) in block.statements.iter().enumerate() {
                info!("    {bb:?}[{index}]: {:?}", stmt);
            }
        }
        info!("pat local <-> mir candidate locals");
        for (local, matches) in self.locals.iter_enumerated() {
            info!("{local:?}: {:?}", matches);
        }
        info!("pat ty metavar <-> mir candidate types");
        for (ty_var, matches) in self.ty_vars.iter_enumerated() {
            info!("{ty_var:?}: {:?}", matches);
        }
        info!("pat const metavar <-> mir candidate constants");
        for (const_var, matches) in self.const_vars.iter_enumerated() {
            info!("{const_var:?}: {:?}", matches);
        }
        info!("pat place metavar <-> mir candidate places");
        for (place_var, matches) in self.place_vars.iter_enumerated() {
            info!("{place_var:?}: {:?}", matches);
        }
    }

    fn span_spanned<'tcx>(&self, spanned: Spanned, body: &mir::Body<'tcx>, decl: &FnDecl<'tcx>) -> Span {
        match spanned {
            Spanned::Location(location) => self[location].span_no_inline(body),
            Spanned::Local(local) => body.local_decls[self[local]].source_info.span,
            // Special case for the function name, which is not a label.
            Spanned::Body => body.span,
            Spanned::Output => decl.output.span(),
        }
    }
}

#[derive(Debug)]
pub struct MatchedWithLabelMap<'a, 'tcx>(pub &'a LabelMap, pub &'a Matched<'tcx>, pub &'a ExtraSpan<'tcx>);

impl<'tcx> pat::Matched<'tcx> for MatchedWithLabelMap<'_, 'tcx> {
    fn span(&self, body: &mir::Body<'tcx>, decl: &FnDecl<'tcx>, name: &str) -> Span {
        let MatchedWithLabelMap(labels, matched, attr) = self;
        let name = Symbol::intern(name);
        labels
            .get(&name)
            .map(|spanned| matched.span_spanned(*spanned, body, decl))
            .or_else(|| attr.get(&name).map(|attr| attr.span))
            .unwrap_or_else(|| {
                panic!("label `{name}` not found in:\n    pattern labels: {labels:?}\n    attributes: {attr:?}");
            })
    }
    fn type_meta_var(&self, idx: pat::TyVarIdx) -> Ty<'tcx> {
        self.1.ty_vars[idx]
    }
    fn const_meta_var(&self, idx: pat::ConstVarIdx) -> Const<'tcx> {
        self.1.const_vars[idx]
    }
    fn place_meta_var(&self, idx: pat::PlaceVarIdx) -> PlaceRef<'tcx> {
        self.1.place_vars[idx]
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MatchedBlock {
    pub statements: Vec<StatementMatch>,
    pub start: Option<mir::BasicBlock>,
    pub end: Option<mir::BasicBlock>,
}

impl Index<pat::BasicBlock> for Matched<'_> {
    type Output = MatchedBlock;

    fn index(&self, bb: pat::BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

impl Index<pat::Location> for Matched<'_> {
    type Output = StatementMatch;

    fn index(&self, stmt: pat::Location) -> &Self::Output {
        &self.basic_blocks[stmt.block].statements[stmt.statement_index]
    }
}

impl Index<pat::Local> for Matched<'_> {
    type Output = mir::Local;

    fn index(&self, local: pat::Local) -> &Self::Output {
        &self.locals[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for Matched<'tcx> {
    type Output = Ty<'tcx>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars[ty_var]
    }
}

impl<'tcx> Index<pat::ConstVarIdx> for Matched<'tcx> {
    type Output = Const<'tcx>;

    fn index(&self, ty_var: pat::ConstVarIdx) -> &Self::Output {
        &self.const_vars[ty_var]
    }
}

impl<'tcx> Index<pat::PlaceVarIdx> for Matched<'tcx> {
    type Output = PlaceRef<'tcx>;

    fn index(&self, place_var: pat::PlaceVarIdx) -> &Self::Output {
        &self.place_vars[place_var]
    }
}

pub fn matches<'tcx>(cx: &CheckMirCtxt<'_, '_, 'tcx>) -> Vec<Matched<'tcx>> {
    let mut matching = MatchCtxt::new(cx);
    matching.do_match();
    matching.matched.take()
}

#[derive(Debug)]
struct Matching<'tcx> {
    basic_blocks: IndexVec<pat::BasicBlock, MatchingBlock>,
    locals: VarDomain<pat::Local, mir::Local>,
    ty_vars: VarDomain<pat::TyVarIdx, Ty<'tcx>>,
    const_vars: VarDomain<pat::ConstVarIdx, Const<'tcx>>,
    place_vars: VarDomain<pat::PlaceVarIdx, PlaceRef<'tcx>>,
    /// Track which pattern statement the statement is matched to.
    mir_statements: IndexVec<mir::BasicBlock, MirStatementBackMatch>,
}

impl Index<pat::BasicBlock> for Matching<'_> {
    type Output = MatchingBlock;

    fn index(&self, bb: pat::BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

impl Index<pat::Location> for Matching<'_> {
    type Output = StatementMatches;

    fn index(&self, stmt: pat::Location) -> &Self::Output {
        &self.basic_blocks[stmt.block].statements[stmt.statement_index]
    }
}

impl Index<pat::Local> for Matching<'_> {
    type Output = VarSlot<mir::Local>;

    fn index(&self, local: pat::Local) -> &Self::Output {
        &self.locals.vars[local]
    }
}

impl<'tcx> Index<pat::TyVarIdx> for Matching<'tcx> {
    type Output = VarSlot<Ty<'tcx>>;

    fn index(&self, ty_var: pat::TyVarIdx) -> &Self::Output {
        &self.ty_vars.vars[ty_var]
    }
}

impl<'tcx> Index<pat::ConstVarIdx> for Matching<'tcx> {
    type Output = VarSlot<Const<'tcx>>;

    fn index(&self, const_var: pat::ConstVarIdx) -> &Self::Output {
        &self.const_vars.vars[const_var]
    }
}

impl<'tcx> Index<pat::PlaceVarIdx> for Matching<'tcx> {
    type Output = VarSlot<PlaceRef<'tcx>>;

    fn index(&self, place_var: pat::PlaceVarIdx) -> &Self::Output {
        &self.place_vars.vars[place_var]
    }
}

#[derive(Debug)]
struct MirStatementBackMatch {
    matched: IndexVec<usize, CountedMatch<pat::Location>>,
}

impl MirStatementBackMatch {
    fn new(n: usize) -> Self {
        Self {
            matched: IndexVec::from_elem_n(CountedMatch::new(), n),
        }
    }
    fn r#match(&self, loc_pat: pat::Location, loc: mir::Location) -> bool {
        debug_assert!(loc.statement_index <= self.matched.len());
        if loc.statement_index < self.matched.len() {
            let matcher = &self.matched[loc.statement_index];
            let matched = matcher.r#match(loc_pat);
            debug!("match_stmt {loc:?} ({matcher:?}) <-> {loc_pat:?}");
            if !matched {
                debug!(?loc_pat, ?loc, ?matched, ?matcher, "match_stmt conflicted");
            }
            matched
        } else {
            true
        }
    }
    fn unmatch(&self, loc_pat: pat::Location, loc: mir::Location) {
        debug_assert!(loc.statement_index <= self.matched.len());
        if loc.statement_index < self.matched.len() {
            let matcher = &self.matched[loc.statement_index];
            matcher.unmatch();
            debug!("unmatch_stmt {loc_pat:?} <-> {loc:?} ({matcher:?})");
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatementMatch {
    /// An argument of the function.
    Arg(mir::Local),
    /// A statement or terminator in the MIR graph.
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
    fn expect_location(&self) -> mir::Location {
        match self {
            StatementMatch::Location(loc) => *loc,
            _ => panic!("expect location"),
        }
    }

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
                        |f, stmt| write!(f, "{location:?}: {stmt:?}"),
                        |f, terminator| write!(f, "{location:?}: {:?}", terminator.kind),
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

    pub fn span(self, body: &mir::Body<'_>) -> Span {
        self.source_info(body).span
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

    pub fn is_arg(self, body: &mir::Body<'_>) -> bool {
        match self {
            StatementMatch::Arg(local) => local_is_arg(local, body),
            StatementMatch::Location(_) => false,
        }
    }
}

#[inline]
#[instrument(level = "trace", skip(body), ret)]
pub fn local_is_arg(local: mir::Local, body: &mir::Body<'_>) -> bool {
    local.as_usize() > 0 && local.as_usize() < body.arg_count + 1
}

struct MatchCtxt<'a, 'pcx, 'tcx> {
    cx: &'a CheckMirCtxt<'a, 'pcx, 'tcx>,
    matching: Matching<'tcx>,
    matched: Cell<Vec<Matched<'tcx>>>,
}

impl<'a, 'pcx, 'tcx> MatchCtxt<'a, 'pcx, 'tcx> {
    fn new(cx: &'a CheckMirCtxt<'a, 'pcx, 'tcx>) -> Self {
        Self {
            cx,
            matching: Self::new_checking(cx),
            matched: Cell::new(Vec::new()),
        }
    }
    fn new_checking(cx: &'a CheckMirCtxt<'a, 'pcx, 'tcx>) -> Matching<'tcx> {
        let num_blocks = cx.mir_pat.basic_blocks.len();
        let num_locals = cx.mir_pat.locals.len();
        let mir_statements = IndexVec::from_fn_n(
            |bb| MirStatementBackMatch::new(cx.body[bb].statements.len()),
            cx.body.basic_blocks.len(),
        );
        Matching {
            basic_blocks: IndexVec::from_fn_n(
                |bb_pat| {
                    let mut num_stmt_pats = cx.mir_pat[bb_pat].num_statements_and_terminator();
                    // We don't need to match the end of the pattern, because it is only a marker and has no
                    // corresponding terminator.
                    if cx.mir_pat[bb_pat].has_pat_end() {
                        num_stmt_pats -= 1;
                    }
                    MatchingBlock::new(num_stmt_pats)
                },
                num_blocks,
            ),
            locals: VarDomain::new(num_locals),
            ty_vars: VarDomain::new(cx.fn_pat.meta.ty_vars.len()),
            const_vars: VarDomain::new(cx.fn_pat.meta.const_vars.len()),
            place_vars: VarDomain::new(cx.fn_pat.meta.place_vars.len()),
            mir_statements,
        }
    }
    #[instrument(level = "debug", skip(self))]
    fn build_candidates(&mut self) {
        if !self.cx.match_ret_ty() {
            return;
        }
        for (bb_pat, block_mat) in self.matching.basic_blocks.iter_enumerated_mut() {
            let _span = debug_span!("build_candidates", ?bb_pat).entered();
            let block_pat = &self.cx.mir_pat[bb_pat];
            for (stmt_pat, matches) in block_mat.statements.iter_mut().enumerate() {
                let loc_pat = (bb_pat, stmt_pat).into_location();
                let _span = debug_span!(
                    "build_candidates",
                    ?loc_pat,
                    stmt_pat = ?self.cx.mir_pat[bb_pat].debug_stmt_at(stmt_pat),
                )
                .entered();
                // Note that this should be outside of the `self.cx.body.basic_blocks.iter_enumerated()` loop to
                // avoid duplicated argument candidates.
                if loc_pat.statement_index < block_pat.statements.len()
                    && let pat::StatementKind::Assign(
                        pat::Place {
                            base: pat::PlaceBase::Local(local_pat),
                            projection: [],
                        },
                        pat::Rvalue::Any,
                    ) = block_pat.statements[loc_pat.statement_index]
                {
                    if self.cx.mir_pat.self_idx == Some(local_pat) && self.cx.has_self {
                        let self_value = mir::Local::from_u32(1);
                        if self.cx.match_local(local_pat, self_value) {
                            info!(
                                "candidate matched: {loc_pat:?} (self) {pat:?} <-> {self_value:?}",
                                pat = self.cx.mir_pat[bb_pat].debug_stmt_at(stmt_pat),
                            );

                            matches.candidates.push(StatementMatch::Arg(self_value));
                        }
                    } else {
                        for arg in self.cx.body.args_iter() {
                            let _span = debug_span!("build_candidates", arg = ?StatementMatch::Arg(arg).debug_with(self.cx.body))
                                .entered();
                            if self.cx.match_local(local_pat, arg) {
                                info!(
                                    "candidate matched: {loc_pat:?} {pat:?} <-> {arg:?}",
                                    pat = self.cx.mir_pat[bb_pat].debug_stmt_at(stmt_pat),
                                );
                                matches.candidates.push(StatementMatch::Arg(arg));
                            }
                        }
                    }
                }
                for (bb, block) in self.cx.body.basic_blocks.iter_enumerated() {
                    let _span = debug_span!("build_candidates", ?bb).entered();
                    for stmt in 0..=block.statements.len() {
                        let loc = (bb, stmt).into_location();
                        let _span =
                            debug_span!("build_candidates", stmt = ?StatementMatch::Location(loc).debug_with(self.cx.body))
                                .entered();
                        if self.cx.match_statement_or_terminator(loc_pat, loc) {
                            info!(
                                "candidate matched: {loc_pat:?} {pat:?} <-> {statement:?}",
                                pat = self.cx.mir_pat[bb_pat].debug_stmt_at(stmt_pat),
                                statement = StatementMatch::Location(loc).debug_with(self.cx.body),
                            );
                            matches.candidates.push(StatementMatch::Location(loc));
                        }
                    }
                }
            }
        }
        for ((local_pat, candidates), slot) in
            core::iter::zip(self.cx.locals.iter_enumerated(), &mut self.matching.locals.vars)
        {
            let bitset = std::mem::replace(
                &mut *candidates.borrow_mut(),
                MixedBitSet::new_empty(self.cx.body.local_decls.len()),
            );
            slot.candidates = bitset.iter().collect();
            if slot.candidates.is_empty() {
                continue;
            }
            // If the local variable is the `self` parameter or the `RET` place, we only need to match the
            // corresponding local variable in the MIR graph.
            let only_candidate = if self.cx.mir_pat.self_idx == Some(local_pat) {
                mir::Local::from_u32(1)
            } else if self.cx.mir_pat.return_idx == Some(local_pat) {
                mir::RETURN_PLACE
            } else {
                continue;
            };
            slot.candidates.retain(|&l| l == only_candidate);
        }
        for (candidates, slot) in core::iter::zip(&self.cx.ty.ty_vars, &mut self.matching.ty_vars.vars) {
            slot.candidates = std::mem::take(&mut *candidates.borrow_mut()).into_iter().collect();
        }
        for (candidates, slot) in core::iter::zip(&self.cx.ty.const_vars, &mut self.matching.const_vars.vars) {
            slot.candidates = std::mem::take(&mut *candidates.borrow_mut()).into_iter().collect();
        }
        for (candidates, slot) in core::iter::zip(&self.cx.places, &mut self.matching.place_vars.vars) {
            slot.candidates = std::mem::take(&mut *candidates.borrow_mut()).into_iter().collect();
        }
    }
    #[instrument(level = "info", skip(self), fields(?pat_name = self.cx.pat_name, ?fn_name = self.cx.fn_pat.name))]
    fn do_match(&mut self) {
        self.build_candidates();
        self.matching.log_candidates();
        if !self.matching.has_empty_candidates(self.cx) {
            self.match_candidates();
            self.log_matched();
        }
    }
    fn log_matched(&self) {
        let matched = self.matched.take();
        debug!("log matched candidates: {}", matched.len());
        for (index, matched) in matched.iter().enumerate() {
            debug!("candidate {index}");
            matched.log_matched();
        }
        self.matched.set(matched);
    }
    fn assert_ty_var_free(&self) {
        #[cfg(feature = "strict")]
        debug_assert!(self.matching.ty_vars.vars.iter().all(|c| c.get().is_none()));
    }
    fn assert_const_var_free(&self) {
        #[cfg(feature = "strict")]
        debug_assert!(self.matching.const_vars.vars.iter().all(|c| c.get().is_none()));
    }
    fn assert_place_var_free(&self) {
        #[cfg(feature = "strict")]
        debug_assert!(self.matching.place_vars.vars.iter().all(|c| c.get().is_none()));
    }
    fn assert_local_free(&self) {
        #[cfg(feature = "strict")]
        debug_assert!(self.matching.locals.vars.iter().all(|c| c.get().is_none()));
    }
    fn assert_stmt_free(&self) {
        #[cfg(feature = "strict")]
        debug_assert!(
            self.matching
                .mir_statements
                .iter()
                .all(|s| s.matched.iter().all(|c| c.get().is_none()))
        );
    }
    // Recursively traverse all candidates of type variables, local variables, and statements, and then
    // match the graph.
    #[instrument(level = "info", skip(self))]
    fn match_candidates(&self) {
        let loc_pats = self.loc_pats().collect::<Vec<_>>();
        self.assert_ty_var_free();
        self.matching.ty_vars.backtrack(
            pat::TyVarIdx::ZERO,
            // CountedMatch is managed internally by VarDomain::backtrack
            &|_ty_var, _cand| true,
            &|_ty_var| {},
            &mut || {
                if !self.match_ret_ty() {
                    return;
                }
                self.assert_const_var_free();
                self.matching.const_vars.backtrack(
                    pat::ConstVarIdx::ZERO,
                    &|_const_var, _cand| true,
                    &|_const_var| {},
                    &mut || {
                        self.assert_place_var_free();
                        self.matching.place_vars.backtrack(
                            pat::PlaceVarIdx::ZERO,
                            &|_place_var, _cand| true,
                            &|_place_var| {},
                            &mut || {
                                self.assert_local_free();
                                self.matching.locals.backtrack(
                                    pat::Local::ZERO,
                                    &|local, cand| self.match_local_ty(
                                        self.cx.mir_pat.locals[local],
                                        self.cx.body.local_decls[cand].ty,
                                    ),
                                    &|_local| {},
                                    &mut || {
                                        self.assert_stmt_free();
                                        self.match_stmt_candidates(&loc_pats);
                                        self.assert_stmt_free();
                                    },
                                );
                                self.assert_local_free();
                            },
                        );
                        self.assert_place_var_free();
                    },
                );
                self.assert_const_var_free();
            },
        );
        self.assert_ty_var_free();
    }
    fn match_stmt_candidates(&self, loc_pats: &[pat::Location]) {
        let Some((&loc_pat, loc_pats)) = loc_pats.split_first() else {
            if self.match_graph() {
                self.matching.log_matched(self.cx);
                let mut matched = self.matched.take();
                matched.push(self.matching.to_matched());
                self.matched.set(matched);
            }
            return;
        };
        for &cand in &self.matching[loc_pat].candidates {
            let _span = debug_span!("match_stmt_candidate", ?loc_pat, ?cand).entered();
            if self.match_stmt(loc_pat, cand) {
                // recursion
                ensure_sufficient_stack(|| self.match_stmt_candidates(loc_pats));
                // backtrack, clear status
                self.unmatch_stmt(loc_pat);
            }
        }
    }

    #[instrument(level = "info", skip(self), ret)]
    fn match_graph(&self) -> bool {
        for block in &self.matching.basic_blocks {
            block.start.take();
            block.end.take();
        }
        self.match_cfg() && self.match_ddg()
    }

    #[instrument(level = "info", skip(self), ret)]
    fn match_cfg(&self) -> bool {
        self.match_block(pat::BasicBlock::ZERO)
    }

    #[instrument(level = "info", skip(self), ret)]
    fn match_ddg(&self) -> bool {
        self.loc_pats().all(|loc_pat| {
            let StatementMatch::Location(loc) = self.matching[loc_pat].force_get_matched() else {
                return true;
            };
            let matched = self.match_stmt_deps(
                self.cx.pat_ddg.deps(loc_pat.block, loc_pat.statement_index),
                |dep_loc, local| {
                    let dep_local =
                        self.cx
                            .mir_ddg
                            .get_dep(loc.block, loc.statement_index, dep_loc.block, dep_loc.statement_index);
                    trace!(?dep_loc, ?local, ?dep_local);
                    dep_local == Some(local)
                    // dep_local.is_none_or(|dep_local| dep_local == local)
                },
            );
            debug!(?loc_pat, ?loc, ?matched, "match_stmt_deps");
            matched
        })
    }

    fn loc_pats(&self) -> impl Iterator<Item = pat::Location> + use<'_> {
        self.matching
            .basic_blocks
            .iter_enumerated()
            .flat_map(|(bb, block)| (0..block.statements.len()).map(move |stmt| (bb, stmt).into_location()))
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn match_block(&self, bb_pat: pat::BasicBlock) -> bool {
        if self.cx.mir_pat[bb_pat].has_pat_end() {
            return true;
        }
        let block = self.matching[bb_pat]
            .terminator()
            .force_get_matched()
            .expect_location()
            .block;
        self.match_block_ends_with(bb_pat, block)
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_block_starts_with(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
        let matching = &self.matching[bb_pat];
        matching.start.get().is_some_and(|block| block == bb)
            || matching.start.get().is_none()
                && self.match_stmt_deps(
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
                    ensure_sufficient_stack(|| self.match_block(bb_pat))
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
    // of `match_block_ends_with` only tries to match `?bb0` with `bb0`, it will fail to match the
    // `bla.len()` statement in `bb3` with `?bb2` due to no data dependency edge can be found from
    // `bla.len()` to the end of `bb0`.
    #[instrument(level = "debug", skip(self), ret)]
    fn match_block_ends_with(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
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
                && self.match_stmt_deps(self.cx.pat_ddg.dep_end(bb_pat), |dep_loc, local| {
                    self.cx.mir_ddg.get_dep_end(bb, dep_loc.block, dep_loc.statement_index)
                        .map(|dep_end| dep_end == local).unwrap_or(true)
                })
                && {
                    matching.end.set(Some(bb));
                    // recursively check all the successor blocks
                    self.match_block_successors(bb_pat, bb)
                }
            || {
                matching.end.set(None);
                false
            }
    }

    /// Match DDG edges of a statement, or the start or end of a block (See 3 callers of this
    /// function).
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
    fn match_stmt_deps(
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
                "match_stmt_deps: {dep_loc_pat:?} <-> {dep_stmt:?}, {local_pat:?} <-> {local:?}",
            );
            matched
        })
    }
    #[instrument(level = "debug", skip(self), ret)]
    fn match_block_successors(&self, bb_pat: pat::BasicBlock, bb: mir::BasicBlock) -> bool {
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
                    return_: box return_pat,
                    cleanup: cleanup_pat,
                },
                AssignOnReturn { box return_, cleanup },
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

    /// Used in [`MatchCtxt::match_candidates`].
    ///
    /// # Returns
    ///
    /// - `true` if the statement is matched. The `matched` field of the [`StatementMatches`] is set
    ///   to the matched statement.
    /// - `false` if the statement is not matched. Nothing should be changed.
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt(&self, loc_pat: pat::Location, stmt_match: StatementMatch) -> bool {
        self.match_stmt_inner(loc_pat, stmt_match)
            && if let StatementMatch::Location(loc) = stmt_match {
                let bb = &self.matching.mir_statements[loc.block];
                bb.r#match(loc_pat, loc)
            } else {
                true
            }
            && {
                self.matching[loc_pat].matched.set(Some(stmt_match));
                true
            }
    }
    #[instrument(level = "debug", skip(self))]
    fn unmatch_stmt(&self, loc_pat: pat::Location) {
        self.unmatch_stmt_adt_matches(loc_pat);
        // self.unmatch_stmt_locals(loc_pat);

        debug_assert!(self.matching[loc_pat].matched.get().is_some());
        if let Some(StatementMatch::Location(loc)) = self.matching[loc_pat].matched.get() {
            let bb = &self.matching.mir_statements[loc.block];
            bb.unmatch(loc_pat, loc);
        }
        self.matching[loc_pat].matched.set(None);
    }
    fn place_context_compatible(place_context_pat: PlaceContext, place_context: PlaceContext, is_copy: bool) -> bool {
        let _ = place_context;
        let _ = place_context_pat;
        let _ = is_copy;
        true
        // place_context_pat == place_context
        // || match (place_context_pat, place_context) {
        //     (PlaceContext::MutatingUse(_), PlaceContext::MutatingUse(_))
        //     | (PlaceContext::NonMutatingUse(_), PlaceContext::NonMutatingUse(_)) => true,
        //     (PlaceContext::NonMutatingUse(_), PlaceContext::MutatingUse(_))
        //     | (PlaceContext::MutatingUse(_), PlaceContext::NonMutatingUse(_)) => is_copy,
        //     _ => false,
        // }
    }

    /// Check if `loc_pat` has the same structure as `stmt_match`.
    #[instrument(level = "debug", skip(self), ret)]
    fn match_stmt_inner(&self, loc_pat: pat::Location, stmt_match: StatementMatch) -> bool {
        let pat_block = &self.cx.fn_pat.expect_body()[loc_pat.block];
        debug_assert!(loc_pat.statement_index <= pat_block.statements.len());
        match stmt_match {
            StatementMatch::Arg(arg) => {
                if loc_pat.statement_index == pat_block.statements.len() {
                    // An argument does not match the end of a basic block in the pattern.
                    false
                } else {
                    let pat_stmt = &pat_block.statements[loc_pat.statement_index];
                    match pat_stmt {
                        pat::StatementKind::Assign(place, value) => {
                            place
                                .as_local()
                                .is_some_and(|local_pat| self.matching.locals.force_get(local_pat) == arg)
                                && matches!(value, pat::Rvalue::Any)
                        },
                        pat::StatementKind::Intrinsic(_) => false,
                    }
                }
            },
            StatementMatch::Location(loc) => self.match_statement_or_terminator(loc_pat, loc),
        }
        // let accesses_pat = self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index);
        // let accesses = match stmt_match {
        //     StatementMatch::Arg(local) => &[(local,
        // PlaceContext::MutatingUse(MutatingUseContext::Store))],
        //     StatementMatch::Location(loc) =>
        // self.cx.mir_ddg[loc.block].accesses(loc.statement_index), };
        // if loc_pat.statement_index < self.cx.mir_pat[loc_pat.block].statements.len()
        //     && let pat::StatementKind::Assign(
        //         pat::Place {
        //             base: pat::PlaceBase::Local(local_pat),
        //             projection: [],
        //         },
        //         pat::Rvalue::Any,
        //     ) = self.cx.mir_pat[loc_pat.block].statements[loc_pat.statement_index]
        // {
        //     return accesses
        //         .iter()
        //         .find(|&&(_, access)| access.is_place_assignment())
        //         // .is_some_and(|&(local, _)| self.match_local(local_pat, local));
        //         .is_some_and(|&(local, _)| self.matching[local_pat].force_get_matched() ==
        // local); }
        // let mut iter = accesses.iter();
        // accesses_pat.iter().all(|&(local_pat, access_pat)| {
        //     debug!(?local_pat, ?access_pat);
        //     let matched_loc = self.matching[local_pat].force_get_matched();
        //     let tcx = self.cx.ty.tcx;
        //     let ty = self.cx.body.local_decls()[matched_loc].ty;
        //     let is_copy = tcx.type_is_copy_modulo_regions(self.cx.ty.typing_env, ty);
        //     accesses
        //         .iter()
        //         .inspect(|&(local, access)| debug!(?local, ?access))
        //         .any(|&(local, access)| {
        //             Self::place_context_compatible(access, access_pat, is_copy) && matched_loc ==
        // local         })
        //     // .find(|&&(_, access)| Self::place_context_compatible(access, access_pat, is_copy))
        //     // .is_some_and(|&(local, _)| self.match_local(local_pat, local))
        //     // .is_some_and(|&(local, _)| matched_loc == local)
        //     // .any(|&(local, _)| self.matching[local_pat].force_get_matched() == local)
        // })
    }

    #[instrument(level = "debug", skip(self), ret)]
    fn match_local_ty(&self, ty_pat: pat::Ty<'pcx>, ty: Ty<'tcx>) -> bool {
        self.match_ty(ty_pat, ty)
        // self.cx.ty.match_ty(ty_pat, ty)
        //     && self.cx.ty.ty_vars.iter_enumerated().all(|(ty_var, tys)| {
        //         let tys = core::mem::take(&mut *tys.borrow_mut());
        //         trace!("type variable {ty_var:?} candidates: {tys:?}",);
        //         let ty = match tys {
        //             tys if tys.is_empty() => return true,
        //             tys if tys.len() == 1 => tys.iter().copied().next().unwrap(),
        //             tys => {
        //                 info!("multiple candidates for type variable {ty_var:?}: {tys:?}",);
        //                 return false;
        //             },
        //         };
        //         let ty_var_matched = self.matching[ty_var].force_get_matched();
        //         trace!("type variable {ty_var:?} matched: {ty_var_matched:?} matching: {ty:?}",);
        //         // self.match_ty_var(ty_var, ty)
        //         ty_var_matched == ty
        //     })
    }
    // #[instrument(level = "debug", skip(self))]
    // fn unmatch_stmt_locals(&self, loc_pat: pat::Location) {
    //     for &(local_pat, _) in self.cx.pat_ddg[loc_pat.block].accesses(loc_pat.statement_index) {
    //         self.unmatch_local(local_pat);
    //     }
    // }
    fn unmatch_stmt_adt_matches(&self, loc_pat: pat::Location) {
        let Some(StatementMatch::Location(loc)) = self.matching[loc_pat].matched.get() else {
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

    fn log_stmt_matched(&self, loc_pat: impl IntoLocation<Location = pat::Location>, stmt_match: StatementMatch) {
        let loc_pat = loc_pat.into_location();
        debug!(
            "statement matched {loc_pat:?} {pat:?} <-> {stmt_match:?} {statement:?}",
            pat = self.cx.mir_pat[loc_pat.block].debug_stmt_at(loc_pat.statement_index),
            statement = stmt_match.debug_with(self.cx.body),
        );
    }
    fn log_local_conflicted(&self, local_pat: pat::Local, local: mir::Local) {
        let conflicted_local = self.matching[local_pat].matched.get().unwrap();
        debug!(
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
    fn log_ty_var_matched(&self, ty_var: pat::TyVarIdx, ty: Ty<'tcx>) {
        debug!("type variable matched, {ty_var:?} <-> {ty:?}");
    }
}

impl<'tcx> Matching<'tcx> {
    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self, cx: &CheckMirCtxt<'_, '_, 'tcx>) -> bool {
        self.basic_blocks
            .iter_enumerated()
            .any(|(bb, matching)| matching.has_empty_candidates(cx, bb))
            || self.locals.vars.iter_enumerated().any(|(local, slot)| {
                slot.has_empty_candidates() && {
                    info!("Local {local:?} has no candidates");
                    true
                }
            })
        // may declare a type variable without using it.
        // || self.ty_vars.has_empty_candidates()
    }

    #[instrument(level = "info", skip(self))]
    fn log_candidates(&self) {
        info!("pat block <-> mir candidate blocks");
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            info!("pat stmt <-> mir candidate statements");
            for (index, stmt) in block.statements.iter().enumerate() {
                info!("    {bb:?}[{index}]: {:?}", stmt.candidates);
            }
        }
        info!("pat local <-> mir candidate locals");
        for (local, slot) in self.locals.vars.iter_enumerated() {
            info!("{local:?}: {:?}", slot.candidates);
        }
        info!("pat ty metavar <-> mir candidate types");
        for (ty_var, slot) in self.ty_vars.vars.iter_enumerated() {
            info!("{ty_var:?}: {:?}", slot.candidates);
        }
        info!("pat const metavar <-> mir candidate constants");
        for (const_var, slot) in self.const_vars.vars.iter_enumerated() {
            info!("{const_var:?}: {:?}", slot.candidates);
        }
        info!("pat place metavar <-> mir candidate places");
        for (place_var, slot) in self.place_vars.vars.iter_enumerated() {
            info!("{place_var:?}: {:?}", slot.candidates);
        }
    }

    #[instrument(level = "info", skip_all)]
    fn log_matched(&self, cx: &CheckMirCtxt<'_, '_, 'tcx>) {
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            for (index, stmt) in block.statements.iter().enumerate() {
                info!(
                    "{bb:?}[{index}]: {:?} <-> {:?}",
                    cx.mir_pat[bb].debug_stmt_at(index),
                    stmt.matched.get().map(|matched| matched.debug_with(cx.body))
                );
            }
        }
        for (local, slot) in self.locals.vars.iter_enumerated() {
            info!("{local:?} <-> {:?}", slot.get());
        }
        for (ty_var, slot) in self.ty_vars.vars.iter_enumerated() {
            info!("{ty_var:?}: {:?}", slot.get());
        }
        for (const_var, slot) in self.const_vars.vars.iter_enumerated() {
            info!("{const_var:?}: {:?}", slot.get());
        }
        for (place_var, slot) in self.place_vars.vars.iter_enumerated() {
            info!("{place_var:?}: {:?}", slot.get());
        }
    }

    fn to_matched(&self) -> Matched<'tcx> {
        let basic_blocks = self
            .basic_blocks
            .iter_enumerated()
            .map(|(bb, matching)| matching.to_matched(bb))
            .collect();
        let locals = self.locals.to_matched();
        let ty_vars = self.ty_vars.to_matched();
        let const_vars = self.const_vars.to_matched();
        let place_vars = self.place_vars.to_matched();

        Matched {
            basic_blocks,
            locals,
            ty_vars,
            const_vars,
            place_vars,
        }
    }
}

#[derive(Debug)]
struct MatchingBlock {
    statements: Vec<StatementMatches>,
    start: Cell<Option<mir::BasicBlock>>,
    end: Cell<Option<mir::BasicBlock>>,
}

impl MatchingBlock {
    fn new(num_stmts: usize) -> Self {
        Self {
            statements: core::iter::repeat_with(Default::default).take(num_stmts).collect(),
            start: Cell::new(None),
            end: Cell::new(None),
        }
    }
    /// Test if there are any empty candidates in the matches.
    fn has_empty_candidates(&self, cx: &CheckMirCtxt<'_, '_, '_>, bb: pat::BasicBlock) -> bool {
        self.statements
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

    fn terminator(&self) -> &StatementMatches {
        self.statements.last().expect("bug: empty block")
    }

    fn to_matched(&self, bb_pat: pat::BasicBlock) -> MatchedBlock {
        MatchedBlock {
            statements: self
                .statements
                .iter()
                .enumerate()
                .map(|(i, stmt)| {
                    stmt.get()
                        .unwrap_or_else(|| panic!("bug: statement {bb_pat:?}[{i}] not matched"))
                })
                .collect(),
            start: self.start.get(),
            end: self.end.get(),
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

    fn get(&self) -> Option<StatementMatch> {
        self.matched.get()
    }

    // After `match_stmt_candidates`, all statements are supposed to be matched,
    // so we can assume that `self.matched` is `Some`.
    #[track_caller]
    fn force_get_matched(&self) -> StatementMatch {
        self.matched.get().expect("bug: statement not matched")
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
