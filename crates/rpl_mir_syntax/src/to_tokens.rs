use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::*;

macro_rules! ToTokens {
    (
        enum $ident:ident {
            $( $( #[$variant_meta:meta] )* $variant:ident $( ($ty:ty) )? ),* $(,)?
        }
    ) => {
        impl ::quote::ToTokens for $ident {
            fn to_tokens(&self, tokens: &mut ::proc_macro2::TokenStream) {
                match self {
                    $( $( $ident::$variant(inner) => <$ty as ::quote::ToTokens>::to_tokens(inner, tokens), )? )*
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
            }
        }
    };

    (
        struct $ident:ident {
            $( $field_vis:vis $field:ident: $ty:ty, )*
        }
    ) => {
        impl ::quote::ToTokens for $ident {
            fn to_tokens(&self, tokens: &mut ::proc_macro2::TokenStream) {
                $( ::quote::ToTokens::to_tokens(&self.$field, tokens); )*
            }
        }
    };
}

impl AsRef<str> for Region {
    fn as_ref(&self) -> &str {
        match self.kind {
            RegionKind::ReAny(_) => "'_",
            RegionKind::ReStatic(_) => "'static",
        }
    }
}

impl ToTokens for Region {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        syn::Lifetime::new(self.as_ref(), self.span).to_tokens(tokens);
    }
}

impl ToTokens for GenericConst {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner = |tokens: &mut _| self.konst.to_tokens(tokens);
        match self.brace {
            Some(brace) => brace.surround(tokens, inner),
            None => inner(tokens),
        }
    }
}

impl ToTokens for TypeArray {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.bracket.surround(tokens, |tokens| {
            self.ty.to_tokens(tokens);
            self.tk_semi.to_tokens(tokens);
            self.len.to_tokens(tokens);
        });
    }
}

impl ToTokens for TypeSlice {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.bracket.surround(tokens, |tokens| self.ty.to_tokens(tokens));
    }
}

impl ToTokens for TypeGroup {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.tk_group.surround(tokens, |tokens| self.ty.to_tokens(tokens));
    }
}

impl ToTokens for TypeParen {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.paren.surround(tokens, |tokens| self.ty.to_tokens(tokens));
    }
}

impl ToTokens for TypeTuple {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.paren.surround(tokens, |tokens| self.tys.to_tokens(tokens));
    }
}

impl ToTokens for ReturnType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ReturnType::Default => {},
            ReturnType::Type(tk_rarrow, ty) => {
                tk_rarrow.to_tokens(tokens);
                ty.to_tokens(tokens);
            },
        }
    }
}

impl ToTokens for ParenthesizedGenericArguments {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.paren.surround(tokens, |tokens| self.inputs.to_tokens(tokens));
        self.output.to_tokens(tokens);
    }
}

impl ToTokens for TypePath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut pairs = self.path.segments.pairs();
        if let Some(qself) = &self.qself {
            qself.tk_lt.to_tokens(tokens);
            qself.ty.to_tokens(tokens);
            if let Some(tk_as) = qself.tk_as {
                tk_as.to_tokens(tokens);
                self.path.leading.to_tokens(tokens);
                for (pos, pair) in pairs.by_ref().take(qself.position).enumerate() {
                    let (seg, tk_colon2) = pair.into_tuple();
                    seg.to_tokens(tokens);
                    if pos + 1 >= qself.position {
                        qself.tk_gt.to_tokens(tokens);
                    }
                    tk_colon2.to_tokens(tokens);
                }
            } else {
                qself.tk_gt.to_tokens(tokens);
                self.path.leading.to_tokens(tokens);
            }
        } else {
            self.path.leading.to_tokens(tokens);
        }
        for pair in pairs {
            let (seg, tk_colon2) = pair.into_tuple();
            seg.to_tokens(tokens);
            tk_colon2.to_tokens(tokens);
        }
    }
}

impl ToTokens for PlaceParen {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.paren.surround(tokens, |tokens| self.place.to_tokens(tokens));
    }
}

impl ToTokens for PlaceIndex {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.place.to_tokens(tokens);
        self.bracket.surround(tokens, |tokens| self.index.to_tokens(tokens));
    }
}

impl ToTokens for PlaceConstIndex {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.place.to_tokens(tokens);
        self.bracket.surround(tokens, |tokens| {
            self.from_end.to_tokens(tokens);
            self.index.to_tokens(tokens);
            self.kw_of.to_tokens(tokens);
            self.min_length.to_tokens(tokens);
        });
    }
}

impl ToTokens for PlaceSubslice {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.place.to_tokens(tokens);
        self.bracket.surround(tokens, |tokens| {
            self.from.to_tokens(tokens);
            self.tk_colon.to_tokens(tokens);
            self.from_end.to_tokens(tokens);
            self.to.to_tokens(tokens);
        });
    }
}

impl ToTokens for RvalueUse {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner = |tokens: &mut _| self.operand.to_tokens(tokens);
        match self.paren {
            Some(paren) => paren.surround(tokens, inner),
            None => inner(tokens),
        }
    }
}

impl ToTokens for RvalueRepeat {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.bracket.surround(tokens, |tokens| {
            self.operand.to_tokens(tokens);
            self.tk_semi.to_tokens(tokens);
            self.len.to_tokens(tokens);
        })
    }
}

impl ToTokens for RvalueLen {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.kw_len.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.place.to_tokens(tokens));
    }
}

impl ToTokens for RvalueCast {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.operand.to_tokens(tokens);
        self.tk_as.to_tokens(tokens);
        self.ty.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.cast_kind.to_tokens(tokens));
    }
}

impl ToTokens for RvalueBinOp {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.op.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| {
            self.lhs.to_tokens(tokens);
            self.tk_comma.to_tokens(tokens);
            self.rhs.to_tokens(tokens);
        });
    }
}

impl ToTokens for RvalueNullOp {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.op.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.ty.to_tokens(tokens));
    }
}

impl ToTokens for RvalueUnOp {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.op.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.operand.to_tokens(tokens));
    }
}

impl ToTokens for RvalueDiscriminant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.kw_discr.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.place.to_tokens(tokens));
    }
}

impl ToTokens for AggregateArray {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.bracket.surround(tokens, |tokens| {
            self.ty.to_tokens(tokens);
            self.tk_semi.to_tokens(tokens);
            self.tk_underscore.to_tokens(tokens);
        });
        self.kw_from.to_tokens(tokens);
        self.operands.to_tokens(tokens);
    }
}

impl ToTokens for StructFields {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.brace.surround(tokens, |tokens| self.fields.to_tokens(tokens));
    }
}

impl ToTokens for AggregateRawPtr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ty.to_tokens(tokens);
        self.kw_from.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| {
            self.ptr.to_tokens(tokens);
            self.tk_comma.to_tokens(tokens);
            self.metadata.to_tokens(tokens);
        });
    }
}

impl ToTokens for ParenthesizedOperands {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.paren.surround(tokens, |tokens| self.operands.to_tokens(tokens));
    }
}

impl ToTokens for BracketedOperands {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.bracket.surround(tokens, |tokens| self.operands.to_tokens(tokens));
    }
}

impl ToTokens for BracedOperands {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.brace.surround(tokens, |tokens| {
            self.operands.to_tokens(tokens);
            self.tk_dotdot.to_tokens(tokens);
        });
    }
}

impl ToTokens for Drop {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.kw_drop.to_tokens(tokens);
        self.paren.surround(tokens, |tokens| self.place.to_tokens(tokens));
        self.tk_semi.to_tokens(tokens);
    }
}

impl<K: ToTokens, C: ToTokens, P> ToTokens for Macro<K, C, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.kw.to_tokens(tokens);
        self.tk_bang.to_tokens(tokens);
        let inner = |tokens: &mut _| self.content.to_tokens(tokens);
        match self.delim {
            syn::MacroDelimiter::Paren(paren) => paren.surround(tokens, inner),
            syn::MacroDelimiter::Brace(brace) => brace.surround(tokens, inner),
            syn::MacroDelimiter::Bracket(bracket) => bracket.surround(tokens, inner),
        }
    }
}

impl ToTokens for Mir {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for meta in &self.metas {
            meta.to_tokens(tokens);
        }
        for statement in &self.statements {
            statement.to_tokens(tokens);
        }
    }
}
