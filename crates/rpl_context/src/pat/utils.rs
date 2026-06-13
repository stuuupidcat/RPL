use std::ops::Deref;

use derive_more::derive::Debug;
use pest_typed::Span;
use rpl_meta::collect_elems_separated_by_comma;
use rpl_meta::symbol_table::WithPath;
use rpl_parser::generics::{Choice3, Choice15};
use rpl_parser::pairs::{self};
use rustc_middle::mir;
use rustc_span::Symbol;

use super::{FnSymbolTable, with_path};
use crate::PatCtxt;
use crate::pat::mir::Operand;

/// Identifier in RPL meta language.
#[derive(Copy, Clone, Debug)]
#[debug("{name}")]
pub struct Ident<'i> {
    pub name: Symbol,
    pub span: Span<'i>,
}

impl PartialEq for Ident<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Ident<'_> {}

use std::hash::{Hash, Hasher};

impl Hash for Ident<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<'i, const INHERITED: usize> From<&pairs::Word<'i, INHERITED>> for Ident<'i> {
    fn from(ident: &pairs::Word<'i, INHERITED>) -> Self {
        let span = ident.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'i> From<&pairs::Identifier<'i>> for Ident<'i> {
    fn from(ident: &pairs::Identifier<'i>) -> Self {
        let span = ident.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'i> From<&pairs::MetaVariable<'i>> for Ident<'i> {
    fn from(meta: &pairs::MetaVariable<'i>) -> Self {
        let span = meta.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'i> From<&pairs::kw_self<'i>> for Ident<'i> {
    fn from(meta: &pairs::kw_self<'i>) -> Self {
        let span = meta.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}
impl<'i> From<&pairs::SelfParam<'i>> for Ident<'i> {
    fn from(meta: &pairs::SelfParam<'i>) -> Self {
        meta.kw_self().into()
    }
}

impl<'i> From<&pairs::Dollarself<'i>> for Ident<'i> {
    fn from(meta: &pairs::Dollarself<'i>) -> Self {
        let span = meta.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'i> From<&pairs::DollarRET<'i>> for Ident<'i> {
    fn from(meta: &pairs::DollarRET<'i>) -> Self {
        let span = meta.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'i> From<Span<'i>> for Ident<'i> {
    fn from(span: Span<'i>) -> Self {
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl std::fmt::Display for Ident<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.name, f)
    }
}

pub(crate) fn mutability_from_pair_mutability(pair: &pairs::Mutability<'_>) -> mir::Mutability {
    if pair.kw_mut().is_some() {
        mir::Mutability::Mut
    } else {
        mir::Mutability::Not
    }
}

pub(crate) fn mutability_from_pair_ptr_mutability(pair: &pairs::PtrMutability<'_>) -> mir::Mutability {
    if pair.kw_mut().is_some() {
        mir::Mutability::Mut
    } else {
        mir::Mutability::Not
    }
}

pub(crate) fn borrow_kind_from_pair_mutability(pair: &pairs::Mutability<'_>) -> mir::BorrowKind {
    if pair.kw_mut().is_some() {
        mir::BorrowKind::Mut {
            kind: mir::MutBorrowKind::Default,
        }
    } else {
        mir::BorrowKind::Shared
    }
}

pub(crate) fn binop_from_pair(pair: &pairs::MirBinOp<'_>) -> mir::BinOp {
    match pair.deref() {
        Choice15::_0(_kw_add) => mir::BinOp::Add,
        Choice15::_1(_kw_sub) => mir::BinOp::Sub,
        Choice15::_2(_kw_mul) => mir::BinOp::Mul,
        Choice15::_3(_kw_div) => mir::BinOp::Div,
        Choice15::_4(_kw_rem) => mir::BinOp::Rem,
        Choice15::_5(_kw_lt) => mir::BinOp::Lt,
        Choice15::_6(_kw_le) => mir::BinOp::Le,
        Choice15::_7(_kw_gt) => mir::BinOp::Gt,
        Choice15::_8(_kw_ge) => mir::BinOp::Ge,
        Choice15::_9(_kw_eq) => mir::BinOp::Eq,
        Choice15::_10(_kw_ne) => mir::BinOp::Ne,
        Choice15::_11(_kw_bit_and) => mir::BinOp::BitAnd,
        Choice15::_12(_kw_bit_or) => mir::BinOp::BitOr,
        Choice15::_13(_kw_bit_xor) => mir::BinOp::BitXor,
        Choice15::_14(_kw_offset) => mir::BinOp::Offset,
    }
}

pub(crate) fn unop_from_pair(pair: &pairs::MirUnOp<'_>) -> mir::UnOp {
    match pair.deref() {
        Choice3::_0(_kw_neg) => mir::UnOp::Neg,
        Choice3::_1(_kw_not) => mir::UnOp::Not,
        Choice3::_2(_kw_ptr_metadata) => mir::UnOp::PtrMetadata,
    }
}

pub(crate) fn collect_operands<'pcx>(
    operands: Option<WithPath<'pcx, &'pcx pairs::MirOperandsSeparatedByComma<'pcx>>>,
    pcx: PatCtxt<'pcx>,
    fn_sym_tab: &'pcx FnSymbolTable<'pcx>,
) -> Vec<Operand<'pcx>> {
    if let Some(operands) = operands {
        let p = operands.path;
        collect_elems_separated_by_comma!(operands)
            .map(|operand| Operand::from(with_path(p, operand), pcx, fn_sym_tab))
            .collect()
    } else {
        vec![]
    }
}
