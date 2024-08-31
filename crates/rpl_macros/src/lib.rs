use proc_macro::TokenStream;
use quote::ToTokens;

extern crate rpl_mir_expand as expand;
extern crate rpl_mir_syntax as syntax;

#[proc_macro_attribute]
pub fn mir_pattern(_attribute: TokenStream, input: TokenStream) -> TokenStream {
    let expanded = syn::parse_macro_input!(input as expand::MirPatternFn).into_token_stream();
    expanded.into()
}
