use core::iter::IntoIterator;
use std::fmt::{self, Debug};
use std::ops::Index;

use either::Either;
use rpl_parser::generics::{Choice5, Choice6, Choice12};
use rustc_abi::FieldIdx;
use rustc_data_structures::fx::FxIndexMap;
use rustc_hir::Target;
use rustc_index::IndexVec;
use rustc_middle::mir;

mod pretty;
pub mod visitor;

use super::utils::{
    binop_from_pair, borrow_kind_from_pair_mutability, collect_operands, mutability_from_pair_ptr_mutability,
    nullop_from_pair, unop_from_pair,
};
pub use super::*;

pub(crate) type FnSymbolTable<'i> = rpl_meta::symbol_table::Fn<'i>;

rustc_index::newtype_index! {
    #[debug_format = "_?{}"]
    pub struct Local {}
}

rustc_index::newtype_index! {
    #[debug_format = "?bb{}"]
    pub struct BasicBlock {}
}

pub struct LocalWithIdent<'pcx> {
    pub local: Local,
    pub ident: Ident<'pcx>,
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

impl PlaceElem<'_> {
    pub fn from_field(field: &pairs::MirPlaceField) -> Self {
        let (_, field) = field.get_matched();
        match field {
            Choice3::_0(ident) => PlaceElem::FieldPat(Symbol::intern(ident.span.as_str())),
            Choice3::_1(ident) => PlaceElem::Field(FieldAcc::from(Symbol::intern(ident.span.as_str()))),
            Choice3::_2(index) => {
                let index = index.span.as_str().parse::<u32>().expect("invalid field index");
                PlaceElem::Field(FieldAcc::from(index))
            },
        }
    }
}

/// Place base is the base of a place, which can be a local
/// or a [variable](`PlaceVar`) declared in meta table.
#[derive(Clone, Copy)]
pub enum PlaceBase {
    Local(Local),
    Var(PlaceVarIdx),
}

impl PlaceBase {
    pub fn as_local(self) -> Option<Local> {
        match self {
            PlaceBase::Local(local) => Some(local),
            PlaceBase::Var(_) => None,
        }
    }
}

impl Debug for PlaceBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaceBase::Local(local) => Debug::fmt(local, f),
            PlaceBase::Var(var) => Debug::fmt(var, f),
        }
    }
}

/// A place is a path to a value in memory.
#[derive(Clone, Copy)]
pub struct Place<'pcx, B = PlaceBase> {
    pub base: B,
    pub projection: &'pcx [PlaceElem<'pcx>],
}

impl<'pcx> Place<'pcx, PlaceBase> {
    pub fn new(local: Local, projection: &'pcx [PlaceElem<'pcx>]) -> Self {
        Self {
            base: PlaceBase::Local(local),
            projection,
        }
    }
    pub fn as_local(&self) -> Option<Local> {
        self.projection.is_empty().then(|| self.base.as_local()).flatten()
    }

    pub fn from(place: &pairs::MirPlace<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (base, suffix) = place.get_matched();
        let (base, mut base_projections) = match base.deref() {
            Choice3::_0(local) => match local.deref() {
                Choice4::_0(_place_holder) => {
                    panic!("expect a non-placeholder local");
                },
                _ => {
                    let local = sym_tab.inner.get_local_idx(Symbol::intern(local.span.as_str()));
                    (PlaceBase::Local(Local::from(local)), vec![])
                },
            },
            Choice3::_1(paren) => {
                let (_, place, _) = paren.get_matched();
                let Place { base, projection } = Place::from(place, pcx, sym_tab);
                (base, projection.to_vec())
            },
            Choice3::_2(deref) => {
                let (_, place) = deref.get_matched();
                let Place { base, projection } = Place::from(place, pcx, sym_tab);
                let mut new_projection = vec![PlaceElem::Deref];
                new_projection.extend(projection);
                (base, new_projection)
            },
        };
        let suffix_projections = suffix
            .iter_matched()
            .map(|suffix| match suffix.deref() {
                Choice5::_0(field) => PlaceElem::from_field(field),
                Choice5::_1(index) => {
                    let (_, local, _) = index.get_matched();
                    let local = sym_tab.inner.get_local_idx(Symbol::intern(local.span.as_str()));
                    PlaceElem::Index(Local::from(local))
                },
                Choice5::_2(const_index) => {
                    let (_, _, index, _, min_length, _) = const_index.get_matched();
                    let a = index.span.as_str().parse::<u64>().expect("invalid constant index");
                    let b = min_length.span.as_str().parse::<u64>().expect("invalid constant index");
                    PlaceElem::ConstantIndex {
                        offset: a,
                        min_length: b,
                        from_end: false, // FIXME
                    }
                },
                Choice5::_3(subslice) => {
                    let (_, from, _, minus, to, _) = subslice.get_matched();
                    let from = from
                        .as_ref()
                        .map(|from| from.span.as_str().parse::<u64>().expect("invalid subslice"));
                    let to = to
                        .as_ref()
                        .map(|to| to.span.as_str().parse::<u64>().expect("invalid subslice"));
                    PlaceElem::Subslice {
                        from: from.unwrap_or(0),
                        to: to.unwrap_or(0),
                        from_end: minus.is_some(),
                    }
                },
                Choice5::_4(downcast) => {
                    let (_, ident) = downcast.get_matched();
                    match ident {
                        Choice2::_0(ident) => PlaceElem::DowncastPat(Symbol::intern(ident.span.as_str())),
                        Choice2::_1(ident) => PlaceElem::Downcast(Symbol::intern(ident.span.as_str())),
                    }
                },
            })
            .collect::<Vec<_>>();
        base_projections.extend(suffix_projections);
        Self {
            base,
            projection: pcx.mk_slice(&base_projections),
        }
    }
}

impl<'pcx, B: Copy> Place<'pcx, B> {
    /// Iterate over the projections in evaluation order, i.e., the first element is the base with
    /// its projection and then subsequently more projections are added.
    /// As a concrete example, given the place a.b.c, this would yield:
    /// - (a, .b)
    /// - (a.b, .c)
    ///
    /// Given a place without projections, the iterator is empty.
    #[inline]
    pub fn iter_projections(self) -> impl DoubleEndedIterator<Item = (Self, PlaceElem<'pcx>)> {
        self.projection.iter().enumerate().map(move |(i, proj)| {
            let base = Place {
                base: self.base,
                projection: &self.projection[..i],
            };
            (base, *proj)
        })
    }

    /// Identity.
    pub fn into_place(self) -> Self {
        self
    }
}

impl<B> From<B> for Place<'_, B> {
    fn from(base: B) -> Self {
        Place { base, projection: &[] }
    }
}

impl From<Local> for Place<'_, PlaceBase> {
    fn from(local: Local) -> Self {
        Place {
            base: PlaceBase::Local(local),
            projection: &[],
        }
    }
}

impl From<PlaceVarIdx> for Place<'_, PlaceBase> {
    fn from(var: PlaceVarIdx) -> Self {
        Place {
            base: PlaceBase::Var(var),
            projection: &[],
        }
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

pub enum RawDecleration<'pcx> {
    TypeAlias(Symbol, Ty<'pcx>),
    UsePath(Path<'pcx>),
    LocalInit(Local, Option<RvalueOrCall<'pcx>>), // In meta pass, we have already collect the local and its ty
}

impl<'pcx> RawDecleration<'pcx> {
    pub fn from(decl: &pairs::MirDecl<'pcx>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        match decl.deref() {
            Choice3::_0(type_alias) => {
                let (_, name, _, ty, _) = type_alias.get_matched();
                Self::TypeAlias(
                    Symbol::intern(name.span.as_str()),
                    Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone()),
                )
            },
            Choice3::_1(use_path) => Self::UsePath(Path::from(use_path.get_matched().1, pcx)),
            Choice3::_2(local_init) => {
                let (_, _, local, _, _, init, _) = local_init.get_matched();
                let local = Local::from(fn_sym_tab.inner.get_local_idx(Symbol::intern(local.span.as_str())));
                let rvalue_or_call = if let Some(init) = init {
                    let (_, init) = init.get_matched();
                    let rvalue_or_call = RvalueOrCall::from(init, pcx, fn_sym_tab);
                    Some(rvalue_or_call)
                } else {
                    None
                };
                Self::LocalInit(local, rvalue_or_call)
            },
        }
    }
}

pub enum RawStatement<'pcx> {
    Assign(Place<'pcx>, Rvalue<'pcx>),
    CallIgnoreRet(Call<'pcx>),
    Drop(Place<'pcx>),
    Break,
    Continue,
    Loop(Vec<RawStatement<'pcx>>),
    SwitchInt {
        operand: Operand<'pcx>,
        targets: Vec<(IntValue, Vec<RawStatement<'pcx>>)>,
        otherwise: Option<Vec<RawStatement<'pcx>>>,
    },
}

impl<'pcx> RawStatement<'pcx> {
    pub fn from(stmt: &pairs::MirStmt<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        match stmt.deref() {
            Choice6::_0(call_ignore_ret) => Self::from_call_ignore_ret(call_ignore_ret.get_matched().0, pcx, sym_tab),
            Choice6::_1(drop_) => Self::from_drop(drop_.get_matched().0, pcx, sym_tab),
            Choice6::_2(control) => Self::from_control(control.get_matched().0),
            Choice6::_3(assign) => Self::from_assign(assign.get_matched().0, pcx, sym_tab),
            Choice6::_4(loop_) => Self::from_loop(loop_, pcx, sym_tab),
            Choice6::_5(switch_int) => Self::from_switch_int(switch_int, pcx, sym_tab),
        }
    }

    pub fn from_assign(stmt: &pairs::MirAssign<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (_, place, _, rvalue_or_call) = stmt.get_matched();
        let place = Place::from(place, pcx, sym_tab);
        let rvalue = match rvalue_or_call.deref() {
            Choice2::_0(_call) => todo!("call in mir assign"),
            Choice2::_1(rvalue) => Rvalue::from_rvalue(rvalue, pcx, sym_tab),
        };
        Self::Assign(place, rvalue)
    }

    pub fn from_call_ignore_ret(
        call_ignore_ret: &pairs::MirCallIgnoreRet<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        let (_label, _, _, call) = call_ignore_ret.get_matched();
        let call = Call::from(call, pcx, fn_sym_tab);
        Self::CallIgnoreRet(call)
    }

    pub fn from_drop(drop_: &pairs::MirDrop<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (_label, _, _, place, _) = drop_.get_matched();
        let place = Place::from(place, pcx, sym_tab);
        Self::Drop(place)
    }

    pub fn from_loop(loop_: &pairs::MirLoop<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (_, _, block) = loop_.get_matched();
        let statements = block
            .get_matched()
            .1
            .iter_matched()
            .map(|stmt| Self::from(stmt, pcx, sym_tab))
            .collect();
        Self::Loop(statements)
    }

    pub fn from_control(control: &pairs::MirControl<'pcx>) -> Self {
        let (_label, break_or_continue, _label2) = control.get_matched();
        match break_or_continue {
            Choice2::_0(_break) => Self::Break,
            Choice2::_1(_continue) => Self::Continue,
        }
    }

    pub fn from_switch_int(
        switch_int: &pairs::MirSwitchInt<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        let (_, _, op, _, _, targets, _) = switch_int.get_matched();
        let operand = Operand::from(op, pcx, fn_sym_tab);
        let mut target_value_and_stmts: Vec<(IntValue, Vec<Self>)> = Vec::new();
        let mut otherwise_stmts: Option<Vec<Self>> = None;
        targets.iter_matched().for_each(|target| {
            let (target_value, _, block) = target.get_matched();
            let target_value = IntValue::from_switch_int_value(target_value);
            let statements = Self::from_switch_int_block(block, pcx, fn_sym_tab);
            if let Some(target_value) = target_value {
                target_value_and_stmts.push((target_value, statements));
            } else {
                otherwise_stmts = Some(statements);
            }
        });
        Self::SwitchInt {
            operand,
            targets: target_value_and_stmts,
            otherwise: otherwise_stmts,
        }
    }

    pub fn from_switch_int_block(
        block: &pairs::MirSwitchBody<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Vec<Self> {
        let mut stmts = Vec::new();
        match block.deref() {
            Choice4::_0(mir_stmt_block) => {
                return Self::from_mir_stmt_block(mir_stmt_block, pcx, fn_sym_tab);
            },
            Choice4::_1(single_stmt_with_comma) => {
                let (stmt, _) = single_stmt_with_comma.get_matched();
                match stmt {
                    Choice4::_0(call_ignore_ret) => {
                        stmts.push(Self::from_call_ignore_ret(call_ignore_ret, pcx, fn_sym_tab));
                    },
                    Choice4::_1(drop) => stmts.push(Self::from_drop(drop, pcx, fn_sym_tab)),
                    Choice4::_2(control) => stmts.push(Self::from_control(control)),
                    Choice4::_3(assign) => stmts.push(Self::from_assign(assign, pcx, fn_sym_tab)),
                };
            },
            Choice4::_2(loop_) => stmts.push(Self::from_loop(loop_, pcx, fn_sym_tab)),
            Choice4::_3(switch_int) => stmts.push(Self::from_switch_int(switch_int, pcx, fn_sym_tab)),
        }
        stmts
    }

    pub fn from_mir_stmt_block(
        block: &pairs::MirStmtBlock<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Vec<Self> {
        block
            .get_matched()
            .1
            .iter_matched()
            .map(|stmt| Self::from(stmt, pcx, fn_sym_tab))
            .collect()
    }
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

impl<'pcx> Rvalue<'pcx> {
    fn from_rvalue(rvalue: &pairs::MirRvalue<'pcx>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        match rvalue.deref() {
            Choice12::_0(_any) => Rvalue::Any,
            Choice12::_1(cast) => {
                let (operand, _, ty, _, cast_kind, _) = cast.get_matched();
                let operand = Operand::from(operand, pcx, fn_sym_tab);
                let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
                let cast_kind = match cast_kind.deref() {
                    Choice3::_0(_ptr_to_ptr) => mir::CastKind::PtrToPtr,
                    Choice3::_1(_int_to_int) => mir::CastKind::IntToInt,
                    Choice3::_2(_transmute) => mir::CastKind::Transmute,
                };
                Rvalue::Cast(cast_kind, operand, ty)
            },
            Choice12::_2(rvalue_use) => {
                let operand = match rvalue_use.deref() {
                    Choice2::_0(op) => Operand::from(op.get_matched().1, pcx, fn_sym_tab),
                    Choice2::_1(op) => Operand::from(op, pcx, fn_sym_tab),
                };
                Self::Use(operand)
            },
            Choice12::_3(repeat) => {
                let (_, operand, _, count, _) = repeat.get_matched();
                let operand = Operand::from(operand, pcx, fn_sym_tab);
                let count = Const::from_integer(count);
                Self::Repeat(operand, count)
            },
            Choice12::_4(rvalue_ref) => {
                let (_, region, mutability, place) = rvalue_ref.get_matched();
                let region_kind = if let Some(region) = region {
                    RegionKind::from(region)
                } else {
                    RegionKind::ReAny
                };
                let mutability = borrow_kind_from_pair_mutability(mutability);
                let place = Place::from(place, pcx, fn_sym_tab);
                Self::Ref(region_kind, mutability, place)
            },
            Choice12::_5(raw_ptr) => {
                let (_, _, ptr_mutability, place) = raw_ptr.get_matched();
                let mutability = mutability_from_pair_ptr_mutability(ptr_mutability);
                let place = Place::from(place, pcx, fn_sym_tab);
                Self::RawPtr(mutability, place)
            },
            Choice12::_6(len) => {
                let (_, _, place, _) = len.get_matched();
                let place = Place::from(place, pcx, fn_sym_tab);
                Self::Len(place)
            },
            Choice12::_7(bin_op) => {
                let (bin_op, _, lop, _, rop, _) = bin_op.get_matched();
                let bin_op = binop_from_pair(bin_op);
                let lop = Operand::from(lop, pcx, fn_sym_tab);
                let rop = Operand::from(rop, pcx, fn_sym_tab);
                Self::BinaryOp(bin_op, Box::new([lop, rop]))
            },
            Choice12::_8(nullary_op) => {
                let (nullary_op, _, ty, _) = nullary_op.get_matched();
                let nullary_op = nullop_from_pair(nullary_op);
                let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
                Self::NullaryOp(nullary_op, ty)
            },
            Choice12::_9(un_op) => {
                let (un_op, _, operand, _) = un_op.get_matched();
                let un_op = unop_from_pair(un_op);
                let operand = Operand::from(operand, pcx, fn_sym_tab);
                Self::UnaryOp(un_op, operand)
            },
            Choice12::_10(discriminant) => {
                let (_, _, place, _) = discriminant.get_matched();
                let place = Place::from(place, pcx, fn_sym_tab);
                Self::Discriminant(place)
            },
            Choice12::_11(agg) => {
                let (agg_kind, operands) = AggKind::from(agg, pcx, fn_sym_tab);
                Self::Aggregate(agg_kind, operands)
            },
        }
    }
}

#[derive(Clone)]
pub enum Operand<'pcx> {
    Any,
    Copy(Place<'pcx>),
    Move(Place<'pcx>),
    Constant(ConstOperand<'pcx>),
    FnPat(Symbol),
}

impl<'pcx> Operand<'pcx> {
    pub fn from(op: &pairs::MirOperand<'pcx>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        match op.deref() {
            Choice6::_0(_any) => Self::Any,
            Choice6::_1(_any_multiple) => Self::Any, // FIXME
            Choice6::_2(meta_var) => Self::from_meta_var(meta_var),
            Choice6::_3(move_) => Self::from_move(move_, pcx, fn_sym_tab),
            Choice6::_4(copy_) => Self::from_copy(copy_, pcx, fn_sym_tab),
            Choice6::_5(konst) => Self::from_constant(konst, pcx, fn_sym_tab),
        }
    }

    pub fn from_meta_var(meta_var: &pairs::MetaVariable<'_>) -> Self {
        Self::FnPat(Symbol::intern(meta_var.span.as_str()))
    }

    pub fn from_move(
        move_: &pairs::MirOperandMove<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        Self::Move(Place::from(move_.MirPlace(), pcx, fn_sym_tab))
    }

    pub fn from_copy(
        copy_: &pairs::MirOperandCopy<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        Self::Copy(Place::from(copy_.MirPlace(), pcx, fn_sym_tab))
    }

    pub fn from_constant(
        konst: &pairs::MirOperandConst<'_>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        Self::Constant(ConstOperand::from(konst, pcx, fn_sym_tab))
    }

    pub fn from_fn_op(op: &pairs::MirFnOperand<'pcx>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        match op.deref() {
            Choice5::_0(copy_) => Self::from_copy(copy_.get_matched().1, pcx, fn_sym_tab),
            Choice5::_1(move_) => Self::from_move(move_.get_matched().1, pcx, fn_sym_tab),
            Choice5::_2(type_path) => Self::Constant(ConstOperand::from_type_path(type_path, pcx, fn_sym_tab)),
            Choice5::_3(lang_item) => Self::Constant(ConstOperand::from_lang_item(lang_item, pcx, fn_sym_tab)),
            Choice5::_4(meta_var) => Self::from_meta_var(meta_var),
        }
    }
}

pub struct Call<'pcx>(Operand<'pcx>, Vec<Operand<'pcx>>);

impl<'pcx> Call<'pcx> {
    pub fn from(call: &pairs::MirCall<'pcx>, pcx: PatCtxt<'pcx>, sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (fn_op, _, args, _) = call.get_matched();
        let func = Operand::from_fn_op(fn_op, pcx, sym_tab);
        let args = collect_operands(args, pcx, sym_tab);
        Self(func, args)
    }
}

pub enum RvalueOrCall<'pcx> {
    Rvalue(Rvalue<'pcx>),
    Call(Call<'pcx>),
}

impl<'pcx> RvalueOrCall<'pcx> {
    pub fn from(
        rvalue_or_call: &pairs::MirRvalueOrCall<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        match rvalue_or_call.deref() {
            Choice2::_0(call) => Self::Call(Call::from(call, pcx, fn_sym_tab)),
            Choice2::_1(rvalue) => Self::Rvalue(Rvalue::from_rvalue(rvalue, pcx, fn_sym_tab)),
        }
    }
}

pub type List<T> = Box<[T]>;

#[derive(Clone)]
pub enum ConstOperand<'pcx> {
    ConstVar(ConstVar<'pcx>),
    ScalarInt(IntValue),
    ZeroSized(PathWithArgs<'pcx>),
}

impl<'pcx> ConstOperand<'pcx> {
    fn from(op: &pairs::MirOperandConst<'_>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        let (_, op) = op.get_matched();
        match op {
            Choice3::_0(lit) => Self::from_literal(lit),
            Choice3::_1(lang_item_with_args) => Self::from_lang_item(lang_item_with_args, pcx, fn_sym_tab),
            Choice3::_2(type_path) => Self::from_type_path(type_path, pcx, fn_sym_tab),
        }
    }

    fn from_type_path(type_path: &pairs::TypePath<'_>, pcx: PatCtxt<'pcx>, fn_sym_tab: &FnSymbolTable<'pcx>) -> Self {
        Self::ZeroSized(PathWithArgs::from_type_path(
            type_path,
            pcx,
            fn_sym_tab.meta_vars.clone(),
        ))
    }

    fn from_lang_item(
        lang_item: &pairs::LangItemWithArgs<'_>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        Self::ZeroSized(PathWithArgs::from_lang_item(
            lang_item,
            pcx,
            fn_sym_tab.meta_vars.clone(),
        ))
    }

    fn from_literal(lit: &pairs::Literal<'_>) -> Self {
        match lit.deref() {
            Choice3::_0(integer) => Self::ScalarInt(IntValue::from_integer(integer)),
            _ => todo!("literal other than integer as const operand"),
        }
    }
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

impl<'pcx> AggKind<'pcx> {
    pub fn from(
        agg: &pairs::MirRvalueAggregate<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> (Self, List<Operand<'pcx>>) {
        match agg.deref() {
            Choice6::_0(array) => {
                let (_, operands, _) = array.get_matched();
                let operands = collect_operands(operands, pcx, fn_sym_tab);
                (Self::Array, operands.into_boxed_slice())
            },
            Choice6::_1(tuple) => {
                let (_, operands, _) = tuple.get_matched();
                let operands = collect_operands(operands, pcx, fn_sym_tab);
                (Self::Tuple, operands.into_boxed_slice())
            },
            Choice6::_2(adt_struct) => {
                let (path_or_lang_item, _, fields, _) = adt_struct.get_matched();
                let path_or_lang_item =
                    PathWithArgs::from_path_or_lang_item(path_or_lang_item, pcx, fn_sym_tab.meta_vars.clone());
                let (symbol_list, op_list): (List<Symbol>, List<Operand>) = if let Some(fields) = fields {
                    let fields = collect_elems_separated_by_comma!(fields);
                    let (symbols, ops): (Vec<Symbol>, Vec<Operand>) = fields
                        .map(|field| {
                            (
                                Symbol::intern(field.Identifier().span.as_str()),
                                Operand::from(field.MirOperand(), pcx, fn_sym_tab),
                            )
                        })
                        .unzip();
                    (symbols.into_boxed_slice(), ops.into_boxed_slice())
                } else {
                    (Box::new([]), Box::new([]))
                };
                let kind = AggAdtKind::Struct(symbol_list);
                (Self::Adt(path_or_lang_item, kind), op_list)
            },
            Choice6::_3(tuple) => {
                let (_, _, _, _, path, _, operands, _) = tuple.get_matched();
                let path = PathWithArgs::from_path(path, pcx, fn_sym_tab.meta_vars.clone());
                let operands = collect_operands(operands, pcx, fn_sym_tab);
                (Self::Adt(path, AggAdtKind::Tuple), operands.into_boxed_slice())
            },
            Choice6::_4(unit) => {
                let path_or_lang_item =
                    PathWithArgs::from_path_or_lang_item(unit.deref(), pcx, fn_sym_tab.meta_vars.clone());
                (Self::Adt(path_or_lang_item, AggAdtKind::Unit), Box::new([]))
            },
            Choice6::_5(raw_ptr) => {
                let (ty_ptr, _, _, op1, _, op2, _) = raw_ptr.get_matched();
                let (_, ptr_mutability, ty) = ty_ptr.get_matched();
                let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
                let mutability = mutability_from_pair_ptr_mutability(ptr_mutability);
                let operands = Box::new([Operand::from(op1, pcx, fn_sym_tab), Operand::from(op2, pcx, fn_sym_tab)]);
                (Self::RawPtr(ty, mutability), operands)
            },
        }
    }
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

    pub fn mk_locals(&mut self, fn_sym_tab: &FnSymbolTable<'pcx>, pcx: PatCtxt<'pcx>) {
        let locals = fn_sym_tab.inner.get_sorted_locals();
        for (_, ty) in locals {
            let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
            self.mk_local(ty);
        }
    }

    fn mk_local(&mut self, ty: Ty<'pcx>) -> Local {
        self.pattern.locals.push(ty)
    }

    #[allow(unused)]
    fn mk_return(&mut self, ty: Ty<'pcx>) -> Local {
        *self.pattern.return_idx.insert(self.pattern.locals.push(ty))
    }

    #[allow(unused)]
    fn mk_self(&mut self, ty: Ty<'pcx>) -> Local {
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

    pub fn mk_raw_stmts(&mut self, stmts: impl IntoIterator<Item = RawStatement<'pcx>>) {
        for stmt in stmts {
            let _loc = self.mk_raw_stmt(stmt);
        }
    }

    fn mk_raw_stmt(&mut self, kind: RawStatement<'pcx>) -> Location {
        match kind {
            RawStatement::Assign(place, rvalue) => self.mk_assign(StatementKind::Assign(place, rvalue)),
            RawStatement::CallIgnoreRet(Call(func, args)) => self.mk_fn_call(func, args.into_boxed_slice(), None),
            RawStatement::Drop(place) => self.mk_drop(place),
            RawStatement::Break => self.mk_break(),
            RawStatement::Continue => self.mk_continue(),
            RawStatement::Loop(stmts) => self.mk_loop(stmts),
            RawStatement::SwitchInt {
                operand,
                targets,
                otherwise,
            } => self.mk_switch_int(operand, targets, otherwise),
        }
    }

    pub fn mk_raw_decls(&mut self, decls: impl IntoIterator<Item = RawDecleration<'pcx>>) {
        for decl in decls {
            self.mk_raw_decl(decl);
        }
    }

    fn mk_raw_decl(&mut self, kind: RawDecleration<'pcx>) {
        if let RawDecleration::LocalInit(local, Some(rvalue_or_call)) = kind {
            match rvalue_or_call {
                RvalueOrCall::Rvalue(rvalue) => _ = self.mk_assign(StatementKind::Assign(local.into(), rvalue)),
                RvalueOrCall::Call(call) => _ = self.mk_fn_call(call.0, call.1.into_boxed_slice(), Some(local.into())),
            }
        }
    }

    fn mk_assign(&mut self, assign: StatementKind<'pcx>) -> Location {
        self.new_block_if_terminated();

        let block = self.current;
        let statement_index = self.pattern.basic_blocks[block].statements.len();

        self.pattern.basic_blocks[block].statements.push(assign);
        Location { block, statement_index }
    }

    fn set_terminator(&mut self, kind: TerminatorKind<'pcx>) -> Location {
        self.pattern.basic_blocks[self.current].set_terminator(kind);
        self.pattern.terminator_loc(self.current)
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
            return self.mk_assign(StatementKind::Assign(
                place,
                Rvalue::Aggregate(AggKind::Adt(path_with_args, AggAdtKind::Tuple), args),
            ));
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
    pub fn mk_switch_int(
        &mut self,
        operand: Operand<'pcx>,
        target_value_and_stmts: Vec<(IntValue, Vec<RawStatement<'pcx>>)>,
        otherwise_stmts: Option<Vec<RawStatement<'pcx>>>,
    ) -> Location {
        self.new_block_if_terminated();
        let current = self.current;
        self.pattern.basic_blocks[current].set_terminator(TerminatorKind::SwitchInt {
            operand,
            targets: SwitchTargets::default(),
        });
        let next = self.pattern.basic_blocks.push(BasicBlockData::default());
        let mut targets = SwitchTargets::default();
        for (value, stmts) in target_value_and_stmts {
            self.mk_switch_target(value, stmts, &mut targets, next);
        }
        if let Some(stmts) = otherwise_stmts {
            self.mk_otherwise(stmts, &mut targets, next);
        }
        self.pattern.basic_blocks[current].set_switch_targets(targets);
        self.current = next;
        self.pattern.terminator_loc(current)
    }
    pub fn mk_switch_target(
        &mut self,
        value: IntValue,
        stmts: impl IntoIterator<Item = RawStatement<'pcx>>,
        targets: &mut SwitchTargets,
        next: BasicBlock,
    ) {
        let target = self.pattern.basic_blocks.push(BasicBlockData::default());
        targets.targets.insert(value, target);
        self.current = target;
        for stmt in stmts {
            self.mk_raw_stmt(stmt);
        }
        self.mk_goto(next);
    }

    pub fn mk_otherwise(
        &mut self,
        stmts: impl IntoIterator<Item = RawStatement<'pcx>>,
        targets: &mut SwitchTargets,
        next: BasicBlock,
    ) {
        let target = self.pattern.basic_blocks.push(BasicBlockData::default());
        targets.otherwise = Some(target);
        self.current = target;
        for stmt in stmts {
            self.mk_raw_stmt(stmt);
        }
        self.mk_goto(next);
    }

    fn mk_goto(&mut self, block: BasicBlock) -> Location {
        self.pattern.basic_blocks[self.current].set_goto(block);
        self.pattern.terminator_loc(self.current)
    }
    pub fn mk_loop(&mut self, stmts: impl IntoIterator<Item = RawStatement<'pcx>>) -> Location {
        let enter = self.pattern.basic_blocks.push(BasicBlockData::default());
        self.mk_goto(enter);
        let exit = self.pattern.basic_blocks.push(BasicBlockData::default());
        self.loop_stack.push(Loop { enter, exit });
        self.current = enter;
        for stmt in stmts {
            self.mk_raw_stmt(stmt);
        }
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
