use std::iter::zip;
use std::ops::Try;

use rustc_data_structures::sync::Lock;
use rustc_hir::def::CtorKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::definitions::DefPathData;
use rustc_hir::LangItem;
use rustc_index::bit_set::{GrowableBitSet, HybridBitSet};
use rustc_index::IndexVec;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{mir, ty};
use rustc_span::symbol::kw;
use rustc_span::{Span, Symbol};
use rustc_target::abi::FieldIdx;
use visitor::PatternVisitor;

pub use arena::IntoArena;
pub use Operand::{Copy, Move};

mod arena;
mod pretty;
pub mod visitor;

pub struct Pattern<'tcx> {
    pub kind: PatternKind<'tcx>,
    dependencies: HybridBitSet<PatternIdx>,
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

#[derive(Debug, Clone, Copy)]
pub enum PlaceElem<'tcx> {
    Deref,
    Field(Field),
    Index(LocalIdx),
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast(Symbol),
    OpaqueCast(Ty<'tcx>),
    Subtype(Ty<'tcx>),
}

#[derive(Clone, Copy)]
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

pub enum StatementKind<'tcx> {
    Assign(Place<'tcx>, Rvalue<'tcx>),
}

pub enum TerminatorKind<'tcx> {
    Call {
        func: Operand<'tcx>,
        args: List<Operand<'tcx>>,
        destination: Place<'tcx>,
    },
    Drop {
        place: Place<'tcx>,
    },
}

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

pub enum Operand<'tcx> {
    Copy(Place<'tcx>),
    Move(Place<'tcx>),
    Constant(ConstOperand<'tcx>),
}

#[derive(Clone, Copy)]
pub enum RegionKind {
    ReStatic,
    ReErased,
}

pub struct List<T, M = ListMatchMode> {
    pub data: Box<[T]>,
    pub mode: M,
}

impl<T> List<T> {
    pub fn ordered(iter: impl IntoIterator<Item = T>) -> Self {
        Self {
            data: iter.into_iter().collect(),
            mode: ListMatchMode::Ordered,
        }
    }
    pub fn unordered(iter: impl IntoIterator<Item = T>) -> Self {
        Self {
            data: iter.into_iter().collect(),
            mode: ListMatchMode::Unordered,
        }
    }
}

pub struct Ordered;

pub type OrderedList<T> = List<T, Ordered>;

#[derive(Debug)]
pub enum ListMatchMode {
    Ordered,
    Unordered,
}

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
pub struct Const<'tcx>(&'tcx ConstKind<'tcx>);

impl<'tcx> Const<'tcx> {
    pub fn kind(self) -> &'tcx ConstKind<'tcx> {
        self.0
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
    Adt(ItemPath<'tcx>, Option<Symbol>, GenericArgsRef<'tcx>, Option<Field>),
    RawPtr(Ty<'tcx>, mir::Mutability),
}

#[derive(Clone, Copy)]
pub enum Field {
    Named(Symbol),
    Relative(FieldIdx),
}

impl From<&str> for Field {
    fn from(name: &str) -> Self {
        Symbol::intern(name).into()
    }
}

impl From<Symbol> for Field {
    fn from(name: Symbol) -> Self {
        Field::Named(name)
    }
}

impl From<u32> for Field {
    fn from(field: u32) -> Self {
        FieldIdx::from_u32(field).into()
    }
}

impl From<FieldIdx> for Field {
    fn from(field: FieldIdx) -> Self {
        Field::Relative(field)
    }
}

#[derive(Clone, Copy)]
pub struct GenericArgsRef<'tcx>(pub &'tcx [GenericArgKind<'tcx>]);

impl<'tcx> std::ops::Deref for GenericArgsRef<'tcx> {
    type Target = [GenericArgKind<'tcx>];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Clone, Copy)]
pub enum GenericArgKind<'tcx> {
    Lifetime(RegionKind),
    Type(Ty<'tcx>),
    Const(Const<'tcx>),
}

impl From<RegionKind> for GenericArgKind<'_> {
    fn from(region: RegionKind) -> Self {
        GenericArgKind::Lifetime(region)
    }
}
impl<'tcx> From<Ty<'tcx>> for GenericArgKind<'tcx> {
    fn from(ty: Ty<'tcx>) -> Self {
        GenericArgKind::Type(ty)
    }
}
impl<'tcx> From<Const<'tcx>> for GenericArgKind<'tcx> {
    fn from(konst: Const<'tcx>) -> Self {
        GenericArgKind::Const(konst)
    }
}

#[derive(Clone, Copy)]
pub struct ItemPath<'tcx>(pub &'tcx [Symbol]);

#[derive(Clone, Copy)]
pub enum Path<'tcx> {
    Item(ItemPath<'tcx>),
    TypeRelative(Ty<'tcx>, Symbol),
    LangItem(LangItem),
}

impl<'tcx, T: IntoArena<'tcx, [Symbol]>> From<T> for Path<'tcx> {
    fn from(item: T) -> Self {
        Path::Item(ItemPath(item.into_arena()))
    }
}

impl<'tcx> From<ItemPath<'tcx>> for Path<'tcx> {
    fn from(item: ItemPath<'tcx>) -> Self {
        Path::Item(item)
    }
}

impl<'tcx> From<(Ty<'tcx>, Symbol)> for Path<'tcx> {
    fn from((ty, path): (Ty<'tcx>, Symbol)) -> Self {
        Path::TypeRelative(ty, path)
    }
}

impl<'tcx> From<(Ty<'tcx>, &str)> for Path<'tcx> {
    fn from((ty, path): (Ty<'tcx>, &str)) -> Self {
        (ty, Symbol::intern(path)).into()
    }
}

impl<'tcx, T: IntoArena<'tcx, TyKind<'tcx>>> From<(T, &str)> for Path<'tcx> {
    fn from((ty, path): (T, &str)) -> Self {
        Path::TypeRelative(Ty(ty.into_arena()), Symbol::intern(path))
    }
}

impl<'tcx> From<LangItem> for Path<'tcx> {
    fn from(lang_item: LangItem) -> Self {
        Path::LangItem(lang_item)
    }
}

// FIXME: Use interning for the types
#[derive(Clone, Copy)]
#[rustc_pass_by_value]
pub struct Ty<'tcx>(&'tcx TyKind<'tcx>);

impl<'tcx> Ty<'tcx> {
    pub fn kind(self) -> &'tcx TyKind<'tcx> {
        self.0
    }
}

#[derive(Clone, Copy)]
pub enum TyKind<'tcx> {
    TyVar(TyVarIdx),
    Array(Ty<'tcx>, Const<'tcx>),
    Slice(Ty<'tcx>),
    Tuple(&'tcx [Ty<'tcx>]),
    Ref(RegionKind, Ty<'tcx>, mir::Mutability),
    RawPtr(Ty<'tcx>, mir::Mutability),
    Adt(ItemPath<'tcx>, GenericArgsRef<'tcx>),
    Uint(ty::UintTy),
    Int(ty::IntTy),
    Float(ty::FloatTy),
    FnDef(Path<'tcx>, GenericArgsRef<'tcx>),
    Alias(ty::AliasTyKind, Path<'tcx>, GenericArgsRef<'tcx>),
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
        Self::with_deps_set(kind, HybridBitSet::new_empty(0))
    }
    pub fn with_deps(kind: PatternKind<'tcx>, child_patterns: &mut [PatternIdx]) -> Self {
        child_patterns.sort_unstable();
        let mut children = HybridBitSet::new_empty(child_patterns.last().copied().map_or(0, PatternIdx::as_usize));
        for &mut pat in child_patterns {
            children.insert(pat);
        }
        Self::with_deps_set(kind, children)
    }
    pub fn with_deps_set(kind: PatternKind<'tcx>, dependencies: HybridBitSet<PatternIdx>) -> Self {
        Self {
            kind,
            dependencies,
            matches: Lock::new(GrowableBitSet::new_empty()),
        }
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn first_matched_span(&self, body: &mir::Body<'tcx>, pat: PatternIdx) -> Option<Span> {
        self.try_for_matched_patterns(pat, |mat| mat.span(body).map_or(Ok(()), Err))
            .err()
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
    pub fn mk_item_path(&self, tcx: TyCtxt<'tcx>, path: &[&str]) -> ItemPath<'tcx> {
        ItemPath((tcx, path).into_arena())
    }
    pub fn mk_adt_ty(
        &self,
        tcx: TyCtxt<'tcx>,
        path: impl IntoArena<'tcx, [Symbol]>,
        generics: impl IntoArena<'tcx, [GenericArgKind<'tcx>]>,
    ) -> Ty<'tcx> {
        self.mk_ty(
            tcx,
            TyKind::Adt(ItemPath(path.into_arena()), GenericArgsRef(generics.into_arena())),
        )
    }
    pub fn mk_ty(&self, tcx: TyCtxt<'tcx>, kind: TyKind<'tcx>) -> Ty<'tcx> {
        Ty((tcx, kind).into_arena())
    }
    pub fn mk_local(&mut self, ty: Ty<'tcx>) -> LocalIdx {
        self.locals.push(Local::new(ty, self.local_count))
    }
    pub fn mk_place(&self, local: LocalIdx, projection: impl IntoArena<'tcx, [PlaceElem<'tcx>]>) -> Place<'tcx> {
        Place {
            local,
            projection: projection.into_arena(),
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
            .push(Pattern::with_deps_set(PatternKind::Statement(statement), children));
        self.update_deps(pat);
        pat
    }
    pub fn mk_fn(
        &self,
        tcx: TyCtxt<'tcx>,
        func: impl Into<Path<'tcx>>,
        generics: impl IntoArena<'tcx, [GenericArgKind<'tcx>]>,
    ) -> Ty<'tcx> {
        self.mk_ty(tcx, TyKind::FnDef(func.into(), GenericArgsRef(generics.into_arena())))
    }
    pub fn mk_fn_call(
        &mut self,
        tcx: TyCtxt<'tcx>,
        func: impl Into<Path<'tcx>>,
        generics: impl IntoArena<'tcx, [GenericArgKind<'tcx>]>,
        args: List<Operand<'tcx>>,
        destination: Place<'tcx>,
    ) -> PatternIdx {
        let func_ty = self.mk_fn(tcx, func, generics);
        self.mk_terminator(TerminatorKind::Call {
            func: Operand::Constant(ConstOperand::Val(ConstValue::ZeroSized, func_ty)),
            args,
            destination,
        })
    }
    pub fn mk_drop(&mut self, place: Place<'tcx>) -> PatternIdx {
        self.mk_terminator(TerminatorKind::Drop { place })
    }
    pub fn mk_terminator(&mut self, terminator: TerminatorKind<'tcx>) -> PatternIdx {
        let children = self.collect_deps(&terminator);
        let pat = self
            .patterns
            .push(Pattern::with_deps_set(PatternKind::Terminator(terminator), children));
        self.update_deps(pat);
        pat
    }
    pub fn add_dependency(&mut self, pat: PatternIdx, dep: PatternIdx) {
        self.patterns[pat].dependencies.insert(dep);
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn ready_patterns(&self) -> impl Iterator<Item = (PatternIdx, &Pattern<'tcx>)> + '_ {
        self.patterns.iter_enumerated().filter(|(pat, pattern)| {
            let ready = self.children_ready(pattern);
            if !ready {
                debug!("{pat:?} not ready: {:?}", pattern.kind);
            }
            ready
        })
    }
    fn children_ready(&self, pattern: &Pattern<'tcx>) -> bool {
        use std::ops::Not;
        pattern
            .dependencies
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
        use self::Field::{Named, Relative};
        use mir::tcx::PlaceTy;
        use mir::ProjectionElem::*;
        let place_proj_and_ty = place
            .projection
            .iter()
            .scan(PlaceTy::from_ty(body.local_decls[place.local].ty), |place_ty, &proj| {
                Some((proj, std::mem::replace(place_ty, place_ty.projection_ty(tcx, proj))))
            });
        self.match_local(tcx, body, pat.local, place.local)
            && pat.projection.len() == place.projection.len()
            && std::iter::zip(pat.projection, place_proj_and_ty).all(|(&proj_pat, (proj, place_ty))| {
                match (place_ty.ty.kind(), proj_pat, proj) {
                    (_, PlaceElem::Deref, Deref) => true,
                    (ty::Adt(adt, _), PlaceElem::Field(field), Field(idx, _)) => {
                        let variant = match place_ty.variant_index {
                            None => adt.non_enum_variant(),
                            Some(idx) => adt.variant(idx),
                        };
                        match (variant.ctor, field) {
                            (None, Named(name)) => variant.ctor.is_none() && variant.fields[idx].name == name,
                            (Some((CtorKind::Fn, _)), Relative(idx_pat)) => idx_pat == idx,
                            _ => false,
                        }
                    },
                    (_, PlaceElem::Index(local_pat), Index(local)) => self.match_local(tcx, body, local_pat, local),
                    (
                        _,
                        PlaceElem::ConstantIndex {
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
                        _,
                        PlaceElem::Subslice {
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
                    (ty::Adt(adt, _), PlaceElem::Downcast(sym), Downcast(_, idx)) => {
                        adt.is_enum() && adt.variant(idx).name == sym
                    },
                    (_, PlaceElem::OpaqueCast(ty_pat), OpaqueCast(ty))
                    | (_, PlaceElem::Subtype(ty_pat), Subtype(ty)) => self.match_ty(tcx, ty_pat, ty),
                    _ => false,
                }
            })
    }

    pub fn match_ty(&self, tcx: TyCtxt<'tcx>, ty_pat: Ty<'tcx>, ty: ty::Ty<'tcx>) -> bool {
        let matched = match (*ty_pat.kind(), *ty.kind()) {
            (TyKind::TyVar(ty_var), _) => {
                self.add_ty_match(ty_var, ty);
                true
            },
            (TyKind::Array(ty_pat, konst_pat), ty::Array(ty, konst)) => {
                self.match_ty(tcx, ty_pat, ty) && self.match_const(tcx, konst_pat, konst)
            },
            (TyKind::Slice(ty_pat), ty::Slice(ty)) => self.match_ty(tcx, ty_pat, ty),
            (TyKind::Tuple(tys_pat), ty::Tuple(tys)) => {
                tys_pat.len() == tys.len() && zip(tys_pat, tys).all(|(&ty_pat, ty)| self.match_ty(tcx, ty_pat, ty))
            },
            (TyKind::Ref(region_pat, pat_ty, pat_mutblty), ty::Ref(region, ty, mutblty)) => {
                self.match_region(region_pat, region) && pat_mutblty == mutblty && self.match_ty(tcx, pat_ty, ty)
            },
            (TyKind::RawPtr(ty_pat, mutability_pat), ty::RawPtr(ty, mutblty)) => {
                mutability_pat == mutblty && self.match_ty(tcx, ty_pat, ty)
            },
            (TyKind::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
            (TyKind::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
            (TyKind::Float(ty_pat), ty::Float(ty)) => ty_pat == ty,
            (TyKind::Adt(path, args_pat), ty::Adt(adt, args)) => {
                self.match_item_path(tcx, path, adt.did()) && self.match_generic_args(tcx, args_pat, args)
            },
            (TyKind::FnDef(path, args_pat), ty::FnDef(def_id, args)) => {
                self.match_path(tcx, path, def_id) && self.match_generic_args(tcx, args_pat, args)
            },
            (TyKind::Alias(alias_kind_pat, path, args), ty::Alias(alias_kind, alias)) => {
                alias_kind_pat == alias_kind
                    && self.match_path(tcx, path, alias.def_id)
                    && self.match_generic_args(tcx, args, alias.args)
            },
            _ => false,
        };
        debug!(?ty_pat, ?ty, matched, "match_ty");
        matched
    }

    pub fn match_path(&self, tcx: TyCtxt<'tcx>, path: Path<'tcx>, def_id: DefId) -> bool {
        let matched = match path {
            Path::Item(path) => self.match_item_path(tcx, path, def_id),
            Path::TypeRelative(ty, name) => {
                tcx.item_name(def_id) == name
                    && tcx
                        .opt_parent(def_id)
                        .is_some_and(|did| self.match_ty(tcx, ty, tcx.type_of(did).instantiate_identity()))
            },
            Path::LangItem(lang_item) => tcx.is_lang_item(def_id, lang_item),
        };
        debug!(?path, ?def_id, matched, "match_path");
        matched
    }

    pub fn match_item_path(&self, tcx: TyCtxt<'tcx>, path: ItemPath<'tcx>, def_id: DefId) -> bool {
        let &[krate, ref in_crate @ ..] = path.0 else {
            return false;
        };
        let def_path = tcx.def_path(def_id);
        let matched = match def_path.krate {
            LOCAL_CRATE => krate == kw::Crate,
            _ => tcx.crate_name(def_path.krate) == krate,
        };
        let mut pat_iter = in_crate.iter();
        use DefPathData::{Impl, TypeNs, ValueNs};
        let mut iter = def_path
            .data
            .iter()
            .filter(|data| matches!(data.data, Impl | TypeNs(_) | ValueNs(_)));
        let matched = matched
            && std::iter::zip(pat_iter.by_ref(), iter.by_ref())
                .all(|(&path, data)| data.data.get_opt_name().is_some_and(|name| name == path));
        let matched = matched && pat_iter.next().is_none() && iter.next().is_none();
        debug!(?path, ?def_id, matched, "match_item_path");
        matched
    }

    pub fn match_generic_args(
        &self,
        tcx: TyCtxt<'tcx>,
        args_pat: GenericArgsRef<'tcx>,
        args: ty::GenericArgsRef<'tcx>,
    ) -> bool {
        args_pat.len() == args.len()
            && zip(&*args_pat, args).all(|(&arg_pat, arg)| self.match_generic_arg(tcx, arg_pat, arg))
    }

    pub fn match_generic_arg(
        &self,
        tcx: TyCtxt<'tcx>,
        arg_pat: GenericArgKind<'tcx>,
        arg: ty::GenericArg<'tcx>,
    ) -> bool {
        match (arg_pat, arg.unpack()) {
            (GenericArgKind::Lifetime(region_pat), ty::GenericArgKind::Lifetime(region)) => {
                self.match_region(region_pat, region)
            },
            (GenericArgKind::Type(ty_pat), ty::GenericArgKind::Type(ty)) => self.match_ty(tcx, ty_pat, ty),
            (GenericArgKind::Const(konst_pat), ty::GenericArgKind::Const(konst)) => {
                self.match_const(tcx, konst_pat, konst)
            },
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
            (
                &AggKind::Adt(path, variant_name, args_pat, field),
                &mir::AggregateKind::Adt(def_id, variant_idx, args, _, field_idx),
            ) if self.match_item_path(tcx, path, def_id) => {
                let adt = tcx.adt_def(def_id);
                let variant = adt.variant(variant_idx);
                let variant_matched = match variant_name {
                    None => {
                        variant_idx.as_u32() == 0 && matches!(adt.adt_kind(), ty::AdtKind::Struct | ty::AdtKind::Union)
                    },
                    Some(name) => variant.name == name,
                };
                variant_matched
                    && self.match_generic_args(tcx, args_pat, args)
                    && match (field, field_idx) {
                        (None, None) => true,
                        (Some(Field::Named(name)), Some(field_idx)) => {
                            adt.is_union() && variant.fields[field_idx].name == name
                        },
                        _ => false,
                    }
            },
            (&AggKind::RawPtr(ty_pat, mutability_pat), &mir::AggregateKind::RawPtr(ty, mutability)) => {
                self.match_ty(tcx, ty_pat, ty) && mutability_pat == mutability
            },
            _ => false,
        }
    }
}
