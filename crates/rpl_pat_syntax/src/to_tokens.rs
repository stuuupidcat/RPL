use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::*;

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

impl ToTokens for TypeGroup {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.tk_group.surround(tokens, |tokens| self.ty.to_tokens(tokens));
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

impl ToTokens for RvalueUse {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner = |tokens: &mut _| self.operand.to_tokens(tokens);
        match self.paren {
            Some(paren) => paren.surround(tokens, inner),
            None => inner(tokens),
        }
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
