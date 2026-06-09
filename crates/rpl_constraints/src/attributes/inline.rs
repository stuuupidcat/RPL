use rustc_hir::Attribute;
use rustc_hir::attrs::{AttributeKind, InlineAttr};
use rustc_span::Span;

#[derive(Debug, Clone, Copy)]
pub enum Inline {
    /// `#[inline]`
    Normal,
    /// `#[inline(always)]`
    Always,
    /// `#[inline(never)]`
    Never,
    /// Not [`Inline::Never`]
    Any,
}

impl Inline {
    /// `#[inline]` is now a *parsed* attribute (`AttributeKind::Inline(InlineAttr, span)`), so it
    /// is no longer visible as an unparsed/name-based attribute. We read the level from the
    /// parsed `InlineAttr` and return the span carried by the `AttributeKind` —
    /// `Attribute::span()` panics on parsed attributes, so we must NOT use it.
    #[instrument(level = "debug", skip(attr), ret)]
    pub fn check<'tcx>(self, mut attr: impl Iterator<Item = &'tcx Attribute>) -> Option<Span> {
        attr.find_map(|attr| {
            let Attribute::Parsed(AttributeKind::Inline(inline_attr, span)) = attr else {
                return None;
            };
            match self {
                // `#[inline]` (plain) is `InlineAttr::Hint`; the broad "any inline attr" case is
                // `Inline::Any` (Hint | Always). Matching Always/Never here too would make `Normal`
                // over-match patterns that ask for exactly `#[inline]`.
                Inline::Normal => matches!(inline_attr, InlineAttr::Hint),
                Inline::Always => matches!(inline_attr, InlineAttr::Always),
                Inline::Never => matches!(inline_attr, InlineAttr::Never),
                Inline::Any => matches!(inline_attr, InlineAttr::Hint | InlineAttr::Always),
            }
            .then_some(*span)
        })
    }
}
