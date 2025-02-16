use parser::generics::Choice2;
use parser::pairs;
use pest_typed::Span;

#[derive(Copy, Clone, Debug)]
pub struct Ident<'i> {
    pub name: &'i str,
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

impl<'i> From<&pairs::PathLeading<'i>> for Ident<'i> {
    fn from(leading: &pairs::PathLeading<'i>) -> Self {
        let (name, _) = leading.get_matched();
        let name = if name.is_some() { "crate" } else { "" };
        let span = leading.span;
        Self { name, span }
    }
}

impl<'i> From<&pairs::PathSegment<'i>> for Ident<'i> {
    fn from(segment: &pairs::PathSegment<'i>) -> Self {
        let (name, _) = segment.get_matched();
        match name {
            Choice2::_0(ident) => Ident::from(ident),
            Choice2::_1(meta) => Ident::from(meta),
        }
    }
}

impl<'i> From<&pairs::Identifier<'i>> for Ident<'i> {
    fn from(ident: &pairs::Identifier<'i>) -> Self {
        let span = ident.span;
        let name = span.as_str();
        Self { name, span }
    }
}

impl<'i> From<&pairs::MetaVariable<'i>> for Ident<'i> {
    fn from(meta: &pairs::MetaVariable<'i>) -> Self {
        let span = meta.span;
        let name = span.as_str();
        Self { name, span }
    }
}

pub struct Path<'i> {
    pub leading: Option<&'i pairs::PathLeading<'i>>,
    pub segments: Vec<&'i pairs::PathSegment<'i>>,
    pub _span: Span<'i>,
}

impl<'i> From<&'i pairs::Path<'i>> for Path<'i> {
    fn from(path: &'i pairs::Path<'i>) -> Self {
        let (leading, seg, segs) = path.get_matched();
        let mut segments = vec![seg];
        segs.iter_matched().for_each(|seg| {
            let (_, seg) = seg.get_matched();
            segments.push(seg);
        });
        let span = path.span;
        Self {
            leading: leading.as_ref(),
            segments,
            _span: span,
        }
    }
}

impl<'i> Path<'i> {
    pub fn as_ident(&self) -> Option<Ident<'i>> {
        if self.leading.is_none() && self.segments.len() == 1 {
            Some(Ident::from(self.segments[0]))
        } else {
            None
        }
    }
    pub fn ident(&self) -> Option<Ident<'i>> {
        let last = self.segments.last()?;
        Some(Ident::from(*last))
    }
}

#[macro_export]
macro_rules! collect_elems_separated_by_comma {
    ($decls:expr) => {{
        let (first, following, _) = $decls.get_matched();
        let following = following
            .iter_matched()
            .map(|comma_with_elem| comma_with_elem.get_matched().1);
        std::iter::once(first).chain(following)
    }};
}
