use derive_more::{Display, From};
use proc_macro2::Span;
use rustc_hash::FxHashMap;
use syn::Ident;
use syn_derive::ToTokens;
use syntax::{Path, PlaceLocal, PlaceLocalKind, PlaceMetaVar, SelfParam, TyVar, Type};

#[derive(Debug, Display)]
pub(crate) enum SymbolKind {
    #[display("struct")]
    Struct,
    #[display("variant")]
    Variant,
    #[display("enum")]
    Enum,
    #[display("field")]
    Field,
    #[display("local varialble")]
    Local,
    #[display("parameter")]
    Param,
    #[display("function")]
    Fn,
}

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum CheckError<'a> {
    #[error("{0} `{1}` is already declared")]
    SymbolAlreadyDeclared(SymbolKind, &'a Ident),
    #[error("{0} `{1}` is not declared")]
    SymbolNotDeclared(SymbolKind, &'a Ident),
    #[error("type or path `${0}` is already declared")]
    TypeVarAlreadyDeclared(&'a Ident),
    #[error("place variable `${0}` is already declared")]
    PlaceVarAlreadyDeclared(&'a Ident),
    #[error("type variable `${0}` is not declared")]
    TypeVarNotDeclared(&'a Ident),
    #[error("place variable `${0}` is not declared")]
    PlaceVarNotDeclared(&'a Ident),
    #[error("export named by `{0}` is already declared")]
    ExportAlreadyDeclared(&'a Ident),
    #[error("type or path named by `{0}` is already declared")]
    TypeOrPathAlreadyDeclared(&'a Ident),
    #[error("type or path named by `{0}` is not declared")]
    TypeOrPathNotDeclared(&'a Ident),
    #[error("missing `$` in before function identifier `{0}`")]
    FnIdentMissingDollar(&'a Ident),
    #[error("method `{0}::{1}` is already declared")]
    MethodAlreadyDeclared(String, &'a Ident),
    #[error("method `{0}::{1}` is not declared")]
    #[expect(dead_code)]
    MethodNotDeclared(String, &'a Ident),
    #[error("`self` is not declared")]
    SelfNotDeclared,
    #[error("`RET` is not declared")]
    RetNotDeclared,
    #[error("`self` is already declared")]
    SelfAlreadyDeclared,
    #[error("using `self` value outside of an `impl` item")]
    #[expect(dead_code)]
    SelfValueOutsideImpl,
    #[error("using `Self` type outside of an `impl` item")]
    SelfTypeOutsideImpl,
    #[error("constant index `{0}` out of bound for minimum length `{1}`")]
    ConstantIndexOutOfBound(u32, u32),
    #[error("multiple otherwise (`_`) branches in switchInt statement")]
    MultipleOtherwiseInSwitchInt,
    #[error("missing integer suffix in switchInt statement")]
    MissingSuffixInSwitchInt,
    #[error("unknown language item \"{0}\"")]
    UnknownLangItem(String),
}

#[derive(Clone, Copy, From, ToTokens)]
pub(crate) enum TypeOrPath<'a> {
    Type(&'a Type),
    Path(&'a Path),
}

#[derive(Clone, Copy)]
pub(crate) enum ExportKind {
    Meta,
    Statement,
    SwitchTarget,
}

impl From<syntax::ExportKind> for ExportKind {
    fn from(kind: syntax::ExportKind) -> Self {
        match kind {
            syntax::ExportKind::Statement(_) => ExportKind::Statement,
        }
    }
}

impl<P: syn::parse::Parse + quote::ToTokens> From<&syntax::PunctAnd<P, syntax::ExportKind>> for ExportKind {
    fn from(kind: &syntax::PunctAnd<P, syntax::ExportKind>) -> Self {
        kind.value.into()
    }
}

#[derive(Default)]
pub(crate) struct MetaTable<'a> {
    ty_vars: FxHashMap<&'a Ident, &'a TyVar>,
    place_vars: FxHashMap<&'a Ident, &'a PlaceMetaVar>,
    exports: FxHashMap<&'a Ident, ExportKind>,
}

pub(crate) struct WithMetaTable<'a, T> {
    pub(crate) meta: MetaTable<'a>,
    pub(crate) inner: T,
}

impl<T> From<T> for WithMetaTable<'_, T> {
    fn from(inner: T) -> Self {
        Self {
            meta: MetaTable::default(),
            inner,
        }
    }
}

#[derive(Default)]
pub(crate) struct SymbolTable<'a> {
    structs: FxHashMap<&'a Ident, Struct<'a>>,
    enums: FxHashMap<&'a Ident, Enum<'a>>,
    fns: FxHashMap<&'a Ident, Fn<'a>>,
    unnamed_fns: Vec<Fn<'a>>,
    impls: Vec<Impl<'a>>,
}

pub(crate) type Enum<'a> = WithMetaTable<'a, EnumInner<'a>>;

pub(crate) struct EnumInner<'a> {
    variants: FxHashMap<&'a Ident, Variant<'a>>,
}

impl<'a> EnumInner<'a> {
    fn new() -> Self {
        Self {
            variants: FxHashMap::default(),
        }
    }
    pub fn add_variant(&mut self, ident: &'a Ident) -> syn::Result<&mut Variant<'a>> {
        self.variants.try_insert(ident, Variant::new()).map_err(|entry| {
            let variant = entry.entry.key();
            syn::Error::new(
                variant.span(),
                CheckError::SymbolAlreadyDeclared(SymbolKind::Variant, variant),
            )
        })
    }
}

pub(crate) struct Variant<'a> {
    fields: FxHashMap<&'a Ident, &'a Type>,
}

impl<'a> Variant<'a> {
    fn new() -> Self {
        Self {
            fields: FxHashMap::default(),
        }
    }
    pub fn add_field(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.fields.try_insert(ident, ty).map_err(|entry| {
            let field = entry.entry.key();
            syn::Error::new(
                field.span(),
                CheckError::SymbolAlreadyDeclared(SymbolKind::Field, field),
            )
        })?;
        Ok(())
    }
}

pub(crate) type Struct<'a> = WithMetaTable<'a, Variant<'a>>;

pub(crate) type Fn<'a> = WithMetaTable<'a, FnInner<'a>>;

pub(crate) struct FnInner<'a> {
    span: Span,
    types: FxHashMap<&'a Ident, TypeOrPath<'a>>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<(syn::Token![self], &'a Type)>,
    self_param: Option<&'a SelfParam>,
    self_ty: Option<&'a Type>,
    return_value: Option<(Span, &'a Type)>,
    params: FxHashMap<&'a Ident, &'a Type>,
    locals: FxHashMap<&'a Ident, Vec<&'a Type>>,
}

impl<'a> FnInner<'a> {
    fn new(span: Span, self_ty: Option<&'a Type>) -> Self {
        Self {
            span,
            types: FxHashMap::default(),
            self_value: None,
            self_param: None,
            self_ty,
            return_value: None,
            params: FxHashMap::default(),
            locals: FxHashMap::default(),
        }
    }
}

pub(crate) type Impl<'a> = WithMetaTable<'a, ImplInner<'a>>;

pub(crate) struct ImplInner<'a> {
    #[expect(unused)]
    trait_: Option<&'a Path>,
    ty: &'a Type,
    fns: FxHashMap<&'a Ident, FnInner<'a>>,
}

impl<'a> ImplInner<'a> {
    pub fn new(impl_pat: &'a syntax::Impl) -> Self {
        Self {
            trait_: impl_pat.kind.as_path(),
            ty: &impl_pat.ty,
            fns: FxHashMap::default(),
        }
    }
}

static PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool", "str",
];

pub(crate) fn is_primitive(ident: &Ident) -> bool {
    PRIMITIVES.contains(&ident.to_string().as_str())
}

impl<'a> FnInner<'a> {
    fn add_type_impl(&mut self, ident: &'a Ident, ty: TypeOrPath<'a>) -> syn::Result<()> {
        self.types
            .try_insert(ident, ty)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::TypeOrPathAlreadyDeclared(ident)))?;
        Ok(())
    }
    pub fn add_type(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.add_type_impl(ident, ty.into())
    }
    pub fn get_type(&self, ident: &Ident) -> syn::Result<TypeOrPath<'a>> {
        self.types
            .get(ident)
            .copied()
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::TypeOrPathNotDeclared(ident)))
    }
    pub fn add_path(&mut self, path: &'a Path) -> syn::Result<()> {
        let ident = path.ident().expect("invalid path without an identifier at the end");
        self.add_type_impl(ident, path.into())
    }
}

impl<'a> MetaTable<'a> {
    pub fn add_ty_var(&mut self, ident: &'a Ident, ty_var: &'a TyVar) -> syn::Result<()> {
        self.ty_vars
            .try_insert(ident, ty_var)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::TypeVarAlreadyDeclared(ident)))?;
        Ok(())
    }
    pub fn get_ty_var(&self, ident: &Ident) -> syn::Result<&'a TyVar> {
        self.ty_vars
            .get(ident)
            .copied()
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::TypeVarNotDeclared(ident)))
    }
    pub fn add_place_var(&mut self, ident: &'a Ident, place_var: &'a PlaceMetaVar) -> syn::Result<()> {
        self.place_vars
            .try_insert(ident, place_var)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::PlaceVarAlreadyDeclared(ident)))?;
        Ok(())
    }
    pub fn get_place_var(&self, ident: &Ident) -> syn::Result<&'a PlaceMetaVar> {
        self.place_vars
            .get(ident)
            .copied()
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::PlaceVarNotDeclared(ident)))
    }
    pub fn add_export(&mut self, export: &'a Ident, kind: ExportKind) -> syn::Result<()> {
        self.exports.try_insert(export, kind).map_err(|entry| {
            let ident = entry.entry.key();
            syn::Error::new(ident.span(), CheckError::ExportAlreadyDeclared(ident))
        })?;
        Ok(())
    }
}

impl<'a> SymbolTable<'a> {
    pub fn add_enum(&mut self, ident: &'a Ident) -> syn::Result<&mut Enum<'a>> {
        self.enums.try_insert(ident, EnumInner::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            syn::Error::new(adt.span(), CheckError::SymbolAlreadyDeclared(SymbolKind::Enum, adt))
        })
    }
    pub fn add_struct(&mut self, ident: &'a Ident) -> syn::Result<&mut Struct<'a>> {
        self.structs.try_insert(ident, Variant::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            syn::Error::new(adt.span(), CheckError::SymbolAlreadyDeclared(SymbolKind::Struct, adt))
        })
    }
    pub fn add_fn(&mut self, ident: &'a syntax::IdentPat, self_ty: Option<&'a Type>) -> syn::Result<&mut Fn<'a>> {
        use syntax::IdentPat::{Ident, Pat, Underscore};
        match ident {
            Underscore(underscore) => {
                self.unnamed_fns.push(FnInner::new(underscore.span, self_ty).into());
                Ok(self.unnamed_fns.last_mut().unwrap())
            },
            Pat(_, ident) => self
                .fns
                .try_insert(ident, FnInner::new(ident.span(), self_ty).into())
                .map_err(|entry| {
                    syn::Error::new(
                        entry.entry.get().inner.span,
                        CheckError::SymbolAlreadyDeclared(SymbolKind::Fn, ident),
                    )
                }),

            Ident(ident) => Err(syn::Error::new(ident.span(), CheckError::FnIdentMissingDollar(ident))),
        }
    }
    #[expect(unused)]
    pub fn add_impl(&mut self, impl_pat: &'a syntax::Impl) -> &mut Impl<'a> {
        self.impls.push(ImplInner::new(impl_pat).into());
        self.impls.last_mut().unwrap()
    }
    pub fn contains_adt(&self, ident: &Ident) -> bool {
        self.structs.contains_key(ident) || self.enums.contains_key(ident)
    }
}

impl<'a> ImplInner<'a> {
    #[expect(unused)]
    pub fn add_fn(&mut self, ident: &'a Ident, fn_def: FnInner<'a>) -> syn::Result<&mut FnInner<'a>> {
        self.fns.try_insert(ident, fn_def).map_err(|entry| {
            syn::Error::new(
                entry.entry.get().span,
                CheckError::MethodAlreadyDeclared(quote::ToTokens::to_token_stream(self.ty).to_string(), ident),
            )
        })
    }
}

impl<'a> FnInner<'a> {
    pub fn add_self_param(&mut self, self_param: &'a SelfParam) -> syn::Result<()> {
        if self.self_param.is_some() {
            return Err(syn::Error::new(
                self_param.tk_self.span,
                CheckError::SelfAlreadyDeclared,
            ));
        }
        self.self_param = Some(self_param);
        Ok(())
    }
    pub fn add_param(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.params.try_insert(ident, ty).map_err(|entry| {
            syn::Error::new(
                entry.entry.key().span(),
                CheckError::SymbolNotDeclared(SymbolKind::Param, ident),
            )
        })?;
        Ok(())
    }
    pub fn add_local(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.locals.entry(ident).or_default().push(ty);
        Ok(())
    }
    pub fn add_place_local(&mut self, local: &'a PlaceLocal, ty: &'a Type) -> syn::Result<()> {
        match &local.kind {
            PlaceLocalKind::Return(return_value) => {
                self.return_value = Some((return_value.span, ty));
                Ok(())
            },
            PlaceLocalKind::Local(ident) => self.add_local(ident, ty),
            &PlaceLocalKind::SelfValue(self_value) => {
                self.self_value = Some((self_value, ty));
                Ok(())
            },
        }
    }
    fn get_local_impl(&self, ident: &Ident) -> Option<&'a Type> {
        self.locals
            .get(ident)
            .and_then(|types| types.last())
            .or_else(|| self.params.get(ident))
            .copied()
    }
    pub fn get_local(&self, ident: &Ident) -> syn::Result<&'a Type> {
        self.get_local_impl(ident)
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::SymbolNotDeclared(SymbolKind::Local, ident)))
    }
    pub fn get_place_local(&self, local: &PlaceLocal) -> syn::Result<&'a Type> {
        match &local.kind {
            PlaceLocalKind::Return(return_value) => self
                .return_value
                .map(|(_, ty)| ty)
                .ok_or_else(|| syn::Error::new(return_value.span, CheckError::RetNotDeclared)),
            PlaceLocalKind::Local(ident) => self.get_local(ident),
            PlaceLocalKind::SelfValue(self_value) if self.self_value.is_none() && self.self_param.is_none() => {
                Err(syn::Error::new(self_value.span, CheckError::SelfNotDeclared))
            },
            PlaceLocalKind::SelfValue(self_value) => self
                .self_value
                .map(|(_, ty)| ty)
                .or(self.self_ty)
                .ok_or_else(|| syn::Error::new(self_value.span, CheckError::SelfTypeOutsideImpl)),
        }
    }
}
