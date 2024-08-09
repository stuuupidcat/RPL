use std::ops::Try;

use rustc_data_structures::intern::Interned;
use rustc_data_structures::sync::Lock;
use rustc_index::bit_set::{GrowableBitSet, HybridBitSet};
use rustc_index::IndexVec;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};
use rustc_span::{Span, Symbol};
use rustc_target::abi::{FieldIdx, VariantIdx};
use visitor::PatternVisitor;

pub use Operand::{Copy, Move};

pub mod visitor;

pub struct Pattern<'tcx> {
    pub kind: PatternKind<'tcx>,
    children: HybridBitSet<PatternIdx>,
    matches: Lock<GrowableBitSet<MatchIdx>>,
}

impl<'tcx> std::ops::Index<PatternIdx> for Patterns<'tcx> {
    type Output = Pattern<'tcx>;

    fn index(&self, index: PatternIdx) -> &Self::Output {
        &self.patterns[index]
    }
}

rustc_index::newtype_index! {
    #[debug_format = "?Pat{}"]
    #[orderable]
    pub struct PatternIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?T{}"]
    pub struct TyVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "?C{}"]
    pub struct ConstVarIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "_?{}"]
    pub struct LocalIdx {}
}

rustc_index::newtype_index! {
    #[debug_format = "Mat{}"]
    pub struct MatchIdx {}
}

#[derive(Default)]
pub struct Patterns<'tcx> {
    pub local_count: usize,
    pub locals: IndexVec<LocalIdx, Local<'tcx>>,
    pub patterns: IndexVec<PatternIdx, Pattern<'tcx>>,
    pub matches: Lock<IndexVec<MatchIdx, Match<'tcx>>>,
}

pub struct Match<'tcx> {
    pub pat: PatternIdx,
    pub kind: MatchKind<'tcx>,
    // pub children: HybridBitSet<MatchIdx>,
}

impl<'tcx> Match<'tcx> {
    fn new(pat: PatternIdx, kind: MatchKind<'tcx>) -> Self {
        Self { pat, kind }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MatchKind<'tcx> {
    TyVar(ty::Ty<'tcx>),
    ConstVar(ty::Const<'tcx>),
    Argument(mir::Local),
    Statement(mir::Location),
    Terminator(mir::BasicBlock, Option<mir::BasicBlock>),
}

impl<'tcx> MatchKind<'tcx> {
    pub fn expect_ty_var(self) -> ty::Ty<'tcx> {
        match self {
            MatchKind::TyVar(ty) => ty,
            _ => rustc_middle::bug!("expect `TyVar`, but found {self:?}"),
        }
    }
    pub fn expect_const_var(self) -> ty::Const<'tcx> {
        match self {
            MatchKind::ConstVar(konst) => konst,
            _ => rustc_middle::bug!("expect `ConstVar`, but found {self:?}"),
        }
    }
    pub fn span(&self, body: &mir::Body<'tcx>) -> Option<Span> {
        Some(match *self {
            MatchKind::Argument(local) => body.local_decls[local].source_info.span,
            MatchKind::Statement(location) => body.source_info(location).span,
            MatchKind::Terminator(block, _) => body[block].terminator().source_info.span,
            MatchKind::ConstVar(_) | MatchKind::TyVar(_) => return None,
        })
    }
}

#[derive(Debug)]
pub enum PatternKind<'tcx> {
    TyVar,
    ConstVar,
    Init(LocalIdx),
    Statement(StatementKind<'tcx>),
    Terminator(TerminatorKind<'tcx>),
}

pub struct ConstVar<'tcx> {
    pub konst: ty::Const<'tcx>,
}

pub struct Local<'tcx> {
    pub ty: Ty<'tcx>,
    pub latest_pat: Option<PatternIdx>,
    pub matches: HybridBitSet<mir::Local>,
}

impl<'tcx> Local<'tcx> {
    fn new(ty: Ty<'tcx>, locals: usize) -> Self {
        Self {
            ty,
            latest_pat: None,
            matches: HybridBitSet::new_empty(locals),
        }
    }
}

pub type PlaceElem<'tcx> = mir::ProjectionElem<LocalIdx, Ty<'tcx>>;

#[derive(Debug, Clone, Copy)]
pub struct Place<'tcx> {
    pub local: LocalIdx,
    pub projection: &'tcx [PlaceElem<'tcx>],
}

impl<'tcx> Place<'tcx> {
    pub fn as_local(&self) -> Option<LocalIdx> {
        self.projection.is_empty().then_some(self.local)
    }
}

impl<'tcx> From<LocalIdx> for Place<'tcx> {
    fn from(local: LocalIdx) -> Self {
        Place { local, projection: &[] }
    }
}

impl LocalIdx {
    pub fn into_place<'tcx>(self) -> Place<'tcx> {
        self.into()
    }
}

#[derive(Debug)]
pub enum StatementKind<'tcx> {
    Assign(Place<'tcx>, Rvalue<'tcx>),
}

#[derive(Debug)]
pub enum TerminatorKind<'tcx> {
    Call {
        func: Operand<'tcx>,
        args: List<Operand<'tcx>>,
    },
}

#[derive(Debug)]
pub enum Rvalue<'tcx> {
    Use(Operand<'tcx>),
    Repeat(Operand<'tcx>, Const<'tcx>),
    Ref(RegionKind, mir::BorrowKind, Place<'tcx>),
    AddressOf(mir::Mutability, Place<'tcx>),
    Len(Place<'tcx>),
    Cast(mir::CastKind, Operand<'tcx>, Ty<'tcx>),
    BinaryOp(mir::BinOp, Box<[Operand<'tcx>; 2]>),
    NullaryOp(mir::NullOp<'tcx>, Ty<'tcx>),
    UnaryOp(mir::UnOp, Operand<'tcx>),
    Discriminant(Place<'tcx>),
    Aggregate(AggKind<'tcx>, IndexVec<FieldIdx, Operand<'tcx>>),
    ShallowInitBox(Operand<'tcx>, Ty<'tcx>),
    CopyForDeref(Place<'tcx>),
}

#[derive(Debug)]
pub enum Operand<'tcx> {
    Copy(Place<'tcx>),
    Move(Place<'tcx>),
    Constant(ConstOperand<'tcx>),
}

#[derive(Debug, Clone, Copy)]
pub enum RegionKind {
    ReStatic,
    ReErased,
}

#[derive(Debug)]
pub struct List<T, M = ListMatchMode> {
    pub data: Box<[T]>,
    pub mode: M,
}

pub struct Ordered;

pub type OrderedList<T> = List<T, Ordered>;

#[derive(Debug)]
pub enum ListMatchMode {
    Ordered,
    Unordered,
}

#[derive(Debug)]
pub enum ConstOperand<'tcx> {
    Ty(Ty<'tcx>, Const<'tcx>),
    Val(ConstValue, Ty<'tcx>),
}

#[derive(Debug, Clone, Copy)]
pub enum ConstValue {
    Scalar(mir::interpret::Scalar),
    ZeroSized,
    // Slice {
    //     data: mir::interpret::ConstAllocation<'tcx>,
    //     meta: u64,
    // },
    // Indirect { alloc_id: AllocId, offset: Size },
}

#[derive(Debug, Clone, Copy)]
#[rustc_pass_by_value]
pub struct Const<'tcx>(Interned<'tcx, ConstKind<'tcx>>);

impl<'tcx> Const<'tcx> {
    pub fn kind(self) -> &'tcx ConstKind<'tcx> {
        self.0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConstKind<'tcx> {
    ConstVar(ConstVarIdx),
    Value(Ty<'tcx>, ty::ScalarInt),
}

#[derive(Debug)]
pub enum AggKind<'tcx> {
    Array(Ty<'tcx>),
    Tuple,
    Adt(
        ItemPath<'tcx>,
        VariantIdx,
        &'tcx [GenericArgKind<'tcx>],
        Option<FieldIdx>,
    ),
    RawPtr(Ty<'tcx>, mir::Mutability),
}

#[derive(Debug)]
pub enum Field {
    Named(Symbol),
    Relative(FieldIdx),
}

#[derive(Debug, Clone, Copy)]
pub enum GenericArgKind<'tcx> {
    Lifetime(RegionKind),
    Ty(Ty<'tcx>),
    Const(Const<'tcx>),
}

#[derive(Debug, Clone, Copy)]
pub struct ItemPath<'tcx>(pub &'tcx [Symbol]);

#[derive(Debug, Clone, Copy)]
pub enum Path<'tcx> {
    Item(ItemPath<'tcx>),
    TypeRelative(Ty<'tcx>, Symbol),
    LangItem(Symbol),
}

#[derive(Debug, Clone, Copy)]
#[rustc_pass_by_value]
pub struct Ty<'tcx>(Interned<'tcx, TyKind<'tcx>>);

impl<'tcx> Ty<'tcx> {
    pub fn kind(self) -> &'tcx TyKind<'tcx> {
        self.0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TyKind<'tcx> {
    TyVar(TyVarIdx),
    Array(Ty<'tcx>, Const<'tcx>),
    Slice(Ty<'tcx>),
    Tuple(&'tcx [Ty<'tcx>]),
    Ref(RegionKind, Ty<'tcx>, mir::Mutability),
    RawPtr(Ty<'tcx>, mir::Mutability),
    Adt(ItemPath<'tcx>, &'tcx [GenericArgKind<'tcx>]),
    Uint(ty::UintTy),
    Int(ty::IntTy),
    Float(ty::FloatTy),
    FnDef(Path<'tcx>),
    Alias(ty::AliasTyKind, Path<'tcx>),
}

impl From<ConstVarIdx> for PatternIdx {
    fn from(konst: ConstVarIdx) -> Self {
        Self::from_u32(konst.as_u32())
    }
}

impl From<TyVarIdx> for PatternIdx {
    fn from(ty: TyVarIdx) -> Self {
        Self::from_u32(ty.as_u32())
    }
}

impl<'tcx> Pattern<'tcx> {
    pub fn new(kind: PatternKind<'tcx>) -> Self {
        Self::with_children_set(kind, HybridBitSet::new_empty(0))
    }
    pub fn with_children(kind: PatternKind<'tcx>, child_patterns: &mut [PatternIdx]) -> Self {
        child_patterns.sort_unstable();
        let mut children = HybridBitSet::new_empty(child_patterns.last().copied().map_or(0, PatternIdx::as_usize));
        for &mut pat in child_patterns {
            children.insert(pat);
        }
        Self::with_children_set(kind, children)
    }
    pub fn with_children_set(kind: PatternKind<'tcx>, children: HybridBitSet<PatternIdx>) -> Self {
        Self {
            kind,
            children,
            matches: Lock::new(GrowableBitSet::new_empty()),
        }
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn try_for_matched_patterns<T: Try<Output = ()>>(
        &self,
        pat: PatternIdx,
        mut f: impl FnMut(&MatchKind<'tcx>) -> T,
    ) -> T {
        let all_matches = self.matches.lock();
        let matches = self.patterns[pat].matches.lock();
        matches.iter().try_for_each(|mat| f(&all_matches[mat].kind))
    }
    pub fn try_for_matched_types<T: Try<Output = ()>>(
        &self,
        ty_var: TyVarIdx,
        mut f: impl FnMut(ty::Ty<'tcx>) -> T,
    ) -> T {
        self.try_for_matched_patterns(PatternIdx::from(ty_var), |mat| f(mat.expect_ty_var()))
    }
    pub fn new_ty_var(&mut self) -> TyVarIdx {
        TyVarIdx::from_u32(self.patterns.push(Pattern::new(PatternKind::TyVar)).as_u32())
    }
    pub fn new_const_var(&mut self) -> ConstVarIdx {
        ConstVarIdx::from_u32(self.patterns.push(Pattern::new(PatternKind::ConstVar)).as_u32())
    }
    pub fn mk_var_ty(&mut self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        let ty_var = self.new_ty_var();
        self.mk_ty(tcx, TyKind::TyVar(ty_var))
    }
    pub fn mk_ty(&self, tcx: TyCtxt<'tcx>, kind: TyKind<'tcx>) -> Ty<'tcx> {
        Ty(Interned::new_unchecked(tcx.arena.dropless.alloc(kind)))
    }
    pub fn mk_local(&mut self, ty: Ty<'tcx>) -> LocalIdx {
        self.locals.push(Local::new(ty, self.local_count))
    }
    pub fn mk_place(&self, tcx: TyCtxt<'tcx>, local: LocalIdx, projections: &[PlaceElem<'tcx>]) -> Place<'tcx> {
        if projections.is_empty() {
            return local.into();
        }
        Place {
            local,
            projection: tcx.arena.dropless.alloc_slice(projections),
        }
    }
    pub fn mk_init(&mut self, local: LocalIdx) -> PatternIdx {
        let pat = self.patterns.push(Pattern::new(PatternKind::Init(local)));
        self.update_deps(pat);
        pat
    }
    pub fn mk_assign(&mut self, place: Place<'tcx>, rvalue: Rvalue<'tcx>) -> PatternIdx {
        self.mk_statement(StatementKind::Assign(place, rvalue))
    }
    pub fn mk_statement(&mut self, statement: StatementKind<'tcx>) -> PatternIdx {
        let children = self.collect_deps(&statement);
        let pat = self
            .patterns
            .push(Pattern::with_children_set(PatternKind::Statement(statement), children));
        self.update_deps(pat);
        pat
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn ready_patterns(&self) -> impl Iterator<Item = (PatternIdx, &Pattern<'tcx>)> + '_ {
        self.patterns
            .iter_enumerated()
            .filter(|(_pat, pattern)| self.children_ready(pattern))
    }
    fn children_ready(&self, pattern: &Pattern<'tcx>) -> bool {
        use std::ops::Not;
        pattern
            .children
            .iter()
            .all(|child| self.patterns[child].matches.lock().is_empty().not())
    }
    fn add_ty_match(&self, pat: TyVarIdx, ty: ty::Ty<'tcx>) -> MatchIdx {
        self.add_match(pat.into(), MatchKind::TyVar(ty))
    }
    pub fn add_match(&self, pat: PatternIdx, kind: MatchKind<'tcx>) -> MatchIdx {
        let mat = self.matches.lock().push(Match::new(pat, kind));
        self.patterns[pat].matches.lock().insert(mat);
        mat
    }
    fn collect_deps<P: visitor::PatternVisitable<'tcx>>(&self, pattern: &P) -> HybridBitSet<PatternIdx> {
        let mut collect_deps = CollectDeps::new(self);
        pattern.visit_with(&mut collect_deps);
        collect_deps.deps
    }

    fn update_deps(&mut self, pat: PatternIdx) {
        UpdateDeps {
            locals: &mut self.locals,
            pat,
        }
        .visit_pattern(&self.patterns[pat]);
    }
}

struct CollectDeps<'a, 'tcx> {
    locals: &'a IndexVec<LocalIdx, Local<'tcx>>,
    deps: HybridBitSet<PatternIdx>,
}

impl<'a, 'tcx> CollectDeps<'a, 'tcx> {
    fn new(patterns: &'a Patterns<'tcx>) -> Self {
        Self {
            locals: &patterns.locals,
            deps: HybridBitSet::new_empty(patterns.patterns.len()),
        }
    }
}

impl<'tcx> PatternVisitor<'tcx> for CollectDeps<'_, 'tcx> {
    fn visit_local(&mut self, local: LocalIdx) {
        if let Some(pat) = self.locals[local].latest_pat {
            self.deps.insert(pat);
        }
    }
}

struct UpdateDeps<'a, 'tcx> {
    locals: &'a mut IndexVec<LocalIdx, Local<'tcx>>,
    pat: PatternIdx,
}

impl<'tcx> PatternVisitor<'tcx> for UpdateDeps<'_, 'tcx> {
    fn visit_local(&mut self, local: LocalIdx) {
        self.locals[local].latest_pat = Some(self.pat);
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn match_local(&self, tcx: TyCtxt<'tcx>, body: &mir::Body<'tcx>, pat: LocalIdx, local: mir::Local) -> bool {
        self.match_ty(tcx, self.locals[pat].ty, body.local_decls[local].ty)
    }
    pub fn match_place_ref(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &mir::Body<'tcx>,
        pat: Place<'tcx>,
        place: mir::PlaceRef<'tcx>,
    ) -> bool {
        use mir::ProjectionElem::*;
        self.match_local(tcx, body, pat.local, place.local)
            && pat.projection.len() == place.projection.len()
            && std::iter::zip(pat.projection, place.projection).all(|(&proj_pat, &proj)| match (proj_pat, proj) {
                (Deref, Deref) => true,
                (Field(idx_pat, ty_pat), Field(idx, ty)) => idx_pat == idx && self.match_ty(tcx, ty_pat, ty),
                (Index(local_pat), Index(local)) => self.match_local(tcx, body, local_pat, local),
                (
                    ConstantIndex {
                        offset: lhs0,
                        min_length: lhs1,
                        from_end: from_end_pat,
                    },
                    ConstantIndex {
                        offset: rhs0,
                        min_length: rhs1,
                        from_end,
                    },
                )
                | (
                    Subslice {
                        from: lhs0,
                        to: lhs1,
                        from_end: from_end_pat,
                    },
                    Subslice {
                        from: rhs0,
                        to: rhs1,
                        from_end,
                    },
                ) => (lhs0, lhs1, from_end_pat) == (rhs0, rhs1, from_end),
                (Downcast(sym_pat, idx_pat), Downcast(sym, idx)) => (sym_pat, idx_pat) == (sym, idx),
                (OpaqueCast(ty_pat), OpaqueCast(ty)) | (Subtype(ty_pat), Subtype(ty)) => self.match_ty(tcx, ty_pat, ty),
                _ => false,
            })
    }

    pub fn match_ty(&self, _tcx: TyCtxt<'tcx>, ty_pat: Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        match (*ty_pat.kind(), *ty.kind()) {
            (TyKind::TyVar(ty_var), _) => {
                self.add_ty_match(ty_var, ty);
                true
            },
            (TyKind::Slice(ty_pat), ty::Slice(ty)) => self.match_ty(_tcx, ty_pat, ty),
            (TyKind::Ref(region_pat, pat_ty, pat_mutblty), ty::Ref(region, ty, mutblty)) => {
                self.match_region(region_pat, region) && pat_mutblty == mutblty && self.match_ty(_tcx, pat_ty, ty)
            },
            (TyKind::RawPtr(ty_pat, mutability_pat), ty::RawPtr(ty, mutblty)) => {
                mutability_pat == mutblty && self.match_ty(_tcx, ty_pat, ty)
            },
            (TyKind::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
            (TyKind::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
            (TyKind::Float(ty_pat), ty::Float(ty)) => ty_pat == ty,
            _ => false,
        }
    }
    pub fn match_const(&self, tcx: TyCtxt<'tcx>, konst_pat: Const<'tcx>, konst: ty::Const<'tcx>) -> bool {
        match (konst_pat.kind(), konst.kind()) {
            (ConstKind::ConstVar(_), _) => true,
            (&ConstKind::Value(ty_pat, value_pat), ty::Value(ty, ty::ValTree::Leaf(value))) => {
                self.match_ty(tcx, ty_pat, ty) && value_pat == value
            },
            _ => false,
        }
    }
    pub fn match_const_operand(
        &self,
        tcx: TyCtxt<'tcx>,
        konst_pat: ConstOperand<'tcx>,
        konst: mir::Const<'tcx>,
    ) -> bool {
        match (konst_pat, konst) {
            (ConstOperand::Ty(ty_pat, konst_pat), mir::Const::Ty(ty, konst)) => {
                self.match_ty(tcx, ty_pat, ty) && self.match_const(tcx, konst_pat, konst)
            },
            (ConstOperand::Val(value_pat, ty_pat), mir::Const::Val(value, ty)) => {
                self.match_const_value(value_pat, value) && self.match_ty(tcx, ty_pat, ty)
            },
            _ => false,
        }
    }
    pub fn match_const_value(&self, konst_pat: ConstValue, konst: mir::ConstValue<'tcx>) -> bool {
        match (konst_pat, konst) {
            (ConstValue::Scalar(scalar_pat), mir::ConstValue::Scalar(scalar)) => scalar_pat == scalar,
            (ConstValue::ZeroSized, mir::ConstValue::ZeroSized) => true,
            _ => false,
        }
    }
    pub fn match_region(&self, region_pat: RegionKind, region: ty::Region<'tcx>) -> bool {
        matches!(
            (region_pat, region.kind()),
            (RegionKind::ReStatic, ty::RegionKind::ReStatic) | (RegionKind::ReErased, _)
        )
    }
    pub fn match_agg_kind(
        &self,
        tcx: TyCtxt<'tcx>,
        agg_kind_pat: &AggKind<'tcx>,
        agg_kind: &mir::AggregateKind<'tcx>,
    ) -> bool {
        match (agg_kind_pat, agg_kind) {
            (&AggKind::Array(ty_pat), &mir::AggregateKind::Array(ty)) => self.match_ty(tcx, ty_pat, ty),
            (AggKind::Tuple, mir::AggregateKind::Tuple) => true,
            (AggKind::Adt(..), mir::AggregateKind::Adt(_, _, _, _, _)) => todo!(),
            (&AggKind::RawPtr(ty_pat, mutability_pat), &mir::AggregateKind::RawPtr(ty, mutability)) => {
                self.match_ty(tcx, ty_pat, ty) && mutability_pat == mutability
            },
            _ => false,
        }
    }
}
