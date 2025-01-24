use crate::error::{RPLMetaError, RPLMetaResult};
use derive_more::derive::From;
use parser::generics::Choice2;
use parser::{generics, pairs, rules, span, Grammar, PositionWrapper as Position, Rule, SpanWrapper};
use pest_typed::Span;
use rustc_hash::FxHashMap;
use rustc_span::Symbol;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Ident<'a> {
    pub name: Symbol,
    pub span: Span<'a>,
}

impl<'a, T> From<&T> for Ident<'a>
where
    T: Into<Ident<'a>>,
{
    fn from(pairs_ref: &T) -> Self {
        pairs_ref.into()
    }
}

impl<'a> From<pairs::PathLeading<'a>> for Ident<'a> {
    fn from(leading: pairs::PathLeading<'a>) -> Self {
        let (name, _) = leading.get_matched();
        let name = if name.is_some() {
            Symbol::intern("crate")
        } else {
            Symbol::intern("")
        };
        let span = leading.span;
        Self { name, span }
    }
}

impl<'a> From<pairs::PathSegment<'a>> for Ident<'a> {
    fn from(segment: pairs::PathSegment<'a>) -> Self {
        let (name, _) = segment.get_matched();
        match name {
            Choice2::_0(ident) => Ident::from(ident),
            Choice2::_1(meta) => Ident::from(meta),
        }
    }
}

impl<'a> From<pairs::Identifier<'a>> for Ident<'a> {
    fn from(ident: pairs::Identifier<'a>) -> Self {
        let span = ident.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

impl<'a> From<pairs::MetaVariable<'a>> for Ident<'a> {
    fn from(meta: pairs::MetaVariable<'a>) -> Self {
        let span = meta.span;
        let name = Symbol::intern(span.as_str());
        Self { name, span }
    }
}

pub struct Path<'a> {
    pub segments: Vec<Ident<'a>>,
    pub span: Span<'a>,
}

impl<'a, T> From<&T> for Path<'a>
where
    T: Into<Path<'a>>,
{
    fn from(pairs_ref: &T) -> Self {
        pairs_ref.into()
    }
}

impl<'a> From<pairs::Path<'a>> for Path<'a> {
    fn from(path: pairs::Path<'a>) -> Self {
        let (leading, seg, segs) = path.get_matched();
        let mut segments = vec![];
        if let Some(leading) = leading {
            segments.push(Ident::from(leading));
        }
        segments.push(Ident::from(seg));
        segs.iter_matched().for_each(|seg| {
            let (_, seg) = seg.get_matched();
            segments.push(Ident::from(seg));
        });
        let span = path.span;
        Self { segments, span }
    }
}
