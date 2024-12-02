#![feature(rustc_private)]

use proc_macro::TokenStream;

extern crate rpl_pat_expand as expand;
extern crate rpl_pat_syntax as syntax;

#[proc_macro_attribute]
pub fn mir_pattern(_attribute: TokenStream, input: TokenStream) -> TokenStream {
    let expanded = expand::expand(syn::parse_macro_input!(input as expand::MirPatternFn));
    expanded.into()
}

#[proc_macro]
pub fn mir(input: TokenStream) -> TokenStream {
    expand::expand_mir(syn::parse_macro_input!(input as syntax::Mir), None)
        .map_or_else(|err| err.into_compile_error().into(), Into::into)
}

#[proc_macro]
pub fn identity(input: TokenStream) -> TokenStream {
    input
}
