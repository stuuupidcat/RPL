use parser::{generics, pairs, rules, Grammar, PositionWrapper as Position, Rule, SpanWrapper as Span};
use rustc_hash::FxHashMap;

pub struct Ident<'i> {
    pub name: &'i str,
    pub span: Span<'i>,
}

/* #[derive(Default)]
pub(crate) struct MetaTable<'i> {
    ty_vars:
}

pub(crate) struct WithMetaTable<'i, T> {
    pub(crate) meta: MetaTable<'i>,
    pub(crate) inner: T,
}


#[derive(Default)]
pub struct SymbolTable<'i> {
    structs: FxHashMap<Ident<'i>, Struct<'i>>,
}
 */
