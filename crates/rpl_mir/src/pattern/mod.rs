use core::iter::IntoIterator;
use std::ops::Index;

use rpl_context::PatCtxt;
use rustc_data_structures::fx::FxIndexMap;
use rustc_hir::Target;
use rustc_index::IndexVec;
use rustc_middle::mir;
use rustc_span::Symbol;
use rustc_target::abi::FieldIdx;

mod pretty;
pub mod visitor;

pub use rpl_context::pat::*;

rustc_index::newtype_index! {
    #[debug_format = "_?{}"]
    pub struct LocalIdx {}
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

pub struct MirPattern<'pcx, 'tcx> {
    pub pcx: PatCtxt<'pcx, 'tcx>,
    pub(crate) ty_vars: IndexVec<TyVarIdx, TyVar<'tcx>>,
    pub(crate) const_vars: IndexVec<ConstVarIdx, ConstVar<'tcx>>,
    pub(crate) self_idx: Option<LocalIdx>,
    pub locals: IndexVec<LocalIdx, Ty<'tcx>>,
    pub basic_blocks: IndexVec<BasicBlock, BasicBlockData<'tcx>>,
}

impl<'tcx> Index<BasicBlock> for MirPattern<'_, 'tcx> {
    type Output = BasicBlockData<'tcx>;

    fn index(&self, bb: BasicBlock) -> &Self::Output {
        &self.basic_blocks[bb]
    }
}

#[derive(Default)]
pub struct BasicBlockData<'tcx> {
    pub statements: Vec<StatementKind<'tcx>>,
    pub terminator: Option<TerminatorKind<'tcx>>,
}

impl<'tcx> BasicBlockData<'tcx> {
    pub fn terminator(&self) -> &TerminatorKind<'tcx> {
        self.terminator.as_ref().expect("terminator not set")
    }
    pub fn debug_stmt_at(&self, index: usize) -> &dyn core::fmt::Debug {
        if index < self.statements.len() {
            &self.statements[index]
        } else {
            self.terminator()
        }
    }
    fn set_terminator(&mut self, terminator: TerminatorKind<'tcx>) {
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

    /// Iterate over the projections in evaluation order, i.e., the first element is the base with
    /// its projection and then subsequently more projections are added.
    /// As a concrete example, given the place a.b.c, this would yield:
    /// - (a, .b)
    /// - (a.b, .c)
    ///
    /// Given a place without projections, the iterator is empty.
    #[inline]
    pub fn iter_projections(self) -> impl DoubleEndedIterator<Item = (Place<'tcx>, PlaceElem<'tcx>)> {
        self.projection.iter().enumerate().map(move |(i, proj)| {
            let base = Place {
                local: self.local,
                projection: &self.projection[..i],
            };
            (base, *proj)
        })
    }
}

impl From<LocalIdx> for Place<'_> {
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
    /// Pattern ends here
    PatEnd,
}

pub enum Rvalue<'tcx> {
    Any,
    Use(Operand<'tcx>),
    Repeat(Operand<'tcx>, Const<'tcx>),
    Ref(RegionKind, mir::BorrowKind, Place<'tcx>),
    RawPtr(mir::Mutability, Place<'tcx>),
    Len(Place<'tcx>),
    Cast(mir::CastKind, Operand<'tcx>, Ty<'tcx>),
    BinaryOp(mir::BinOp, Box<[Operand<'tcx>; 2]>),
    NullaryOp(mir::NullOp<'tcx>, Ty<'tcx>),
    UnaryOp(mir::UnOp, Operand<'tcx>),
    Discriminant(Place<'tcx>),
    Aggregate(AggKind<'tcx>, List<Operand<'tcx>>),
    ShallowInitBox(Operand<'tcx>, Ty<'tcx>),
    CopyForDeref(Place<'tcx>),
}

#[derive(Clone)]
pub enum Operand<'tcx> {
    Any,
    Copy(Place<'tcx>),
    Move(Place<'tcx>),
    Constant(ConstOperand<'tcx>),
}

#[derive(Clone)]
pub enum FnOperand<'tcx> {
    Copy(Place<'tcx>),
    Move(Place<'tcx>),
    Constant(ConstOperand<'tcx>),
}

pub type List<T> = Box<[T]>;

#[derive(Clone)]
pub enum ConstOperand<'tcx> {
    ConstVar(ConstVar<'tcx>),
    ScalarInt(IntValue),
    ZeroSized(PathWithArgs<'tcx>),
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
pub enum AggKind<'tcx> {
    Array,
    Tuple,
    Adt(PathWithArgs<'tcx>, AggAdtKind),
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

pub struct MirPatternBuilder<'pcx, 'tcx> {
    pattern: MirPattern<'pcx, 'tcx>,
    loop_stack: Vec<Loop>,
    current: BasicBlock,
}

struct Loop {
    enter: BasicBlock,
    exit: BasicBlock,
}

impl<'pcx, 'tcx> MirPatternBuilder<'pcx, 'tcx> {
    pub fn new(pcx: PatCtxt<'pcx, 'tcx>) -> Self {
        let mut pattern = MirPattern {
            pcx,
            locals: IndexVec::new(),
            ty_vars: IndexVec::new(),
            const_vars: IndexVec::new(),
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
    pub fn build(mut self) -> MirPattern<'pcx, 'tcx> {
        self.new_block_if_terminated();
        self.pattern.basic_blocks[self.current].set_terminator(TerminatorKind::PatEnd);
        self.pattern
    }

    pub fn new_ty_var(&mut self) -> TyVar<'tcx> {
        self.pattern.mk_ty_var(None)
    }
    pub fn set_ty_var_pred(&mut self, ty_var: TyVarIdx, pred: TyPred<'tcx>) {
        self.pattern.ty_vars[ty_var].pred = Some(pred);
    }
    pub fn mk_const_var(&mut self, ty: Ty<'tcx>) -> ConstVar<'tcx> {
        self.pattern.mk_const_var(ty)
    }
    pub fn mk_local(&mut self, ty: Ty<'tcx>) -> LocalIdx {
        self.pattern.locals.push(ty)
    }
    pub fn mk_self(&mut self, ty: Ty<'tcx>) -> LocalIdx {
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
    fn mk_statement(&mut self, kind: StatementKind<'tcx>) -> Location {
        self.new_block_if_terminated();

        let block = self.current;
        let statement_index = self.pattern.basic_blocks[block].statements.len();

        self.pattern.basic_blocks[block].statements.push(kind);
        Location { block, statement_index }
    }
    fn set_terminator(&mut self, kind: TerminatorKind<'tcx>) -> Location {
        self.pattern.basic_blocks[self.current].set_terminator(kind);
        self.pattern.terminator_loc(self.current)
    }
    pub fn mk_assign(&mut self, place: impl Into<Place<'tcx>>, rvalue: Rvalue<'tcx>) -> Location {
        self.mk_statement(StatementKind::Assign(place.into(), rvalue))
    }
    pub fn mk_fn_call(
        &mut self,
        func: Operand<'tcx>,
        args: List<Operand<'tcx>>,
        destination: Option<Place<'tcx>>,
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
    pub fn mk_drop(&mut self, place: impl Into<Place<'tcx>>) -> Location {
        let target = self.next_block();
        let place = place.into();
        self.set_terminator(TerminatorKind::Drop { place, target })
    }
    pub fn mk_switch_int(
        &mut self,
        operand: Operand<'tcx>,
        f: impl FnOnce(SwitchIntBuilder<'_, 'pcx, 'tcx>),
    ) -> Location {
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
    pub fn mk_loop(&mut self, f: impl FnOnce(&mut MirPatternBuilder<'pcx, 'tcx>)) -> Location {
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

impl<'pcx, 'tcx> std::ops::Deref for MirPatternBuilder<'pcx, 'tcx> {
    type Target = MirPattern<'pcx, 'tcx>;

    fn deref(&self) -> &Self::Target {
        &self.pattern
    }
}

pub struct SwitchIntBuilder<'a, 'pcx, 'tcx> {
    builder: &'a mut MirPatternBuilder<'pcx, 'tcx>,
    next: BasicBlock,
    targets: &'a mut SwitchTargets,
}

impl<'pcx, 'tcx> SwitchIntBuilder<'_, 'pcx, 'tcx> {
    pub fn mk_switch_target(&mut self, value: impl Into<IntValue>, f: impl FnOnce(&mut MirPatternBuilder<'pcx, 'tcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.pattern.basic_blocks.push(BasicBlockData::default());
        targets.targets.insert(value.into(), target);
        builder.current = target;
        f(builder);
        builder.mk_goto(*next);
    }
    pub fn mk_otherwise(self, f: impl FnOnce(&mut MirPatternBuilder<'pcx, 'tcx>)) {
        let Self { builder, next, targets } = self;
        let target = builder.pattern.basic_blocks.push(BasicBlockData::default());
        targets.otherwise = Some(target);
        builder.current = target;
        f(builder);
        builder.mk_goto(next);
    }
}

impl<'pcx, 'tcx> std::ops::Deref for SwitchIntBuilder<'_, 'pcx, 'tcx> {
    type Target = MirPatternBuilder<'pcx, 'tcx>;
    fn deref(&self) -> &Self::Target {
        self.builder
    }
}

impl std::ops::DerefMut for SwitchIntBuilder<'_, '_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.builder
    }
}

impl<'tcx> MirPattern<'_, 'tcx> {
    pub fn terminator_loc(&self, block: BasicBlock) -> Location {
        // assert the terminator is set
        let _ = self.basic_blocks[block].terminator();
        let statement_index = self.basic_blocks[block].statements.len();
        Location { block, statement_index }
    }
}

impl<'pcx, 'tcx> MirPattern<'pcx, 'tcx> {
    pub fn mk_ty_var(&mut self, pred: Option<TyPred<'tcx>>) -> TyVar<'tcx> {
        let idx = self.ty_vars.next_index();
        let ty_var = TyVar { idx, pred };
        self.ty_vars.push(ty_var);
        ty_var
    }
    pub fn mk_const_var(&mut self, ty: Ty<'tcx>) -> ConstVar<'tcx> {
        let idx = self.const_vars.next_index();
        let const_var = ConstVar { idx, ty };
        self.const_vars.push(const_var);
        const_var
    }
    pub fn mk_zeroed(&self, path_with_args: PathWithArgs<'tcx>) -> ConstOperand<'tcx> {
        ConstOperand::ZeroSized(path_with_args)
    }
    pub fn mk_list<T>(&self, items: impl IntoIterator<Item = T>) -> List<T> {
        items.into_iter().collect()
    }
    pub fn mk_projection(&self, projection: &[PlaceElem<'tcx>]) -> &'tcx [PlaceElem<'tcx>] {
        self.pcx.mk_slice(projection)
    }
}

impl BasicBlockData<'_> {
    pub fn num_statements_and_terminator(&self) -> usize {
        self.statements.len() + self.terminator.is_some() as usize
    }
}
