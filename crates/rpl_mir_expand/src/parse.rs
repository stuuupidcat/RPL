use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};

const MACRO_MIR: &str = "mir";

pub struct MirPatternFn {
    pub(crate) item_fn: syn::ItemFn,
}

impl Parse for MirPatternFn {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let item_fn = input.parse()?;
        Ok(MirPatternFn { item_fn })
    }
}

impl MirPatternFn {
    pub(crate) fn expand_macro_mir(mut self) -> syn::Result<TokenStream> {
        let inputs = self.item_fn.sig.inputs.iter().collect::<Vec<_>>();
        let tcx_and_patterns = if let [syn::FnArg::Typed(tcx), syn::FnArg::Typed(patterns)] = inputs[..]
            && let box syn::Pat::Ident(ref tcx) = tcx.pat
            && let box syn::Pat::Ident(ref patterns) = patterns.pat
        {
            Some((&tcx.ident, &patterns.ident))
        } else {
            None
        };
        for stmt in &mut self.item_fn.block.stmts {
            if let syn::Stmt::Macro(syn::StmtMacro { mac, .. }) = stmt
                && mac.path.is_ident(MACRO_MIR)
            {
                mac.path = syn::parse_quote!(::rpl_macros::identity);
                let mir = syn::parse2(mac.tokens.clone())?;
                mac.tokens = crate::expand_mir(mir, tcx_and_patterns)?;
            }
        }
        Ok(self.item_fn.into_token_stream())
    }
}
