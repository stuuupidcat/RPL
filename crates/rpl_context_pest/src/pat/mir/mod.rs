use core::iter::IntoIterator;
use std::ops::Index;

use either::Either;
use rustc_abi::FieldIdx;
use rustc_data_structures::fx::FxIndexMap;
use rustc_hir::Target;
use rustc_index::IndexVec;
use rustc_middle::mir;
use rustc_span::Symbol;

mod pretty;
pub mod visitor;

pub use super::*;

rustc_index::newtype_index! {
    #[debug_format = "_?{}"]
    pub struct Local {}
}

rustc_index::newtype_index! {
    #[debug_format = "?bb{}"]
    pub struct BasicBlock {}
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub block: BasicBlock,
    pub statement_index: usize,
}

impl From<(BasicBlock, usize)> for Location {
    fn from((block, statement_index): (BasicBlock, usize)) -> Self {
        Self { block, statement_index }
    }
}

impl Location {
    /// Create a new `Location` that is out of bound.
    ///
    /// Must be assigned to before use.
    ///
    /// # Note
    ///
    /// The return value is actually initialized to an invalid location
    /// `Location { block: 0xFFFF_FF00u32, statement_index: usize::MAX }`.
    pub fn uninitialized() -> Self {
        Self {
            // block: BasicBlock::from(u32::MAX),
            block: BasicBlock::from(0xFFFF_FF00u32),
            statement_index: usize::MAX,
        }
    }
}

pub struct MirPattern<'pcx> {
    pub self_idx: Option<Local>,
    pub return_idx: Option<Local>,
    pub locals: IndexVec<Local, Ty<'pcx>>,
    pub basic_blocks: IndexVec<BasicBlock, BasicBlockData<'pcx>>,
}

impl<'pcx> Index<BasicBlock> for MirPattern<'pcx> {
    type Output = BasicBlockData<'pcx>;

    fn index(&self, bb: BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

#[derive(Default)]
pub struct BasicBlockData<'pcx> {
    pub statements: Vec<StatementKind<'pcx>>,
    pub terminator: Option<TerminatorKind<'pcx>>,
}

impl<'pcx> BasicBlockData<'pcx> {
    pub fn has_pat_end(&self) -> bool {
        matches!(self.terminator(), TerminatorKind::PatEnd)
    }
    pub fn terminator(&self) -> &TerminatorKind<'pcx> {
        self.terminator.as_ref().expect("terminator not set")
    }
    pub fn debug_stmt_at(&self, index: usize) -> &dyn core::fmt::Debug {
        if index < self.statements.len() {
            &self.statements[index]
        } else {
            self.terminator()
        }
    }
    fn set_terminator(&mut self, terminator: TerminatorKind<'pcx>) {
        assert!(self.terminator.is_none(), "terminator already set");
        self.terminator = Some(terminator);
    }
    fn set_goto(&mut self, block: BasicBlock) {
        match &mut self.terminator {
            None => self.terminator = Some(TerminatorKind::Goto(block)),
            Some(TerminatorKind::Call { target, .. } | TerminatorKind::Drop { target, .. }) => *target = block,
            // Here the `goto ?bb` termiantor comes from `break` or `continue`,
            // plus the `return` termnator, are all skipped because thay are
            // abnormal control flows.
            Some(TerminatorKind::Goto(_) | TerminatorKind::Return) => {},
            Some(terminator @ (TerminatorKind::SwitchInt { .. } | TerminatorKind::PatEnd)) => {
                panic!("expect `{:?}`, but found `{terminator:?}`", TerminatorKind::Goto(block));
            },
        }
    }
    fn set_switch_targets(&mut self, switch_targets: SwitchTargets) {
        match &mut self.terminator {
            Some(TerminatorKind::SwitchInt { targets, .. }) => *targets = switch_targets,
            None => panic!("`switchInt` terminator not set"),
            Some(terminator) => panic!("expect `switchInt` terminator, but found `{terminator:?}`"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PlaceElem<'pcx> {
    Deref,
    Field(FieldAcc),
    FieldPat(Symbol),
    Index(Local),
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
    DowncastPat(Symbol),
    OpaqueCast(Ty<'pcx>),
    Subtype(Ty<'pcx>),
}

#[derive(Clone, Copy)]
pub struct Place<'pcx> {
    pub local: Local,
    pub projection: &'pcx [PlaceElem<'pcx>],
}

impl<'pcx> Place<'pcx> {
    pub fn new(local: Local, projection: &'pcx [PlaceElem<'pcx>]) -> Self {
        Self { local, projection }
    }
    pub fn as_local(&self) -> Option<Local> {
        self.projection.is_empty().then_some(self.local)
    }

    /// Iterate over the projections in evaluation order, i.e., the first element is the base with
    /// its projection and then subsequently more projections are added.
    /// As a concrete example, given the place a.b.c, this would yield:
    /// - (a, .b)
    /// - (a.b, .c)
    ///
    /// Given a place without projections, the iterator is empty.
    #[inline]
    pub fn iter_projections(self) -> impl DoubleEndedIterator<Item = (Place<'pcx>, PlaceElem<'pcx>)> {
        self.projection.iter().enumerate().map(move |(i, proj)| {
            let base = Place {
                local: self.local,
                projection: &self.projection[..i],
            };
            (base, *proj)
        })
    }
}

impl From<Local> for Place<'_> {
    fn from(local: Local) -> Self {
        Place { local, projection: &[] }
    }
}

impl Local {
    pub fn into_place<'pcx>(self) -> Place<'pcx> {
        self.into()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlaceTy<'pcx> {
    pub ty: Ty<'pcx>,
    pub variant: Option<Symbol>,
}

impl<'pcx> PlaceTy<'pcx> {
    pub fn from_ty(ty: Ty<'pcx>) -> Self {
        Self { ty, variant: None }
    }
    pub fn projection_ty(&self, pat: &'pcx Pattern<'pcx>, proj: PlaceElem<'pcx>) -> Option<Self> {
        match proj {
            PlaceElem::Deref => match self.ty.kind() {
                &TyKind::Ref(_, ty, _) | &TyKind::RawPtr(ty, _) => Some(PlaceTy::from_ty(ty)),
                _ => None,
            },
            PlaceElem::Field(_) => None,
            PlaceElem::FieldPat(field) => {
                let &TyKind::AdtPat(adt) = self.ty.kind() else {
                    return None;
                };
                let adt = pat.get_adt(adt)?;
                let variant = if adt.is_enum() {
                    adt.variant(
                        self.variant
                            .expect("Cannot assess field without downcasting to a variant"),
                    )
                } else {
                    adt.non_enum_variant()
                };
                Some(PlaceTy::from_ty(variant.fields.get(&field)?.ty))
            },
            PlaceElem::Index(_) | PlaceElem::ConstantIndex { .. } => match self.ty.kind() {
                &TyKind::Array(ty, _) | &TyKind::Slice(ty) => Some(PlaceTy::from_ty(ty)),
                _ => None,
            },
            PlaceElem::Subslice { .. } => match self.ty.kind() {
                &TyKind::Array(ty, _) | &TyKind::Slice(ty) => Some(PlaceTy::from_ty(pat.pcx.mk_slice_ty(ty))),
                _ => None,
            },
            PlaceElem::Downcast(_) => None,
            PlaceElem::DowncastPat(variant) => Some(PlaceTy {
                ty: self.ty,
                variant: Some(variant),
            }),
            PlaceElem::OpaqueCast(ty) | PlaceElem::Subtype(ty) => Some(PlaceTy::from_ty(ty)),
        }
    }
}

pub enum StatementKind<'pcx> {
    Assign(Place<'pcx>, Rvalue<'pcx>),
}

#[derive(Default)]
pub struct SwitchTargets {
    pub targets: FxIndexMap<IntValue, BasicBlock>,
    pub otherwise: Option<BasicBlock>,
}

pub enum TerminatorKind<'pcx> {
    SwitchInt {
        operand: Operand<'pcx>,
        targets: SwitchTargets,
    },
    Goto(BasicBlock),
    Call {
        func: Operand<'pcx>,
        args: List<Operand<'pcx>>,
        destination: Option<Place<'pcx>>,
        target: BasicBlock,
    },
    Drop {
        place: Place<'pcx>,
        target: BasicBlock,
    },
    Return,
    /// Pattern ends here
    PatEnd,
}

pub enum Rvalue<'pcx> {
    Any,
    Use(Operand<'pcx>),
    Repeat(Operand<'pcx>, Const<'pcx>),
    Ref(RegionKind, mir::BorrowKind, Place<'pcx>),
    RawPtr(mir::Mutability, Place<'pcx>),
    Len(Place<'pcx>),
    Cast(mir::CastKind, Operand<'pcx>, Ty<'pcx>),
    BinaryOp(mir::BinOp, Box<[Operand<'pcx>; 2]>),
    NullaryOp(mir::NullOp<'pcx>, Ty<'pcx>),
    UnaryOp(mir::UnOp, Operand<'pcx>),
    Discriminant(Place<'pcx>),
    Aggregate(AggKind<'pcx>, List<Operand<'pcx>>),
    ShallowInitBox(Operand<'pcx>, Ty<'pcx>),
    CopyForDeref(Place<'pcx>),
}

//FIXME: Add a new variant for `Copy` or `Move` a value that is a `Copy` type.
#[derive(Clone)]
pub enum Operand<'pcx> {
    Any,
    Copy(Place<'pcx>),
    Move(Place<'pcx>),
    Constant(ConstOperand<'pcx>),
    FnPat(Symbol),
}

pub type List<T> = Box<[T]>;

#[derive(Clone)]
pub enum ConstOperand<'pcx> {
    ConstVar(ConstVar<'pcx>),
    ScalarInt(IntValue),
    ZeroSized(PathWithArgs<'pcx>),
}

#[derive(Debug, Clone)]
pub enum AggAdtKind {
    Unit,
    Tuple,
    Struct(List<Symbol>),
}

impl From<List<Symbol>> for AggAdtKind {
    fn from(fields: List<Symbol>) -> Self {
        AggAdtKind::Struct(fields)
    }
}

#[derive(Debug, Clone)]
pub enum AggKind<'pcx> {
    Array,
    Tuple,
    Adt(PathWithArgs<'pcx>, AggAdtKind),
    RawPtr(Ty<'pcx>, mir::Mutability),
}

#[derive(Clone, Copy)]
pub enum FieldAcc {
    Named(Symbol),
    Unnamed(FieldIdx),
}

impl From<&str> for FieldAcc {
    fn from(name: &str) -> Self {
        Symbol::intern(name).into()
    }
}

impl From<Symbol> for FieldAcc {
    fn from(name: Symbol) -> Self {
        FieldAcc::Named(name)
    }
}

impl From<u32> for FieldAcc {
    fn from(field: u32) -> Self {
        FieldIdx::from_u32(field).into()
    }
}

impl From<FieldIdx> for FieldAcc {
    fn from(field: FieldIdx) -> Self {
        FieldAcc::Unnamed(field)
    }
}

pub struct MirPatternBuilder<'pcx> {
    pattern: MirPattern<'pcx>,
    loop_stack: Vec<Loop>,
    current: BasicBlock,
}

struct Loop {
    enter: BasicBlock,
    exit: BasicBlock,
}

impl<'pcx> MirPattern<'pcx> {
    pub fn builder() -> MirPatternBuilder<'pcx> {
        MirPatternBuilder::new()
    }
    pub fn stmt_at(&self, loc: Location) -> Either<&StatementKind<'pcx>, &TerminatorKind<'pcx>> {
        if loc.statement_index < self[loc.block].statements.len() {
            Either::Left(&self[loc.block].statements[loc.statement_index])
        } else {
            Either::Right(self[loc.block].terminator())
        }
    }
}

impl<'pcx> MirPatternBuilder<'pcx> {
    fn new() -> Self {
        let mut pattern = MirPattern {
            locals: IndexVec::new(),
            return_idx: None,
            self_idx: None,
            basic_blocks: IndexVec::new(),
        };
        let current = pattern.basic_blocks.push(BasicBlockData::default());
        Self {
            pattern,
            loop_stack: Vec::new(),
            current,
        }
    }
    pub fn build(mut self) -> MirPattern<'pcx> {
        self.new_block_if_terminated();
        self.pattern.basic_blocks[self.current].set_terminator(TerminatorKind::PatEnd);
        self.pattern
    }

    pub fn mk_local(&mut self, ty: Ty<'pcx>) -> Local {
        self.pattern.locals.push(ty)
    }
    pub fn mk_return(&mut self, ty: Ty<'pcx>) -> Local {
        *self.pattern.return_idx.insert(self.pattern.locals.push(ty))
    }
    pub fn mk_self(&mut self, ty: Ty<'pcx>) -> Local {
        *self.pattern.self_idx.insert(self.pattern.locals.push(ty))
    }
    fn new_block_if_terminated(&mut self) {
        if self.pattern.basic_blocks[self.current].terminator.is_some() {
            self.current = self.pattern.basic_blocks.push(BasicBlockData::default());
        }
    }
    fn next_block(&mut self) -> BasicBlock {
        self.new_block_if_terminated();
        self.pattern.basic_blocks.next_index()
    }
    fn mk_statement(&mut self, kind: StatementKind<'pcx>) -> Location {
        self.new_block_if_terminated();

        let block = self.current;
        let statement_index = self.pattern.basic_blocks[block].statements.len();

        self.pattern.basic_blocks[block].statements.push(kind);
        Location { block, statement_index }
    }
    fn set_terminator(&mut self, kind: TerminatorKind<'pcx>) -> Location {
        self.pattern.basic_blocks[self.current].set_terminator(kind);
        self.pattern.terminator_loc(self.current)
    }
    pub fn mk_assign(&mut self, place: impl Into<Place<'pcx>>, rvalue: Rvalue<'pcx>) -> Location {
        self.mk_statement(StatementKind::Assign(place.into(), rvalue))
    }
    pub fn mk_fn_call(
        &mut self,
        func: Operand<'pcx>,
        args: List<Operand<'pcx>>,
        destination: Option<Place<'pcx>>,
    ) -> Location {
        if let Some(place) = destination
            && let Operand::Constant(ConstOperand::ZeroSized(
                path_with_args @ PathWithArgs {
                    path: Path::LangItem(lang_item),
                    ..
                },
            )) = func
            && let Target::Variant | Target::Struct | Target::Union = lang_item.target()
        {
            return self.mk_assign(
                place,
                Rvalue::Aggregate(AggKind::Adt(path_with_args, AggAdtKind::Tuple), args),
            );
        }
        let target = self.next_block();
        self.set_terminator(TerminatorKind::Call {
            func,
            args,
            destination,
            target,
        })
    }
    pub fn mk_drop(&mut self, place: impl Into<Place<'pcx>>) -> Location {
        let target = self.next_block();
        let place = place.into();
        self.set_terminator(TerminatorKind::Drop { place, target })
    }
    pub fn mk_switch_int(&mut self, operand: Operand<'pcx>, f: impl FnOnce(SwitchIntBuilder<'_, 'pcx>)) -> Location {
        self.new_block_if_terminated();
        let current = self.current;
        self.pattern.basic_blocks[current].set_terminator(TerminatorKind::SwitchInt {
            operand,
            targets: SwitchTargets::default(),
        });
        let next = self.pattern.basic_blocks.push(BasicBlockData::default());
        let mut targets = SwitchTargets::default();
        let builder = SwitchIntBuilder {
            builder: self,
            next,
            targets: &mut targets,
        };
        f(builder);
        self.pattern.basic_blocks[current].set_switch_targets(targets);
        self.current = next;
        self.pattern.terminator_loc(current)
    }
    fn mk_goto(&mut self, block: BasicBlock) -> Location {
        self.pattern.basic_blocks[self.current].set_goto(block);
        self.pattern.terminator_loc(self.current)
    }
    pub fn mk_loop(&mut self, f: impl FnOnce(&mut MirPatternBuilder<'pcx>)) -> Location {
        let enter = self.pattern.basic_blocks.push(BasicBlockData::default());
        self.mk_goto(enter);
        let exit = self.pattern.basic_blocks.push(BasicBlockData::default());
        self.loop_stack.push(Loop { enter, exit });
        self.current = enter;
        f(self);
        self.loop_stack.pop();
        let location = self.mk_goto(enter);
        self.current = exit;
        location
    }
    pub fn mk_break(&mut self) -> Location {
        let exit = self.loop_stack.last().expect("no loop to break from").exit;
        self.mk_goto(exit)
    }
    pub fn mk_continue(&mut self) -> Location {
        let enter = self.loop_stack.last().expect("no loop to continue").enter;
        self.mk_goto(enter)
    }
}

impl<'pcx> std::ops::Deref for MirPatternBuilder<'pcx> {
    type Target = MirPattern<'pcx>;

    fn deref(&self) -> &Self::Target {
        &self.pattern
    }
}

pub struct SwitchIntBuilder<'a, 'pcx> {
    builder: &'a mut MirPatternBuilder<'pcx>,
    next: BasicBlock,
    targets: &'a mut SwitchTargets,
}

impl<'pcx> SwitchIntBuilder<'_, 'pcx> {
    pub fn mk_switch_target(&mut self, value: impl Into<IntValue>, f: impl FnOnce(&mut MirPatternBuilder<'pcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.pattern.basic_blocks.push(BasicBlockData::default());
        targets.targets.insert(value.into(), target);
        builder.current = target;
        f(builder);
        builder.mk_goto(*next);
    }
    pub fn mk_otherwise(self, f: impl FnOnce(&mut MirPatternBuilder<'pcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.pattern.basic_blocks.push(BasicBlockData::default());
        targets.otherwise = Some(target);
        builder.current = target;
        f(builder);
        builder.mk_goto(next);
    }
}

impl<'pcx> std::ops::Deref for SwitchIntBuilder<'_, 'pcx> {
    type Target = MirPatternBuilder<'pcx>;
    fn deref(&self) -> &Self::Target {
        self.builder
    }
}

impl std::ops::DerefMut for SwitchIntBuilder<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.builder
    }
}

impl MirPattern<'_> {
    pub fn terminator_loc(&self, block: BasicBlock) -> Location {
        // assert the terminator is set
        let _ = self.basic_blocks[block].terminator();
        let statement_index = self.basic_blocks[block].statements.len();
        Location { block, statement_index }
    }
}

impl<'pcx> MirPattern<'pcx> {
    pub fn mk_zeroed(&self, path_with_args: PathWithArgs<'pcx>) -> ConstOperand<'pcx> {
        ConstOperand::ZeroSized(path_with_args)
    }
    pub fn mk_list<T>(&self, items: impl IntoIterator<Item = T>) -> List<T> {
        items.into_iter().collect()
    }
}

impl BasicBlockData<'_> {
    pub fn num_statements_and_terminator(&self) -> usize {
        self.statements.len() + self.terminator.is_some() as usize
    }
}
