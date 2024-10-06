use std::iter::zip;

use rustc_arena::DroplessArena;
use rustc_data_structures::fx::FxIndexMap;
use rustc_data_structures::packed::Pu128;
use rustc_hash::FxHashMap;
use rustc_hir::def::CtorKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::definitions::DefPathData;
use rustc_hir::LangItem;
use rustc_index::{IndexSlice, IndexVec};
use rustc_middle::{mir, ty};
use rustc_span::symbol::kw;
use rustc_span::Symbol;
use rustc_target::abi::FieldIdx;

mod matching;
mod pretty;
pub mod visitor;

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
    #[debug_format = "?bb{}"]
    pub struct BasicBlock {}
}

pub struct PrimitiveTypes<'tcx> {
    pub u8: Ty<'tcx>,
    pub u16: Ty<'tcx>,
    pub u32: Ty<'tcx>,
    pub u64: Ty<'tcx>,
    pub u128: Ty<'tcx>,
    pub usize: Ty<'tcx>,
    pub i8: Ty<'tcx>,
    pub i16: Ty<'tcx>,
    pub i32: Ty<'tcx>,
    pub i64: Ty<'tcx>,
    pub i128: Ty<'tcx>,
    pub isize: Ty<'tcx>,
    pub bool: Ty<'tcx>,
    pub str: Ty<'tcx>,
}

impl<'tcx> PrimitiveTypes<'tcx> {
    fn new(arena: &'tcx DroplessArena) -> Self {
        Self {
            u8: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U8))),
            u16: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U16))),
            u32: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U32))),
            u64: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U64))),
            u128: Ty(arena.alloc(TyKind::Uint(ty::UintTy::U128))),
            usize: Ty(arena.alloc(TyKind::Uint(ty::UintTy::Usize))),
            i8: Ty(arena.alloc(TyKind::Int(ty::IntTy::I8))),
            i16: Ty(arena.alloc(TyKind::Int(ty::IntTy::I16))),
            i32: Ty(arena.alloc(TyKind::Int(ty::IntTy::I32))),
            i64: Ty(arena.alloc(TyKind::Int(ty::IntTy::I64))),
            i128: Ty(arena.alloc(TyKind::Int(ty::IntTy::I128))),
            isize: Ty(arena.alloc(TyKind::Int(ty::IntTy::Isize))),
            bool: Ty(arena.alloc(TyKind::Bool)),
            str: Ty(arena.alloc(TyKind::Str)),
        }
    }
}

pub struct Patterns<'tcx> {
    arena: &'tcx DroplessArena,
    ty_vars: IndexVec<TyVarIdx, ()>,
    const_vars: IndexVec<ConstVarIdx, Ty<'tcx>>,
    locals: IndexVec<LocalIdx, Ty<'tcx>>,
    self_idx: Option<LocalIdx>,
    basic_blocks: IndexVec<BasicBlock, BasicBlockData<'tcx>>,
    pub primitive_types: PrimitiveTypes<'tcx>,
}

#[derive(Default)]
pub struct BasicBlockData<'tcx> {
    pub statements: Vec<StatementKind<'tcx>>,
    pub terminator: Option<TerminatorKind<'tcx>>,
}

pub struct ConstVar<'tcx> {
    pub konst: ty::Const<'tcx>,
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
    pub fn new(local: LocalIdx, projection: &'tcx [PlaceElem<'tcx>]) -> Self {
        Self { local, projection }
    }
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
    Init(Place<'tcx>),
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum IntTy {
    NegInt(ty::IntTy),
    Int(ty::IntTy),
    Uint(ty::UintTy),
    Bool,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct IntValue {
    pub value: Pu128,
    pub ty: IntTy,
}

macro_rules! impl_uint {
    ($($ty:ident => $variant:ident),* $(,)?) => {$(
        impl From<$ty> for IntValue {
            fn from(value: $ty) -> Self {
                Self {
                    value: Pu128(value as u128),
                    ty: IntTy::Uint(ty::UintTy::$variant),
                }
            }
        }
    )* };
}

macro_rules! impl_int {
    ($($ty:ident => $variant:ident),* $(,)?) => {$(
        impl From<$ty> for IntValue {
            fn from(value: $ty) -> Self {
                let ty = if value < 0 { IntTy::NegInt } else { IntTy::Int };
                Self {
                    value: Pu128(value.unsigned_abs() as u128),
                    ty: ty(ty::IntTy::$variant),
                }
            }
        }
    )* };
}

impl_uint!(u8 => U8, u16 => U16, u32 => U32, u64 => U64, u128 => U128, usize => Usize);
impl_int!(i8 => I8, i16 => I16, i32 => I32, i64 => I64, i128 => I128, isize => Isize);

impl From<bool> for IntValue {
    fn from(value: bool) -> Self {
        Self {
            value: Pu128(value.into()),
            ty: IntTy::Bool,
        }
    }
}

#[derive(Default)]
pub struct SwitchTargets {
    pub targets: FxIndexMap<IntValue, BasicBlock>,
    pub otherwise: Option<BasicBlock>,
}

pub enum TerminatorKind<'tcx> {
    SwitchInt {
        operand: Operand<'tcx>,
        targets: SwitchTargets,
    },
    Goto(BasicBlock),
    Call {
        func: Operand<'tcx>,
        args: List<Operand<'tcx>>,
        destination: Option<Place<'tcx>>,
        target: BasicBlock,
    },
    Drop {
        place: Place<'tcx>,
        target: BasicBlock,
    },
    Return,
}

pub enum Rvalue<'tcx> {
    Use(Operand<'tcx>),
    Repeat(Operand<'tcx>, Const),
    Ref(RegionKind, mir::BorrowKind, Place<'tcx>),
    AddressOf(mir::Mutability, Place<'tcx>),
    Len(Place<'tcx>),
    Cast(mir::CastKind, Operand<'tcx>, Ty<'tcx>),
    BinaryOp(mir::BinOp, Box<[Operand<'tcx>; 2]>),
    NullaryOp(mir::NullOp<'tcx>, Ty<'tcx>),
    UnaryOp(mir::UnOp, Operand<'tcx>),
    Discriminant(Place<'tcx>),
    Aggregate(AggKind<'tcx>, Box<[Operand<'tcx>]>),
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
    ReAny,
    ReStatic,
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
    ConstVar(ConstVarIdx),
    ScalarInt(IntValue),
    ZeroSized(Ty<'tcx>),
}

#[derive(Debug, Clone, Copy)]
pub enum Const {
    ConstVar(ConstVarIdx),
    Value(IntValue),
}

#[derive(Debug)]
pub enum AggKind<'tcx> {
    Array,
    Tuple,
    Adt(ItemPath<'tcx>, GenericArgsRef<'tcx>, Option<Box<[Symbol]>>),
    RawPtr(Ty<'tcx>, mir::Mutability),
}

#[derive(Clone, Copy)]
pub enum Field {
    Named(Symbol),
    Unnamed(FieldIdx),
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
        Field::Unnamed(field)
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
    Const(Const),
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
impl<'tcx> From<Const> for GenericArgKind<'tcx> {
    fn from(konst: Const) -> Self {
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
    Array(Ty<'tcx>, Const),
    Slice(Ty<'tcx>),
    Tuple(&'tcx [Ty<'tcx>]),
    Ref(RegionKind, Ty<'tcx>, mir::Mutability),
    RawPtr(Ty<'tcx>, mir::Mutability),
    Adt(ItemPath<'tcx>, GenericArgsRef<'tcx>),
    Uint(ty::UintTy),
    Int(ty::IntTy),
    Float(ty::FloatTy),
    Bool,
    Str,
    FnDef(Path<'tcx>, GenericArgsRef<'tcx>),
    Alias(ty::AliasTyKind, Path<'tcx>, GenericArgsRef<'tcx>),
}

pub struct PatternsBuilder<'tcx> {
    patterns: Patterns<'tcx>,
    loop_stack: Vec<Loop>,
    current: BasicBlock,
}

struct Loop {
    enter: BasicBlock,
    exit: BasicBlock,
}

impl<'tcx> PatternsBuilder<'tcx> {
    pub fn new(arena: &'tcx DroplessArena) -> Self {
        let mut patterns = Patterns {
            arena,
            ty_vars: IndexVec::new(),
            const_vars: IndexVec::new(),
            locals: IndexVec::new(),
            self_idx: None,
            basic_blocks: IndexVec::new(),
            primitive_types: PrimitiveTypes::new(arena),
        };
        let current = patterns.basic_blocks.push(BasicBlockData::default());
        Self {
            patterns,
            loop_stack: Vec::new(),
            current,
        }
    }
    pub fn build(self) -> Patterns<'tcx> {
        self.patterns
    }

    pub fn new_ty_var(&mut self) -> TyVarIdx {
        self.patterns.ty_vars.push(())
    }
    pub fn new_const_var(&mut self, ty: Ty<'tcx>) -> ConstVarIdx {
        self.patterns.const_vars.push(ty)
    }
    pub fn mk_local(&mut self, ty: Ty<'tcx>) -> LocalIdx {
        self.patterns.locals.push(ty)
    }
    pub fn mk_self(&mut self, ty: Ty<'tcx>) -> LocalIdx {
        *self.patterns.self_idx.insert(self.patterns.locals.push(ty))
    }
    fn check_terminator(&mut self) {
        if self.patterns.basic_blocks[self.current].terminator.is_some() {
            self.current = self.patterns.basic_blocks.push(BasicBlockData::default());
        }
    }
    fn mk_statement(&mut self, kind: StatementKind<'tcx>) {
        self.check_terminator();
        self.patterns.basic_blocks[self.current].statements.push(kind);
    }
    pub fn mk_init(&mut self, place: impl Into<Place<'tcx>>) {
        self.mk_statement(StatementKind::Init(place.into()));
    }
    pub fn mk_assign(&mut self, place: impl Into<Place<'tcx>>, rvalue: Rvalue<'tcx>) {
        self.mk_statement(StatementKind::Assign(place.into(), rvalue));
    }
    pub fn mk_fn_call(&mut self, func: Operand<'tcx>, args: List<Operand<'tcx>>, destination: Option<Place<'tcx>>) {
        self.check_terminator();
        let target = self.patterns.basic_blocks.next_index();
        self.patterns.basic_blocks[self.current].terminator = Some(TerminatorKind::Call {
            func,
            args,
            destination,
            target,
        });
    }
    pub fn mk_drop(&mut self, place: impl Into<Place<'tcx>>) {
        self.check_terminator();
        let target = self.patterns.basic_blocks.next_index();
        let place = place.into();
        self.patterns.basic_blocks[self.current].terminator = Some(TerminatorKind::Drop { place, target });
    }
    pub fn mk_switch_int(&mut self, operand: Operand<'tcx>, f: impl FnOnce(SwitchIntBuilder<'_, 'tcx>)) {
        self.check_terminator();
        let current = self.current;
        self.patterns.basic_blocks[current].terminator = Some(TerminatorKind::SwitchInt {
            operand,
            targets: SwitchTargets::default(),
        });
        let next = self.patterns.basic_blocks.push(BasicBlockData::default());
        let mut built_targets = SwitchTargets::default();
        let builder = SwitchIntBuilder {
            builder: self,
            next,
            targets: &mut built_targets,
        };
        f(builder);
        if let Some(terminator) = &mut self.patterns.basic_blocks[current].terminator
            && let TerminatorKind::SwitchInt { targets, .. } = terminator
        {
            *targets = built_targets;
        }
        self.current = next;
    }
    fn goto(&mut self, block: BasicBlock) {
        let terminator = &mut self.patterns.basic_blocks[self.current].terminator;
        match terminator {
            None => *terminator = Some(TerminatorKind::Goto(block)),
            Some(TerminatorKind::Call { target, .. } | TerminatorKind::Drop { target, .. }) => *target = block,
            Some(TerminatorKind::Goto(_) | TerminatorKind::SwitchInt { .. } | TerminatorKind::Return) => {},
        }
    }
    pub fn mk_loop(&mut self, f: impl FnOnce(&mut PatternsBuilder<'tcx>)) {
        let enter = self.patterns.basic_blocks.push(BasicBlockData::default());
        self.goto(enter);
        let exit = self.patterns.basic_blocks.push(BasicBlockData::default());
        self.loop_stack.push(Loop { enter, exit });
        self.current = enter;
        f(self);
        self.loop_stack.pop();
        self.goto(enter);
        self.current = exit;
    }
    pub fn mk_break(&mut self) {
        let exit = self.loop_stack.last().expect("no loop to break from").exit;
        self.goto(exit);
    }
    pub fn mk_continue(&mut self) {
        let enter = self.loop_stack.last().expect("no loop to continue").enter;
        self.goto(enter);
    }
}

impl<'tcx> std::ops::Deref for PatternsBuilder<'tcx> {
    type Target = Patterns<'tcx>;

    fn deref(&self) -> &Self::Target {
        &self.patterns
    }
}

pub struct SwitchIntBuilder<'a, 'tcx> {
    builder: &'a mut PatternsBuilder<'tcx>,
    next: BasicBlock,
    targets: &'a mut SwitchTargets,
}

impl<'tcx> SwitchIntBuilder<'_, 'tcx> {
    pub fn mk_switch_target(&mut self, value: impl Into<IntValue>, f: impl FnOnce(&mut PatternsBuilder<'tcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.patterns.basic_blocks.push(BasicBlockData::default());
        targets.targets.insert(value.into(), target);
        builder.current = target;
        f(builder);
        builder.goto(*next);
    }
    pub fn mk_otherwise(self, f: impl FnOnce(&mut PatternsBuilder<'tcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.patterns.basic_blocks.push(BasicBlockData::default());
        targets.otherwise = Some(target);
        builder.current = target;
        f(builder);
        builder.goto(next);
    }
}

impl<'tcx> std::ops::Deref for SwitchIntBuilder<'_, 'tcx> {
    type Target = PatternsBuilder<'tcx>;
    fn deref(&self) -> &Self::Target {
        self.builder
    }
}

impl<'tcx> std::ops::DerefMut for SwitchIntBuilder<'_, 'tcx> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.builder
    }
}

impl<'tcx> Patterns<'tcx> {
    fn mk_symbols(&self, syms: &[&str]) -> &'tcx [Symbol] {
        self.arena.alloc_from_iter(syms.iter().copied().map(Symbol::intern))
    }
    fn mk_slice<T: Copy>(&self, slice: &[T]) -> &'tcx [T] {
        if slice.is_empty() {
            return &[];
        }
        self.arena.alloc_slice(slice)
    }
}

impl<'tcx> Patterns<'tcx> {
    pub fn mk_type_relative(&self, ty: Ty<'tcx>, path: &str) -> Path<'tcx> {
        Path::TypeRelative(ty, Symbol::intern(path))
    }
    pub fn mk_generic_args(&self, generics: &[GenericArgKind<'tcx>]) -> GenericArgsRef<'tcx> {
        GenericArgsRef(self.mk_slice(generics))
    }
    pub fn mk_var_ty(&self, ty_var: TyVarIdx) -> Ty<'tcx> {
        self.mk_ty(TyKind::TyVar(ty_var))
    }
    pub fn mk_item_path(&self, path: &[&str]) -> ItemPath<'tcx> {
        ItemPath(self.mk_symbols(path))
    }
    pub fn mk_adt_ty(&self, path: ItemPath<'tcx>, generics: GenericArgsRef<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::Adt(path, generics))
    }
    pub fn mk_slice_ty(&self, ty: Ty<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::Slice(ty))
    }
    pub fn mk_ref_ty(&self, region: RegionKind, ty: Ty<'tcx>, mutability: mir::Mutability) -> Ty<'tcx> {
        self.mk_ty(TyKind::Ref(region, ty, mutability))
    }
    pub fn mk_raw_ptr_ty(&self, ty: Ty<'tcx>, mutability: mir::Mutability) -> Ty<'tcx> {
        self.mk_ty(TyKind::RawPtr(ty, mutability))
    }
    pub fn mk_fn(&self, func: impl Into<Path<'tcx>>, generics: GenericArgsRef<'tcx>) -> Ty<'tcx> {
        self.mk_ty(TyKind::FnDef(func.into(), generics))
    }
    pub fn mk_zeroed(&self, ty: Ty<'tcx>) -> Operand<'tcx> {
        Operand::Constant(ConstOperand::ZeroSized(ty))
    }
    pub fn mk_projection(&self, projection: &[PlaceElem<'tcx>]) -> &'tcx [PlaceElem<'tcx>] {
        self.mk_slice(projection)
    }
    fn mk_ty(&self, kind: TyKind<'tcx>) -> Ty<'tcx> {
        Ty(self.arena.alloc(kind))
    }
}
