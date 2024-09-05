use std::panic::panic_any;

use proc_macro2::TokenStream;
use rustc_hash::FxHashMap;
use syn::Ident;
use syntax::{MetaItem, MetaKind, Path, Type};

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ResolveError {
    #[error("type or path `${0}` is already declared")]
    TypeVarAlreadyDeclared(String),
    #[error("type variable `${0}` is not declared")]
    TypeVarNotDeclared(String),
    #[error("type or path named by `{0}` is already declared")]
    TypeOrAlreadyDeclared(String),
    #[error("type or path named by `{0}` is not declared")]
    TypeOrPathNotDeclared(String),
    #[error("local `{0}` is not declared")]
    LocalNotDeclared(String),
}

#[derive(Default)]
pub struct SymbolTable {
    locals: FxHashMap<Ident, Type>,
    types: FxHashMap<Ident, TypeKind>,
    ty_vars: FxHashMap<Ident, MetaItem>,
}

pub enum TypeKind {
    Type(Type),
    Path(Path),
}

impl From<Type> for TypeKind {
    fn from(ty: Type) -> Self {
        TypeKind::Type(ty)
    }
}

impl From<Path> for TypeKind {
    fn from(path: Path) -> Self {
        TypeKind::Path(path)
    }
}

impl quote::ToTokens for TypeKind {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TypeKind::Type(ty) => ty.to_tokens(tokens),
            TypeKind::Path(path) => path.to_tokens(tokens),
        }
    }
}

impl SymbolTable {
    fn add_type_impl(&mut self, ident: Ident, ty: TypeKind) {
        self.types
            .try_insert(ident.clone(), ty)
            .map(|_| {})
            .unwrap_or_else(|entry| {
                panic_any(syn::Error::new_spanned(
                    entry.entry.get(),
                    ResolveError::TypeOrAlreadyDeclared(ident.to_string()),
                ))
            })
    }
    pub fn add_ty_var(&mut self, meta_item: MetaItem) {
        assert!(matches!(meta_item.kind, MetaKind::Ty(_)));
        let ident = meta_item.ident.clone();
        self.ty_vars
            .try_insert(ident.clone(), meta_item)
            .map(|_| {})
            .unwrap_or_else(|entry| {
                panic_any(syn::Error::new_spanned(
                    entry.entry.get(),
                    ResolveError::TypeVarAlreadyDeclared(ident.to_string()),
                ))
            })
    }
    pub fn add_type(&mut self, ident: Ident, ty: Type) {
        self.add_type_impl(ident, ty.into());
    }
    pub fn get_ty_var(&self, ident: &Ident) -> &MetaItem {
        self.ty_vars.get(ident).unwrap_or_else(|| {
            panic_any(syn::Error::new(
                ident.span(),
                ResolveError::TypeVarNotDeclared(ident.to_string()),
            ))
        })
    }
    pub fn get_type(&self, ident: &Ident) -> &TypeKind {
        self.types.get(ident).unwrap_or_else(|| {
            panic_any(syn::Error::new(
                ident.span(),
                ResolveError::TypeOrPathNotDeclared(ident.to_string()),
            ))
        })
    }
    pub fn add_local(&mut self, ident: Ident, ty: Type) {
        self.locals.insert(ident, ty);
    }
    pub fn get_local(&self, ident: &Ident) -> &Type {
        self.locals.get(ident).unwrap_or_else(|| {
            panic_any(syn::Error::new(
                ident.span(),
                ResolveError::LocalNotDeclared(ident.to_string()),
            ))
        })
    }
    pub fn add_path(&mut self, path: Path) {
        let ident = path
            .ident()
            .expect("invalid path without an identifier at the end")
            .clone();
        self.add_type_impl(ident, path.into());
    }
}
