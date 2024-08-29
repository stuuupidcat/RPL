use syn::parse::{Parse, ParseStream};
use syn::{token, Ident};
use syntax::MirPattern;

mod kw {
    syn::custom_keyword!(mir_pattern);
}

#[allow(dead_code)]
enum Delimiter {
    Brace(token::Brace),
    Paren(token::Paren),
    Bracket(token::Bracket),
}

#[derive(thiserror::Error, Debug)]
enum ParseError {
    #[error("expect `(`, `[`, or `{{")]
    ExpectDelimiter,
}

pub struct MirPatternFn {
    pub vis: syn::Visibility,
    pub(crate) tk_fn: syn::Token![fn],
    pub ident: Ident,
    pub generics: syn::Generics,
    pub(crate) paren: token::Paren,
    pub tcx: Ident,
    pub(crate) tk_colon1: syn::Token![:],
    pub tcx_ty: syn::Type,
    pub(crate) tk_comma1: syn::Token![,],
    pub patterns: Ident,
    pub(crate) tk_colon2: syn::Token![:],
    pub patterns_ty: syn::Type,
    pub(crate) tk_comma2: Option<syn::Token![,]>,
    pub(crate) tk_arrow: syn::Token![->],
    pub ret: syn::Type,
    pub(crate) brace: token::Brace,
    _kw_mir_pattern: kw::mir_pattern,
    _tk_bang: syn::Token![!],
    _delim: Delimiter,
    pub mir_pattern: MirPattern,
    _tk_semi: Option<syn::Token![;]>,
    pub stmts: Vec<syn::Stmt>,
}

impl Parse for MirPatternFn {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        use Delimiter::*;
        let vis = input.parse()?;
        let tk_fn = input.parse()?;
        let ident = input.parse()?;
        let generics = input.parse()?;
        let params;
        let paren = syn::parenthesized!(params in input);
        let tcx = params.parse()?;
        let tk_colon1 = params.parse()?;
        let tcx_ty = params.parse()?;
        let tk_comma1 = params.parse()?;
        let patterns = params.parse()?;
        let tk_colon2 = params.parse()?;
        let patterns_ty = params.parse()?;
        let tk_comma2 = params.parse()?;
        drop(params);
        let tk_arrow = input.parse()?;
        let ret = input.parse()?;
        let body;
        let brace = syn::braced!(body in input);
        let kw_mir_pattern = body.parse()?;
        let tk_bang = body.parse()?;
        let pattern;
        let delim = if body.peek(token::Paren) {
            Paren(syn::parenthesized!(pattern in body))
        } else if body.peek(token::Brace) {
            Brace(syn::braced!(pattern in body))
        } else if body.peek(token::Bracket) {
            Bracket(syn::bracketed!(pattern in body))
        } else {
            return Err(body.error(ParseError::ExpectDelimiter));
        };
        let mir_pattern = pattern.parse()?;
        let tk_semi = match delim {
            Paren(_) | Bracket(_) => Some(body.parse()?),
            Brace(_) => body.parse()?,
        };
        let stmts = body.call(syn::Block::parse_within)?;
        Ok(MirPatternFn {
            vis,
            tk_fn,
            ident,
            generics,
            paren,
            tcx,
            tk_colon1,
            tcx_ty,
            tk_comma1,
            patterns,
            tk_colon2,
            patterns_ty,
            tk_comma2,
            tk_arrow,
            ret,
            brace,
            _kw_mir_pattern: kw_mir_pattern,
            _tk_bang: tk_bang,
            _delim: delim,
            mir_pattern,
            _tk_semi: tk_semi,
            stmts,
        })
    }
}
