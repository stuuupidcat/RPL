use proc_macro2::TokenStream;
use quote::{format_ident, quote_each_token, quote_token, ToTokens};
use syn::punctuated::{Pair, Pairs, Punctuated};
use syn::Ident;
use syntax::*;

use crate::MirPatternFn;

pub(crate) struct ExpandCtxt<'ecx> {
    tcx: &'ecx Ident,
    patterns: &'ecx Ident,
}

pub(crate) fn expand_impl<T>(value: T, tcx: &Ident, patterns: &Ident, tokens: &mut TokenStream)
where
    for<'ecx> Expand<'ecx, T>: ToTokens,
{
    let ecx = ExpandCtxt::new(tcx, patterns);
    ecx.expand(value).to_tokens(tokens);
}

pub fn expand(mir_pattern: MirPatternFn) -> TokenStream {
    mir_pattern
        .expand_macro_mir()
        .unwrap_or_else(syn::Error::into_compile_error)
}

static PARAM_TCX: &str = "tcx";
pub(crate) static PARAM_PATTERNS: &str = "patterns";

pub fn expand_mir(mir: Mir, tcx_and_patterns: Option<(&Ident, &Ident)>) -> syn::Result<TokenStream> {
    crate::check_mir(&mir)?;
    let (tcx, patterns) = match tcx_and_patterns {
        None => (
            &syn::Ident::new(PARAM_TCX, proc_macro2::Span::call_site()),
            &syn::Ident::new(PARAM_PATTERNS, proc_macro2::Span::call_site()),
        ),
        Some(tcx_and_patterns) => tcx_and_patterns,
    };
    let mut tokens = TokenStream::new();
    expand_impl(&mir, &tcx, &patterns, &mut tokens);
    Ok(tokens)
}

trait ExpandIdent: Sized + std::borrow::Borrow<Ident> + Into<Ident> {
    fn with_suffix(&self, suffix: impl std::fmt::Display) -> Ident {
        let ident = self.borrow();
        format_ident!("{ident}{suffix}")
    }
    // fn with_span(self, span: Span) -> Ident {
    //     let mut ident = self.into();
    //     ident.set_span(span);
    //     ident
    // }
    fn as_symbol(&self) -> IdentSymbol<'_> {
        IdentSymbol(self.borrow())
    }
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

impl ExpandIdent for Ident {}

impl<'ecx> ExpandCtxt<'ecx> {
    pub(crate) fn new(tcx: &'ecx Ident, patterns: &'ecx Ident) -> Self {
        Self { tcx, patterns }
    }
    pub(crate) fn expand<T>(&self, value: T) -> Expand<'_, T> {
        Expand { value, ecx: self }
    }
    fn expand_punctuated<'a, U: 'a, P: 'a>(
        &'ecx self,
        value: &'a Punctuated<U, P>,
    ) -> ExpandPunctPairs<'ecx, 'a, U, P> {
        self.expand_with(value, Punctuated::pairs)
    }
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
        let Mir { metas, statements } = &self.value;
        let metas = metas.iter().map(|meta| self.expand(meta));
        let statements = statements.iter().map(|statement| self.expand(statement));
        quote_each_token!(tokens #(#metas)* #(#statements)*);
    }
}

impl ToTokens for Expand<'_, &MetaItem> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { tcx, patterns } = self.ecx;
        let MetaItem { ident, kind, .. } = &self.value;
        match kind {
            MetaKind::Ty(_) => {
                let ty_ident = ident.as_ty();
                let ty_var_ident = ident.as_ty_var();
                quote_each_token!(tokens
                    #[allow(non_snake_case)]
                    let #ty_var_ident = #patterns.new_ty_var();
                    #[allow(non_snake_case)]
                    let #ty_ident = #patterns.mk_var_ty(#tcx, #ty_var_ident);
                );
            },
        }
    }
}

impl ToTokens for Expand<'_, &Meta> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.expand_punctuated(&self.value.meta.content).to_tokens(tokens);
    }
}

impl ToTokens for Expand<'_, &Statement> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.value {
            Statement::TypeDecl(type_decl) => self.expand(type_decl).to_tokens(tokens),
            Statement::UsePath(use_path) => self.expand(use_path).to_tokens(tokens),
            Statement::LocalDecl(local_decl) => self.expand(local_decl).to_tokens(tokens),
            Statement::Assign(assign) => self.expand(assign).to_tokens(tokens),
            Statement::Drop(drop) => self.expand(drop).to_tokens(tokens),
        }
    }
}

impl ToTokens for Expand<'_, &TypeDecl> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let TypeDecl { ident, ty, .. } = self.value;
        let ty_ident = ident.as_ty();
        let ty = self.expand(ty);
        quote_each_token!(tokens let #ty_ident = #ty;);
    }
}
impl ToTokens for Expand<'_, &UsePath> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}

impl ToTokens for Expand<'_, &LocalDecl> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let LocalDecl {
            ident,
            ty,
            rvalue_or_call,
            ..
        } = self.value;
        let ExpandCtxt { patterns, .. } = self.ecx;
        let ty = self.expand(ty);
        let local = ident.as_local();
        quote_each_token!(tokens let #local = #patterns.mk_local(#ty); );
        self.expand(Assign {
            place: &Place::Local(ident.clone().into()),
            rvalue_or_call,
        })
        .to_tokens(tokens);
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
        let ExpandCtxt { patterns, tcx } = self.ecx;
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
                let operands = self.expand(operands);
                quote_each_token!(tokens #patterns.mk_fn_call(
                    #tcx, #func, #operands, [/* TODO: generics */], #place,
                ); );
            },
            RvalueOrCall::Any(_) => {
                let local = local.as_local();
                quote_each_token!(tokens #patterns.mk_init(#local); );
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

impl ToTokens for Expand<'_, &Type> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ExpandCtxt { tcx, patterns } = self.ecx;
        match self.value {
            Type::Array(TypeArray { box ty, len, .. }) => {
                let ty = self.expand(ty);
                let len = self.expand(len);
                quote_each_token!(tokens #patterns.mk_array(#tcx, #ty, #len));
            },
            Type::Group(TypeGroup { box ty, .. }) | Type::Paren(TypeParen { box ty, .. }) => {
                self.expand(ty).to_tokens(tokens);
            },
            Type::Never(_) => todo!(),
            Type::Path(TypePath { qself: None, path }) if let Some(ident) = path.as_ident() => {
                if crate::is_primitive(ident) {
                    quote_each_token!(tokens #patterns.primitive_types.#ident);
                } else {
                    ident.as_ty().to_tokens(tokens);
                }
            },
            Type::Path(TypePath { .. }) => todo!(),
            Type::Ptr(TypePtr { mutability, box ty, .. }) => {
                let ty = self.expand(ty);
                let mutability = self.expand(*mutability);
                quote_each_token!(tokens #patterns.mk_raw_ptr_ty(#tcx, #ty, #mutability));
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
                quote_each_token!(tokens #patterns.mk_ref_ty(#tcx, #region, #ty, #mutability));
            },
            Type::Slice(TypeSlice { box ty, .. }) => {
                let ty = self.expand(ty);
                quote_each_token!(tokens #patterns.mk_slice_ty(#tcx, #ty));
            },
            Type::Tuple(_) => todo!(),
            Type::TyVar(TypeVar { ident, .. }) => ident.as_ty().to_tokens(tokens),
        }
    }
}

impl ToTokens for Expand<'_, &CallOperands> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}

impl ToTokens for Expand<'_, &Rvalue> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::Rvalue::);
        match self.value {
            Rvalue::Use(RvalueUse { operand, .. }) => {
                let operand = self.expand(operand);
                quote_each_token!(tokens Use(#operand));
            },
            Rvalue::Repeat(RvalueRepeat { operand, len, .. }) => {
                let operand = self.expand(operand);
                let len = self.expand(len);
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
            Rvalue::AddressOf(RvalueAddrOf { mutability, place, .. }) => {
                let mutability = self.expand(*mutability);
                let place = self.expand(place);
                quote_each_token!(tokens AddressOf(#mutability, #place));
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
        quote_each_token!(tokens ::rpl_mir::pat::);
        match self.value {
            Operand::Copy(OperandCopy { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Copy(#place));
            },
            Operand::Move(OperandMove { place, .. }) => {
                let place = self.expand(place);
                quote_each_token!(tokens Move(#place));
            },
            Operand::Constant(konst) => {
                let konst = self.expand(konst);
                quote_each_token!(tokens Constant(#konst));
            },
        }
    }
}

impl ToTokens for Expand<'_, &Const> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
    }
}

struct Projections<'a>(&'a Place);

impl ToTokens for Expand<'_, Projections<'_>> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        if let Place::Local(_) = self.value.0 {
            return;
        }
        quote_each_token!(tokens ::rpl_mir::pat::PlaceElem::);
        let inner = match self.value.0 {
            Place::Local(_) => return,
            Place::Paren(PlaceParen { box place, .. }) => place,
            Place::Deref(PlaceDeref { box place, .. }) => {
                quote_each_token!(tokens Deref,);
                place
            },
            Place::Field(PlaceField { box place, field, .. }) => {
                let field = self.expand(field);
                quote_each_token!(tokens Field(#field),);
                place
            },
            Place::Index(PlaceIndex { box place, index, .. }) => {
                let index = index.as_local();
                quote_each_token!(tokens Index(#index),);
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
                quote_each_token!(tokens ConstIndex { offset: #index, min_length: #min_length, from_end: #from_end },);
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
                quote_each_token!(tokens Subslice { from: #from, to: #to, from_end: #from_end },);
                place
            },
            Place::DownCast(PlaceDowncast { box place, variant, .. }) => {
                let variant = self.expand(variant.as_symbol());
                quote_each_token!(tokens Downcast(#variant),);
                place
            },
        };
        self.expand(Projections(inner)).to_tokens(tokens);
    }
}

impl ToTokens for Expand<'_, &syn::Member> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::Field::);
        match self.value {
            syn::Member::Named(name) => {
                let name = self.expand(name.as_symbol());
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
        let ExpandCtxt { tcx, patterns, .. } = self.ecx;
        if let Place::Local(PlaceLocal { local }) = self.value {
            let local = local.as_local();
            quote_each_token!(tokens #local.into_place());
        } else {
            let local = self.value.local().as_local();
            let projections = self.expand(Projections(self.value));
            quote_each_token!(tokens #patterns.mk_place(#local, (#tcx, &[#projections])));
        }
    }
}

struct IdentSymbol<'a>(&'a Ident);

impl ToTokens for Expand<'_, IdentSymbol<'_>> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        let ident = self.value.0.to_string();
        quote_each_token!(tokens ::rustc_span::Symbol::intern(#ident));
    }
}

impl ToTokens for Expand<'_, &RvalueAggregate> {
    fn to_tokens(&self, mut tokens: &mut TokenStream) {
        quote_each_token!(tokens ::rpl_mir::pat::AggKind::);
        match self.value {
            RvalueAggregate::Array(AggregateArray { box ty, operands, .. }) => {
                let ty = self.expand(ty);
                let operands = self.expand_punctuated(&operands.operands);
                quote_each_token!(tokens Array(#ty), [#operands].into_iter().collect());
            },
            RvalueAggregate::Tuple(AggregateTuple { operands }) => {
                let operands = self.expand_punctuated(&operands.operands);
                quote_each_token!(tokens Tuple, [#operands].into_iter().collect());
            },
            RvalueAggregate::Adt(AggregateAdt {
                adt,
                fields: StructFields { fields, .. },
            }) => {
                let adt = self.expand(adt);
                let operands = self.expand_punctuated_mapped(fields, |f| &f.operand);
                let fields = self.expand_punctuated_mapped(fields, |f| f.ident.as_symbol());
                quote_each_token!(tokens
                    Adt(#adt, [/* TODO: generic argmuents */], [#fields].into_iter().collect()),
                    [#operands].into_iter().collect(),
                );
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
                quote_each_token!(tokens RawPtr(#ty, #mutability), [#ptr, #metadata].into_iter().collect());
            },
        }
    }
}

impl ToTokens for Expand<'_, &Path> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        todo!()
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
