use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_each_token, quote_token, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::{Pair, Pairs, Punctuated};
use syn::Ident;
use syntax::*;

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
        let patterns = if let [syn::FnArg::Typed(patterns)] = inputs[..]
            && let box syn::Pat::Ident(ref patterns) = patterns.pat
        {
            Some(&patterns.ident)
        } else {
            None
        };
        for stmt in &mut self.item_fn.block.stmts {
            if let syn::Stmt::Macro(syn::StmtMacro { mac, .. }) = stmt
                && mac.path.is_ident(MACRO_MIR)
            {
                mac.path = syn::parse_quote!(::rpl_macros::identity);
                let mir = syn::parse2(mac.tokens.clone())?;
                mac.tokens = crate::expand_mir(mir, patterns)?;
            }
        }
        Ok(self.item_fn.into_token_stream())
    }
}

pub(crate) struct ExpandCtxt<'ecx> {
    patterns: &'ecx Ident,
}

pub(crate) fn expand_impl<T>(value: T, patterns: &Ident, tokens: &mut TokenStream)
where
    for<'ecx> Expand<'ecx, T>: ToTokens,
{
    let ecx = ExpandCtxt::new(patterns);
    ecx.expand(value).to_tokens(tokens);
}

pub fn expand(mir_pattern: MirPatternFn) -> TokenStream {
    mir_pattern
        .expand_macro_mir()
        .unwrap_or_else(syn::Error::into_compile_error)
}

static PARAM_PATTERNS: &str = "patterns";

pub fn expand_mir(mir: Mir, tcx_and_patterns: Option<&Ident>) -> syn::Result<TokenStream> {
    crate::check_mir(&mir)?;
    let patterns = match tcx_and_patterns {
        None => &syn::Ident::new(PARAM_PATTERNS, proc_macro2::Span::call_site()),
        Some(patterns) => patterns,
    };
    let mut tokens = TokenStream::new();
    expand_impl(&mir, patterns, &mut tokens);
    Ok(tokens)
}

trait ToSymbol: Sized + ToString {
    fn to_symbol(&self) -> IdentSymbol {
        IdentSymbol(self.to_string())
    }
}

impl<S: ToString> ToSymbol for S {}

trait ExpandIdent: Sized {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident;
    // fn with_span(self, span: Span) -> Ident {
    //     let mut ident = self.into();
    //     ident.set_span(span);
    //     ident
    // }
    fn as_ty(&self) -> Ident {
        self.with_suffix("_ty")
    }
    fn as_ty_var(&self) -> Ident {
        self.with_suffix("_ty_var")
    }
    fn as_local(&self) -> Ident {
        self.with_suffix("_local")
    }
    fn as_stmt(&self) -> Ident {
        self.with_suffix("_stmt")
    }
}

impl ExpandIdent for &'static str {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident {
        format_ident!("{self}{suffix}")
    }
}
impl ExpandIdent for syn::Token![self] {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident {
        format_ident!("self{suffix}")
    }
}
impl ExpandIdent for Ident {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident {
        format_ident!("{self}{suffix}")
    }
}
impl ExpandIdent for PlaceLocal {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident {
        match self {
            PlaceLocal::Local(ident) => ident.with_suffix(suffix),
            PlaceLocal::SelfValue(self_value) => self_value.with_suffix(suffix),
        }
    }
}

impl<'ecx> ExpandCtxt<'ecx> {
    pub(crate) fn new(patterns: &'ecx Ident) -> Self {
        Self { patterns }
    }
    pub(crate) fn expand<T>(&self, value: T) -> Expand<'_, T> {
        Expand { value, ecx: self }
    }
    fn expand_projections<'a>(&self, place: &'a Place) -> Expand<'_, Projections<'a>> {
        Expand {
            value: Projections(place),
            ecx: self,
        }
    }
    fn expand_punctuated<'a, U: 'a, P: 'a>(
        &'ecx self,
        value: &'a Punctuated<U, P>,
    ) -> ExpandPunctPairs<'ecx, 'a, U, P> {
        self.expand_with(value, Punctuated::pairs)
    }
    #[allow(clippy::type_complexity)] // FIXME: return type too complex
    fn expand_punctuated_mapped<'a, U: 'a, V: 'a, P: 'a>(
        &'ecx self,
        value: &'a Punctuated<U, P>,
        f: fn(&'a U) -> V,
    ) -> ExpandPunct<'ecx, 'a, U, P, impl Fn(&'a Punctuated<U, P>) -> impl IntoIterator<Item = Pair<V, &'a P>>> {
        self.expand_with(value, move |value| value.pairs().map_value(f))
    }
    fn expand_with<'a, U: 'a, I, F: Fn(&'a U) -> I>(&'ecx self, value: &'a U, f: F) -> ExpandWith<'ecx, &'a U, F> {
        ExpandWith { value, f, ecx: self }
    }
}

type ExpandPunct<'ecx, 'a, U, P, F> = ExpandWith<'ecx, &'a Punctuated<U, P>, F>;
type ExpandPunctPairs<'ecx, 'a, U, P> = ExpandPunct<'ecx, 'a, U, P, fn(&'a Punctuated<U, P>) -> Pairs<'a, U, P>>;

pub(crate) struct Expand<'ecx, T> {
    value: T,
    ecx: &'ecx ExpandCtxt<'ecx>,
}

struct ExpandWith<'ecx, T, F> {
    value: T,
    f: F,
    ecx: &'ecx ExpandCtxt<'ecx>,
}

impl<'ecx, T> std::ops::Deref for Expand<'ecx, T> {
    type Target = ExpandCtxt<'ecx>;

    fn deref(&self) -> &Self::Target {
        self.ecx
    }
}

impl<'ecx, T, F> std::ops::Deref for ExpandWith<'ecx, T, F> {
    type Target = ExpandCtxt<'ecx>;

    fn deref(&self) -> &Self::Target {
        self.ecx
    }
}

impl<'a, T: 'a, I, F, U: 'a, P: 'a> ToTokens for ExpandWith<'_, &'a T, F>
where
    F: Fn(&'a T) -> I,
    I: IntoIterator<Item = Pair<U, &'a P>>,
    for<'ecx> Expand<'ecx, U>: ToTokens,
    P: ToTokens,
{
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        for pair in (self.f)(self.value) {
            let (value, punct) = pair.into_tuple();
            let value = self.expand(value);
            quote_each_token!(tokens #value #punct);
        }
    }
}

trait PairsExt<'a, T: 'a, P: 'a> {
    fn map_value<U>(self, f: impl FnMut(&'a T) -> U) -> impl Iterator<Item = Pair<U, &'a P>>;
}

impl<'a, T: 'a, P: 'a> PairsExt<'a, T, P> for Pairs<'a, T, P> {
    fn map_value<U>(self, mut f: impl FnMut(&'a T) -> U) -> impl Iterator<Item = Pair<U, &'a P>> {
        self.map(move |pair| {
            let (value, punct) = pair.into_tuple();
            Pair::new(f(value), punct)
        })
    }
}

impl ToTokens for Expand<'_, &Mir> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let Mir {
            metas,
            declarations,
            statements,
        } = &self.value;
        let metas = metas.iter().map(|meta| self.expand(meta));
        let declarations = declarations.iter().map(|declaration| self.expand(declaration));
        let statements = statements.iter().map(|statement| self.expand(statement));
        quote_each_token!(tokens #(#metas)* #(#declarations)* #(#statements)*);
    }
}

impl ToTokens for Expand<'_, &MetaItem> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let MetaItem { ident, kind, .. } = &self.value;
        match kind {
            MetaKind::Ty(_) => {
                let ty_ident = ident.as_ty();
                let ty_var_ident = ident.as_ty_var();
                quote_each_token!(tokens
                    #[allow(non_snake_case)]
                    let #ty_var_ident = #patterns.new_ty_var();
                    #[allow(non_snake_case)]
                    let #ty_ident = #patterns.pcx.mk_var_ty(#ty_var_ident);
                );
            },
        }
    }
}

impl ToTokens for Expand<'_, &Meta> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for meta_item in self.value.meta.content.iter() {
            self.expand(meta_item).to_tokens(tokens);
        }
    }
}

impl ToTokens for Expand<'_, &Declaration> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            Declaration::TypeDecl(type_decl) => self.expand(type_decl).to_tokens(tokens),
            Declaration::UsePath(use_path) => self.expand(use_path).to_tokens(tokens),
            Declaration::LocalDecl(local_decl) => self.expand(local_decl).to_tokens(tokens),
        }
    }
}

impl<End: Parse + ToTokens> ToTokens for Expand<'_, &Statement<End>> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            Statement::Assign(assign, _) => self.expand(assign).to_tokens(tokens),
            Statement::Call(call, _) => self.expand(call).to_tokens(tokens),
            Statement::Drop(drop, _) => self.expand(drop).to_tokens(tokens),
            Statement::Control(control, _) => self.expand(control).to_tokens(tokens),
            Statement::Loop(loop_) => self.expand(loop_).to_tokens(tokens),
            Statement::SwitchInt(switch_int) => self.expand(switch_int).to_tokens(tokens),
        }
    }
}

impl ToTokens for Expand<'_, &CallIgnoreRet> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let Call { func, operands } = &self.value.call;
        let func = self.expand(func);
        let operands = self.expand_punctuated(&operands.value);
        quote_each_token!(tokens #patterns.mk_fn_call(#func, #patterns.mk_list([#operands]), None); );
    }
}

impl ToTokens for Expand<'_, &Control> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        match self.value {
            Control::Break(_, _) => {
                quote_each_token!(tokens #patterns.mk_break(););
            },
            Control::Continue(_, _) => {
                quote_each_token!(tokens #patterns.mk_continue(););
            },
        }
    }
}

impl ToTokens for Expand<'_, &Loop> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let statements = self
            .value
            .block
            .statements
            .iter()
            .map(|statement| self.expand(statement));
        quote_each_token!(tokens #patterns.mk_loop(|#patterns| { #(#statements)* }););
    }
}

impl ToTokens for Expand<'_, &SwitchInt> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let operand = self.expand(&self.value.operand);
        let targets = self.value.targets.iter().map(|target| self.expand(target));
        quote_each_token!(tokens #patterns.mk_switch_int(#operand, |mut #patterns| { #(#targets)* }););
    }
}

impl ToTokens for Expand<'_, &SwitchTarget> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let SwitchTarget { value, body, .. } = self.value;
        let body = self.expand(body);
        let value = match value {
            SwitchValue::Bool(lit_bool) => Some(quote!(#lit_bool)),
            SwitchValue::Int(lit_int) => Some(quote!(#lit_int)),
            SwitchValue::Underscore(_) => None,
        };
        let body = quote!(|#patterns| { #body });
        match value {
            Some(value) => {
                quote_each_token!(tokens #patterns.mk_switch_target(#value, #body););
            },
            None => {
                quote_each_token!(tokens #patterns.mk_otherwise(#body););
            },
        }
    }
}

impl ToTokens for Expand<'_, &SwitchBody> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            SwitchBody::Statement(statement, _) => self.expand(statement).to_tokens(tokens),
            SwitchBody::Block(block) => self.expand(block).to_tokens(tokens),
        }
    }
}
impl ToTokens for Expand<'_, &Block> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for statement in &self.value.statements {
            self.expand(statement).to_tokens(tokens);
        }
    }
}

impl ToTokens for Expand<'_, &TypeDecl> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let TypeDecl { ident, ty, .. } = self.value;
        let ty_ident = ident.as_ty();
        let ty = self.expand(ty);
        quote_each_token!(tokens #[allow(non_snake_case)] let #ty_ident = #ty;);
    }
}
impl ToTokens for Expand<'_, &UsePath> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}

impl ToTokens for Expand<'_, &LocalDecl> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let LocalDecl { local, ty, init, .. } = self.value;
        let ExpandCtxt { patterns, .. } = self.ecx;
        let ty = self.expand(ty);
        let ident = local.as_local();
        let mk_local_or_self = match local {
            PlaceLocal::Local(_) => format_ident!("mk_local"),
            PlaceLocal::SelfValue(_) => format_ident!("mk_self"),
        };
        quote_each_token!(tokens let #ident = #patterns.#mk_local_or_self(#ty); );
        if let Some(LocalInit { rvalue_or_call, .. }) = init {
            self.expand(Assign {
                place: &Place::Local(local.clone()),
                rvalue_or_call,
            })
            .to_tokens(tokens);
        }
    }
}

struct Assign<'a> {
    place: &'a Place,
    rvalue_or_call: &'a RvalueOrCall,
}

impl ToTokens for Expand<'_, &syntax::Assign> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.expand(Assign {
            place: &self.value.place,
            rvalue_or_call: &self.value.rvalue_or_call,
        })
        .to_tokens(tokens);
    }
}

impl ToTokens for Expand<'_, Assign<'_>> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let Assign { place, rvalue_or_call } = self.value;
        let local = place.local();
        let stmt = local.as_stmt();
        quote_each_token!(tokens let #stmt = );
        let place = self.expand(place);
        match rvalue_or_call {
            RvalueOrCall::Rvalue(rvalue) => {
                let rvalue = self.expand(rvalue);
                quote_each_token!(tokens #patterns.mk_assign(#place, #rvalue); );
            },
            RvalueOrCall::Call(Call { func, operands }) => {
                let func = self.expand(func);
                let operands = self.expand_punctuated(&operands.value);
                quote_each_token!(tokens #patterns.mk_fn_call(#func, #patterns.mk_list([#operands]), Some(#place)); );
            },
        }
    }
}

impl ToTokens for Expand<'_, &Drop> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns, .. } = self.ecx;
        let Drop { ref place, .. } = self.value;
        let stmt = place.local().as_stmt();
        quote_each_token!(tokens let #stmt = #patterns.mk_drop(#place); );
    }
}

trait RegionExt {
    fn kind(&self) -> RegionKind;
}

impl RegionExt for Option<Region> {
    fn kind(&self) -> RegionKind {
        self.map_or(RegionKind::ReAny(Default::default()), |region| region.kind)
    }
}

impl ToTokens for Expand<'_, &Path> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let mut iter = self.value.segments.iter();
        let mut path = TokenStream::new();
        if let PathLeading::Crate(_) = self.value.leading {
            quote_each_token!(path "crate",);
        }
        let mut gen_args = TokenStream::new();
        for segment in iter.by_ref() {
            let ident = segment.ident.to_string();
            quote_each_token!(path #ident,);
            if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = &segment.arguments {
                let args = self.expand_punctuated(args);
                quote_each_token!(gen_args #args);
            }
        }
        quote_each_token!(tokens #patterns.pcx.mk_path_with_args(
            #patterns.pcx.mk_item_path(&[#path]), &[#gen_args]
        ));
        if let Some(_rem) = iter.next() {
            todo!();
        }
    }
}

impl ToTokens for Expand<'_, &GenericArgument> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        match self.value {
            GenericArgument::Region(Region { kind, .. }) => self.expand(*kind).to_tokens(tokens),
            GenericArgument::Type(ty) => self.expand(ty).to_tokens(tokens),
            GenericArgument::Const(GenericConst { konst, .. }) => self.expand(konst).to_tokens(tokens),
        }
        quote_each_token!(tokens.into());
    }
}

impl ToTokens for Expand<'_, &Type> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        match self.value {
            Type::Array(TypeArray { box ty, len, .. }) => {
                let ty = self.expand(ty);
                quote_each_token!(tokens #patterns.pcx.mk_array_ty(#ty, #len));
            },
            Type::Group(TypeGroup { box ty, .. }) | Type::Paren(TypeParen { value: box ty, .. }) => {
                self.expand(ty).to_tokens(tokens);
            },
            Type::Never(_) => todo!(),
            Type::Path(TypePath { qself: None, path }) if let Some(ident) = path.as_ident() => {
                if crate::is_primitive(ident) {
                    quote_each_token!(tokens #patterns.pcx.primitive_types.#ident);
                } else {
                    ident.as_ty().to_tokens(tokens);
                }
            },
            Type::Path(TypePath { path, .. }) => {
                let path = self.expand(path);
                quote_each_token!(tokens #patterns.pcx.mk_path_ty(#path));
            },
            Type::Ptr(TypePtr { mutability, box ty, .. }) => {
                let ty = self.expand(ty);
                let mutability = self.expand(*mutability);
                quote_each_token!(tokens #patterns.pcx.mk_raw_ptr_ty(#ty, #mutability));
            },
            Type::Reference(TypeReference {
                region,
                mutability,
                box ty,
                ..
            }) => {
                let region = self.expand(region.kind());
                let ty = self.expand(ty);
                let mutability = self.expand(*mutability);
                quote_each_token!(tokens #patterns.pcx.mk_ref_ty(#region, #ty, #mutability));
            },
            Type::Slice(TypeSlice { box ty, .. }) => {
                let ty = self.expand(ty);
                quote_each_token!(tokens #patterns.pcx.mk_slice_ty(#ty));
            },
            Type::Tuple(TypeTuple { tys, .. }) => {
                let tys = self.expand_punctuated(tys);
                quote_each_token!(tokens #patterns.pcx.mk_tuple_ty(&[#tys]));
            },
            Type::TyVar(TypeVar { ident, .. }) => ident.as_ty().to_tokens(tokens),
            Type::LangItem(lang_item) => {
                let lang_item = self.expand(lang_item);
                quote_each_token!(tokens #patterns.pcx.mk_adt_ty(#lang_item));
            },
        }
    }
}

impl ToTokens for Expand<'_, &LangItemWithArgs> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        let LangItemWithArgs { item, args, .. } = self.value;
        let mut gen_args = TokenStream::new();
        if let Some(args) = args {
            let args = self.expand_punctuated(&args.args);
            quote_each_token!(gen_args #args);
        }
        quote_each_token!(tokens #patterns.pcx.mk_path_with_args(
            #patterns.pcx.mk_lang_item(#item), &[#gen_args]
        ));
    }
}

impl ToTokens for Expand<'_, &Rvalue> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::Rvalue::);
        match self.value {
            Rvalue::Any(_) => {
                quote_each_token!(tokens Any);
            },
            Rvalue::Use(RvalueUse { operand, .. }) => {
                let operand = self.expand(operand);
                quote_each_token!(tokens Use(#operand));
            },
            Rvalue::Repeat(RvalueRepeat { operand, len, .. }) => {
                let operand = self.expand(operand);
                quote_each_token!(tokens Repeat(#operand, #len));
            },
            Rvalue::Ref(RvalueRef {
                region,
                mutability,
                place,
                ..
            }) => {
                let region = self.expand(region.kind());
                let mutability = self.expand(BorrowKind(*mutability));
                let place = self.expand(place);
                quote_each_token!(tokens Ref(#region, #mutability, #place));
            },
            Rvalue::RawPtr(RvalueRawPtr { mutability, place, .. }) => {
                let mutability = self.expand(*mutability);
                let place = self.expand(place);
                quote_each_token!(tokens RawPtr(#mutability, #place));
            },
            Rvalue::Len(RvalueLen { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Len(#place));
            },
            Rvalue::Cast(RvalueCast {
                operand, ty, cast_kind, ..
            }) => {
                let operand = self.expand(operand);
                let ty = self.expand(ty);
                let cast_kind = self.expand(*cast_kind);
                quote_each_token!(tokens Cast(#cast_kind, #operand, #ty));
            },
            Rvalue::BinaryOp(RvalueBinOp { op, lhs, rhs, .. }) => {
                let op = self.expand(*op);
                let lhs = self.expand(lhs);
                let rhs = self.expand(rhs);
                quote_each_token!(tokens BinaryOp(#op, Box::new([#lhs, #rhs])));
            },
            Rvalue::NullaryOp(RvalueNullOp { op, ty, .. }) => {
                let op = self.expand(*op);
                let ty = self.expand(ty);
                quote_each_token!(tokens NullaryOp(#op, #ty));
            },
            Rvalue::UnaryOp(RvalueUnOp { op, operand, .. }) => {
                let op = self.expand(*op);
                let operand = self.expand(operand);
                quote_each_token!(tokens UnaryOp(#op, #operand));
            },
            Rvalue::Discriminant(RvalueDiscriminant { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Discriminant(#place));
            },
            Rvalue::Aggregate(aggregate) => {
                let aggregate = self.expand(aggregate);
                quote_each_token!(tokens Aggregate(#aggregate));
            },
        }
    }
}

impl ToTokens for Expand<'_, &Operand> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::Operand::);
        match self.value {
            Operand::Any(_) => quote_token!(Any tokens),
            Operand::AnyMultiple(_) => todo!(),
            Operand::Copy(OperandCopy { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Copy(#place));
            },
            Operand::Move(OperandMove { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Move(#place));
            },
            Operand::Constant(konst) => {
                let konst = self.expand(&konst.kind);
                quote_each_token!(tokens Constant(#konst));
            },
        }
    }
}

impl ToTokens for Expand<'_, &FnOperand> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        quote_each_token!(tokens ::rpl_mir::pat::Operand::);
        match self.value {
            FnOperand::Copy(Parenthesized {
                value: OperandCopy { place, .. },
                ..
            }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Copy(#place));
            },
            FnOperand::Move(Parenthesized {
                value: OperandMove { place, .. },
                ..
            }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Move(#place));
            },
            FnOperand::Type(TypePath { qself: None, path }) => {
                let path = self.expand(path);
                quote_each_token!(tokens Constant(#patterns.mk_zeroed(#path)));
            },
            FnOperand::Type(TypePath {
                qself: Some(_qself),
                path: _,
            }) => {
                todo!()
            },
            FnOperand::LangItem(lang_item) => {
                let lang_item = self.expand(lang_item);
                quote_each_token!(tokens Constant(#patterns.mk_zeroed(#lang_item)));
            },
        }
    }
}

impl ToTokens for Expand<'_, &ConstOperandKind> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns } = self.ecx;
        match self.value {
            ConstOperandKind::Lit(lit) => self.expand(lit).to_tokens(tokens),
            ConstOperandKind::Type(TypePath { qself: None, path }) => {
                // todo!();
                let path = self.expand(path);
                quote_each_token!(tokens #patterns.mk_zeroed(#path));
            },
            ConstOperandKind::Type(TypePath {
                qself: Some(_qself),
                path: _,
            }) => {
                todo!();
            },
            ConstOperandKind::LangItem(lang_item) => {
                let lang_item = self.expand(lang_item);
                quote_each_token!(tokens #patterns.mk_zeroed(#lang_item));
            },
        }
    }
}

impl ToTokens for Expand<'_, &Const> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            Const::Lit(lit) => self.expand(lit).to_tokens(tokens),
            Const::Path(TypePath { qself: None, path }) => {
                todo!("{}", path.to_token_stream());
                // let path = self.expand(path);
                // quote_each_token!(tokens ::rpl_mir::pat::ConstOperand::ZeroSized(
                //     #patterns.pcx.mk_item_path(&[#path]),
                // ));
            },
            Const::Path(TypePath {
                qself: Some(_qself),
                path: _,
            }) => {
                todo!();
            },
        }
    }
}
impl ToTokens for Expand<'_, &syn::Lit> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        match self.value {
            syn::Lit::Str(_lit_str) => todo!(),
            syn::Lit::ByteStr(_lit_byte_str) => todo!(),
            syn::Lit::CStr(_lit_cstr) => todo!(),
            syn::Lit::Byte(_lit_byte) => todo!(),
            syn::Lit::Char(_lit_char) => todo!(),
            syn::Lit::Int(lit_int) => {
                quote_each_token!(tokens ::rpl_mir::pat::ConstOperand::ScalarInt(#lit_int.into()));
            },
            syn::Lit::Float(_lit_float) => todo!(),
            syn::Lit::Bool(_lit_bool) => todo!(),
            syn::Lit::Verbatim(_literal) => todo!(),
            _ => todo!(),
        }
    }
}

struct Projections<'a>(&'a Place);

impl ToTokens for Expand<'_, Projections<'_>> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let mut place = self.value.0;
        let mut projections = Vec::new();
        loop {
            let mut proj = quote!(::rpl_mir::pat::PlaceElem::);
            let inner = match place {
                Place::Local(_) => break,
                Place::Paren(PlaceParen { place: box inner, .. }) => {
                    place = inner;
                    continue;
                },
                Place::Deref(PlaceDeref { box place, .. }) => {
                    quote_each_token!(proj Deref,);
                    place
                },
                Place::Field(PlaceField { box place, field, .. }) => {
                    let field = self.expand(field);
                    quote_each_token!(proj Field(#field),);
                    place
                },
                Place::Index(PlaceIndex { box place, index, .. }) => {
                    let index = index.as_local();
                    quote_each_token!(proj Index(#index),);
                    place
                },
                &Place::ConstIndex(PlaceConstIndex {
                    box ref place,
                    from_end,
                    index: syn::Index { index, .. },
                    min_length: syn::Index { index: min_length, .. },
                    ..
                }) => {
                    let from_end = from_end.is_some();
                    quote_each_token!(proj ConstIndex {
                        offset: #index,
                        min_length: #min_length,
                        from_end: #from_end
                    },);
                    place
                },
                &Place::Subslice(PlaceSubslice {
                    box ref place,
                    ref from,
                    from_end,
                    to: syn::Index { index: to, .. },
                    ..
                }) => {
                    let from = from.as_ref().map_or(0, |from| from.index);
                    let from_end = from_end.is_some();
                    quote_each_token!(proj Subslice { from: #from, to: #to, from_end: #from_end },);
                    place
                },
                Place::DownCast(PlaceDowncast { box place, variant, .. }) => {
                    let variant = self.expand(variant.to_symbol());
                    quote_each_token!(proj Downcast(#variant),);
                    place
                },
            };
            projections.push(proj);
            place = inner;
        }
        projections.reverse();
        quote_each_token!(tokens #(#projections)*);
    }
}

impl ToTokens for Expand<'_, &syn::Member> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::Field::);
        match self.value {
            syn::Member::Named(name) => {
                let name = self.expand(name.to_symbol());
                quote_each_token!(tokens Named(#name));
            },
            &syn::Member::Unnamed(syn::Index { index, .. }) => {
                quote_each_token!(tokens Unnamed(#index.into()));
            },
        }
    }
}

impl ToTokens for Expand<'_, &Place> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns, .. } = self.ecx;
        if let Place::Local(local) = self.value {
            let local = local.as_local();
            quote_each_token!(tokens #local.into_place());
        } else {
            let local = self.value.local().as_local();
            let projections = self.expand_projections(self.value);
            quote_each_token!(tokens ::rpl_mir::pat::Place::new(#local, #patterns.mk_projection(&[#projections])));
        }
    }
}

struct IdentSymbol(String);

impl ToTokens for Expand<'_, IdentSymbol> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ident = self.value.0.as_str();
        quote_each_token!(tokens ::rustc_span::Symbol::intern(#ident));
    }
}

impl ToTokens for Expand<'_, &RvalueAggregate> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { patterns, .. } = self.ecx;
        quote_each_token!(tokens ::rpl_mir::pat::AggKind::);
        match self.value {
            RvalueAggregate::Array(AggregateArray { operands, .. }) => {
                // let ty = self.expand(ty);
                let operands = self.expand_punctuated(&operands.operands);
                quote_each_token!(tokens Array, #patterns.mk_list([#operands]));
            },
            RvalueAggregate::Tuple(AggregateTuple { operands }) => {
                let operands = self.expand_punctuated(&operands.value);
                quote_each_token!(tokens Tuple, #patterns.mk_list([#operands]));
            },
            RvalueAggregate::AdtStruct(AggregateAdtStruct {
                adt,
                fields: StructFields { fields, .. },
            }) => {
                let adt = self.expand(adt);
                let operands = self.expand_punctuated_mapped(fields, |f| &f.operand);
                let fields = self.expand_punctuated_mapped(fields, |f| f.ident.to_symbol());
                quote_each_token!(tokens
                    Adt(#adt, #patterns.mk_list([#fields]).into()),
                    #patterns.mk_list([#operands]),
                );
            },
            RvalueAggregate::AdtTuple(AggregateAdtTuple { adt, fields, .. }) => {
                let adt = self.expand(adt);
                let operands = self.expand_punctuated(&fields.value);
                quote_each_token!(tokens
                    Adt(#adt, ::rpl_mir::pat::AggAdtKind::Tuple),
                    #patterns.mk_list([#operands]),
                );
            },
            RvalueAggregate::AdtUnit(AggregateAdtUnit { adt }) => {
                let adt = self.expand(adt);
                quote_each_token!(tokens Adt(#adt, ::rpl_mir::pat::AggAdtKind::Unit), Box::new([]));
            },
            RvalueAggregate::RawPtr(AggregateRawPtr {
                ty: TypePtr { mutability, box ty, .. },
                ptr,
                metadata,
                ..
            }) => {
                let ty = self.expand(ty);
                let mutability = self.expand(*mutability);
                let ptr = self.expand(ptr);
                let metadata = self.expand(metadata);
                quote_each_token!(tokens RawPtr(#ty, #mutability), #patterns.mk_list([#ptr, #metadata]));
            },
        }
    }
}

impl ToTokens for Expand<'_, &PathOrLangItem> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            PathOrLangItem::Path(path) => self.expand(path).to_tokens(tokens),
            PathOrLangItem::LangItem(lang_item) => self.expand(lang_item).to_tokens(tokens),
        }
    }
}

impl ToTokens for Expand<'_, RegionKind> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::RegionKind::);
        match self.value {
            RegionKind::ReAny(_) => quote_token!(ReAny tokens),
            RegionKind::ReStatic(_) => quote_token!(ReStatic tokens),
        }
    }
}

struct BorrowKind(Mutability);

impl ToTokens for Expand<'_, BorrowKind> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rustc_middle::mir::BorrowKind::);
        match self.value.0 {
            Mutability::Not => {
                quote_each_token!(tokens Shared);
            },
            Mutability::Mut(_) => {
                quote_each_token!(tokens Mut { kind: ::rustc_middle::mir::MutBorrowKind::Default });
            },
        }
    }
}

impl ToTokens for Expand<'_, Mutability> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rustc_middle::mir::Mutability::);
        match self.value {
            Mutability::Not => quote_token!(Not tokens),
            Mutability::Mut(_) => quote_token!(Mut tokens),
        }
    }
}

impl ToTokens for Expand<'_, PtrMutability> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rustc_middle::mir::Mutability::);
        match self.value {
            PtrMutability::Const(_) => quote_token!(Not tokens),
            PtrMutability::Mut(_) => quote_token!(Mut tokens),
        }
    }
}

impl ToTokens for Expand<'_, CastKind> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let value = self.value;
        quote_each_token!(tokens ::rustc_middle::mir::CastKind::#value);
    }
}

impl ToTokens for Expand<'_, BinOp> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let value = self.value;
        quote_each_token!(tokens ::rustc_middle::mir::BinOp::#value);
    }
}

impl ToTokens for Expand<'_, UnOp> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let value = self.value;
        quote_each_token!(tokens ::rustc_middle::mir::UnOp::#value);
    }
}

impl ToTokens for Expand<'_, NullOp> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let value = self.value;
        quote_each_token!(tokens ::rustc_middle::mir::NullOp::#value);
    }
}
