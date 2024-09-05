use syn::parse::{Parse, ParseStream};
use syn::Ident;
use syntax::Mir;

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
    pub(crate) fn tcx_and_patterns(&self) -> Option<(&Ident, &Ident)> {
        let inputs = self.item_fn.sig.inputs.iter().collect::<Vec<_>>();
        if let [syn::FnArg::Typed(tcx), syn::FnArg::Typed(patterns)] = inputs[..]
            && let box syn::Pat::Ident(ref tcx) = tcx.pat
            && let box syn::Pat::Ident(ref patterns) = patterns.pat
        {
            return Some((&tcx.ident, &patterns.ident));
        }
        None
    }

    pub(crate) fn stmt_is_macro_mir(&self, stmt: &syn::Stmt) -> syn::Result<Option<Mir>> {
        if let syn::Stmt::Macro(syn::StmtMacro { mac, .. }) = stmt
            && mac.path.is_ident(MACRO_MIR)
        {
            return syn::parse2(mac.tokens.clone()).map(Some);
        }
        Ok(None)
    }
}
