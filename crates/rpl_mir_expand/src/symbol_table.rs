use proc_macro2::TokenStream;
use rustc_hash::FxHashMap;
use syn::Ident;
use syntax::{MetaItem, MetaKind, Path, Type};

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum CheckError {
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
    #[error("constant index `{0}` out of bound for minimum length `{1}`")]
    ConstantIndexOutOfBound(u32, u32),
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

static PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool", "str",
];

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

pub(crate) fn is_primitive(ident: &Ident) -> bool {
    PRIMITIVES.contains(&ident.to_string().as_str())
}

impl SymbolTable {
    fn add_type_impl(&mut self, ident: Ident, ty: TypeKind) -> syn::Result<()> {
        self.types.try_insert(ident.clone(), ty).map_err(|entry| {
            syn::Error::new_spanned(entry.entry.get(), CheckError::TypeOrAlreadyDeclared(ident.to_string()))
        })?;
        Ok(())
    }
    pub fn add_ty_var(&mut self, meta_item: MetaItem) -> syn::Result<()> {
        assert!(matches!(meta_item.kind, MetaKind::Ty(_)));
        let ident = meta_item.ident.clone();
        self.ty_vars.try_insert(ident.clone(), meta_item).map_err(|entry| {
            syn::Error::new_spanned(entry.entry.get(), CheckError::TypeVarAlreadyDeclared(ident.to_string()))
        })?;
        Ok(())
    }
    pub fn add_type(&mut self, ident: Ident, ty: Type) -> syn::Result<()> {
        self.add_type_impl(ident, ty.into())
    }
    pub fn get_ty_var(&self, ident: &Ident) -> syn::Result<&MetaItem> {
        self.ty_vars
            .get(ident)
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::TypeVarNotDeclared(ident.to_string())))
    }
    pub fn get_type(&self, ident: &Ident) -> syn::Result<&TypeKind> {
        self.types
            .get(ident)
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::TypeOrPathNotDeclared(ident.to_string())))
    }
    pub fn add_local(&mut self, ident: Ident, ty: Type) {
        self.locals.insert(ident, ty);
    }
    pub fn get_local(&self, ident: &Ident) -> syn::Result<&Type> {
        self.locals
            .get(ident)
            .ok_or_else(|| syn::Error::new(ident.span(), CheckError::LocalNotDeclared(ident.to_string())))
    }
    pub fn add_path(&mut self, path: Path) -> syn::Result<()> {
        let ident = path
            .ident()
            .expect("invalid path without an identifier at the end")
            .clone();
        self.add_type_impl(ident, path.into())
    }
}
