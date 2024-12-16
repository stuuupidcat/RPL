use derive_more::From;
use proc_macro2::Span;
use rustc_hash::FxHashMap;
use syn::Ident;
use syn_derive::ToTokens;
use syntax::{Path, PlaceLocal, SelfParam, TyVar, Type};

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum CheckError<'a> {
    #[error("type or path `${0}` is already declared")]
    TypeVarAlreadyDeclared(&'a Ident),
    #[error("type variable `${0}` is not declared")]
    TypeVarNotDeclared(&'a Ident),
    #[error("{0} `{1}` is already declared")]
    AdtAlreadyDeclared(&'static str, &'a Ident),
    #[error("struct or enum `{0}` is not declared")]
    #[expect(dead_code)]
    AdtNotDeclared(&'a Ident),
    #[error("export named by `{0}` is already declared")]
    ExportAlreadyDeclared(&'a Ident),
    #[error("type or path named by `{0}` is already declared")]
    TypeOrPathAlreadyDeclared(&'a Ident),
    #[error("type or path named by `{0}` is not declared")]
    TypeOrPathNotDeclared(&'a Ident),
    #[error("local variable `{0}` is not declared")]
    LocalNotDeclared(&'a Ident),
    #[error("parameter `{0}` is not declared")]
    ParamNotDeclared(&'a Ident),
    #[error("parameter `{0}` is already declared")]
    #[expect(dead_code)]
    ParamAlreadyDeclared(&'a Ident),
    #[error("function `{0}` is already declared")]
    FnAlreadyDeclared(&'a Ident),
    #[error("function `{0}` is not declared")]
    #[expect(dead_code)]
    FnNotDeclared(&'a Ident),
    #[error("missing `$` in before function identifier `{0}`")]
    FnIdentMissingDollar(&'a Ident),
    #[error("method `{0}::{1}` is already declared")]
    MethodAlreadyDeclared(String, &'a Ident),
    #[error("method `{0}::{1}` is not declared")]
    #[expect(dead_code)]
    MethodNotDeclared(String, &'a Ident),
    #[error("`self` is not declared")]
    SelfNotDeclared,
    #[error("`self` is already declared")]
    SelflreadyDeclared,
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

#[derive(Default)]
pub(crate) struct MetaTable<'a> {
    ty_vars: FxHashMap<&'a Ident, &'a TyVar>,
    types: FxHashMap<&'a Ident, TypeOrPath<'a>>,
    exports: FxHashMap<Ident, syntax::ExportKind>,
}

#[derive(Default)]
pub(crate) struct SymbolTable<'a> {
    #[expect(dead_code)]
    pub(super) meta: MetaTable<'a>,
    adts: FxHashMap<&'a Ident, AdtDef<'a>>,
    fns: FxHashMap<&'a Ident, FnDef<'a>>,
    unnamed_fns: Vec<FnDef<'a>>,
    impls: Vec<ImplDef<'a>>,
}

pub(crate) struct AdtDef<'a> {
    ident: &'a Ident,
    kind: AdtKind<'a>,
}

impl<'a> AdtDef<'a> {
    fn new(ident: &'a Ident, kind: AdtKind<'a>) -> Self {
        Self { ident, kind }
    }
}

pub(crate) struct VariantDef<'a> {
    #[expect(unused)]
    ident: &'a Ident,
    #[expect(unused)]
    fields: FxHashMap<&'a Ident, &'a Type>,
}

impl<'a> VariantDef<'a> {
    #[expect(unused)]
    fn new(ident: &'a Ident) -> Self {
        Self {
            ident,
            fields: FxHashMap::default(),
        }
    }
}

pub(crate) enum AdtKind<'a> {
    #[expect(unused)]
    Enum(FxHashMap<&'a Ident, VariantDef<'a>>),
    #[expect(unused)]
    Struct(FxHashMap<&'a Ident, &'a Type>),
}

impl AdtKind<'_> {
    fn descr(&self) -> &'static str {
        match self {
            AdtKind::Enum(_) => "enum",
            AdtKind::Struct(_) => "struct",
        }
    }
}

pub(crate) struct FnDef<'a> {
    span: Span,
    pub(super) meta: MetaTable<'a>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<(syn::Token![self], &'a Type)>,
    self_param: Option<&'a SelfParam>,
    self_ty: Option<&'a Type>,
    params: FxHashMap<&'a Ident, &'a Type>,
    locals: FxHashMap<&'a Ident, Vec<&'a Type>>,
}

impl<'a> FnDef<'a> {
    fn new(span: Span, self_ty: Option<&'a Type>) -> Self {
        Self {
            span,
            meta: MetaTable::default(),
            self_value: None,
            self_param: None,
            self_ty,
            params: FxHashMap::default(),
            locals: FxHashMap::default(),
        }
    }
}

pub(crate) struct ImplDef<'a> {
    #[expect(unused)]
    trait_: Option<&'a Path>,
    ty: &'a Type,
    fns: FxHashMap<&'a Ident, FnDef<'a>>,
}

impl<'a> ImplDef<'a> {
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

impl<'a> MetaTable<'a> {
    fn add_type_impl(&mut self, ident: &'a Ident, ty: TypeOrPath<'a>) -> syn::Result<()> {
        self.types
            .try_insert(ident, ty)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::TypeOrPathAlreadyDeclared(ident)))?;
        Ok(())
    }
    pub fn add_ty_var(&mut self, ident: &'a Ident, ty_var: &'a TyVar) -> syn::Result<()> {
        self.ty_vars
            .try_insert(ident, ty_var)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::TypeVarAlreadyDeclared(ident)))?;
        Ok(())
    }
    pub fn add_type(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.add_type_impl(ident, ty.into())
    }
    pub fn get_ty_var(&self, ident: &Ident) -> syn::Result<&'a TyVar> {
        self.ty_vars
            .get(ident)
            .copied()
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::TypeVarNotDeclared(ident)))
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
    pub fn add_export(&mut self, export: syntax::Export) -> syn::Result<()> {
        self.exports.try_insert(export.ident, export.kind).map_err(|entry| {
            let ident = entry.entry.key();
            syn::Error::new(ident.span(), CheckError::ExportAlreadyDeclared(ident))
        })?;
        Ok(())
    }
}

impl<'a> SymbolTable<'a> {
    fn add_adt(&mut self, ident: &'a Ident, kind: AdtKind<'a>) -> syn::Result<&mut AdtDef<'a>> {
        self.adts.try_insert(ident, AdtDef::new(ident, kind)).map_err(|entry| {
            let adt = entry.entry.get();
            syn::Error::new(
                adt.ident.span(),
                CheckError::AdtAlreadyDeclared(adt.kind.descr(), ident),
            )
        })
    }
    pub fn add_struct(&mut self, ident: &'a Ident) -> syn::Result<&mut AdtDef<'a>> {
        self.add_adt(ident, AdtKind::Struct(FxHashMap::default()))
    }
    pub fn add_enum(&mut self, ident: &'a Ident) -> syn::Result<&mut AdtDef<'a>> {
        self.add_adt(ident, AdtKind::Enum(FxHashMap::default()))
    }
    pub fn add_fn(&mut self, ident: &'a syntax::IdentPat, self_ty: Option<&'a Type>) -> syn::Result<&mut FnDef<'a>> {
        use syntax::IdentPat::{Ident, Pat, Underscore};
        match ident {
            Underscore(underscore) => {
                self.unnamed_fns.push(FnDef::new(underscore.span, self_ty));
                Ok(self.unnamed_fns.last_mut().unwrap())
            },
            Pat(_, ident) => self
                .fns
                .try_insert(ident, FnDef::new(ident.span(), self_ty))
                .map_err(|entry| syn::Error::new(entry.entry.get().span, CheckError::FnAlreadyDeclared(ident))),

            Ident(ident) => Err(syn::Error::new(ident.span(), CheckError::FnIdentMissingDollar(ident))),
        }
    }
    #[expect(unused)]
    pub fn add_impl(&mut self, impl_pat: &'a syntax::Impl) -> &mut ImplDef<'a> {
        self.impls.push(ImplDef::new(impl_pat));
        self.impls.last_mut().unwrap()
    }
}

impl<'a> ImplDef<'a> {
    #[expect(unused)]
    pub fn add_fn(&mut self, ident: &'a Ident, fn_def: FnDef<'a>) -> syn::Result<&mut FnDef<'a>> {
        self.fns.try_insert(ident, fn_def).map_err(|entry| {
            syn::Error::new(
                entry.entry.get().span,
                CheckError::MethodAlreadyDeclared(quote::ToTokens::to_token_stream(self.ty).to_string(), ident),
            )
        })
    }
}

impl<'a> FnDef<'a> {
    pub fn add_self_param(&mut self, self_param: &'a SelfParam) -> syn::Result<()> {
        if self.self_param.is_some() {
            return Err(syn::Error::new(self_param.tk_self.span, CheckError::SelflreadyDeclared));
        }
        self.self_param = Some(self_param);
        Ok(())
    }
    pub fn add_param(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.params
            .try_insert(ident, ty)
            .map_err(|entry| syn::Error::new(entry.entry.key().span(), CheckError::ParamNotDeclared(ident)))?;
        Ok(())
    }
    pub fn add_local(&mut self, ident: &'a Ident, ty: &'a Type) -> syn::Result<()> {
        self.locals.entry(ident).or_default().push(ty);
        Ok(())
    }
    pub fn add_place_local(&mut self, local: &'a PlaceLocal, ty: &'a Type) -> syn::Result<()> {
        match local {
            PlaceLocal::Local(ident) => self.add_local(ident, ty),
            &PlaceLocal::SelfValue(self_value) => {
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
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::LocalNotDeclared(ident)))
    }
    pub fn get_place_local(&self, local: &PlaceLocal) -> syn::Result<&'a Type> {
        match local {
            PlaceLocal::Local(ident) => self.get_local(ident),
            PlaceLocal::SelfValue(self_value) if self.self_value.is_none() && self.self_param.is_none() => {
                Err(syn::Error::new(self_value.span, CheckError::SelfNotDeclared))
            },
            PlaceLocal::SelfValue(self_value) => self
                .self_value
                .map(|(_, ty)| ty)
                .or(self.self_ty)
                .ok_or_else(|| syn::Error::new(self_value.span, CheckError::SelfTypeOutsideImpl)),
        }
    }
}
