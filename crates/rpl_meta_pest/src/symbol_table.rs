use std::ops::Deref;

use crate::error::{RPLMetaError, RPLMetaResult};
use crate::utils::{Ident, Path};
use derive_more::derive::From;
use parser::generics::Choice3;
use parser::{generics, pairs, rules, Grammar, PositionWrapper as Position, Rule, SpanWrapper};
use pest_typed::Span;
use rustc_hash::FxHashMap;
use rustc_span::Symbol;

#[derive(Clone, Copy, From)]
pub(crate) enum TypeOrPath<'a> {
    Type(&'a pairs::Type<'a>),
    Path(&'a pairs::Path<'a>),
}

impl<'a> TypeOrPath<'a> {
    pub fn span(&self) -> Span<'a> {
        match self {
            Self::Type(ty) => ty.span,
            Self::Path(path) => path.span,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ExportKind {
    Meta,
    Statement,
    SwitchTarget,
}

#[derive(Default)]
pub(crate) struct MetaTable<'a> {
    ty_vars: FxHashMap<Ident<'a>, &'a pairs::MetaVariableDecl<'a>>,
    // ident meta variables?
    exports: FxHashMap<Ident<'a>, ExportKind>,
}

impl<'a> MetaTable<'a> {
    pub fn add_ty_var(&mut self, ident: Ident<'a>, ty_var: &'a pairs::MetaVariableDecl<'a>) -> RPLMetaResult<()> {
        self.ty_vars
            .try_insert(ident, ty_var)
            .map_err(|entry| RPLMetaError::TypeVarAlreadyDeclared {
                span: entry.entry.key().span,
            })?;
        Ok(())
    }
    pub fn get_ty_var(&self, ident: Ident<'a>) -> RPLMetaResult<&'a pairs::MetaVariableDecl<'a>> {
        self.ty_vars
            .get(&ident)
            .copied()
            .ok_or_else(|| RPLMetaError::TypeVarNotDeclared { span: ident.span })
    }
    pub fn add_export(&mut self, export: Ident<'a>, kind: ExportKind) -> RPLMetaResult<()> {
        self.exports.try_insert(export, kind).map_err(|entry| {
            let ident = entry.entry.key();
            RPLMetaError::ExportAlreadyDeclared { span: ident.span }
        })?;
        Ok(())
    }
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
    structs: FxHashMap<Ident<'a>, Struct<'a>>,
    enums: FxHashMap<Ident<'a>, Enum<'a>>,
    fns: FxHashMap<Ident<'a>, Fn<'a>>,
    unnamed_fns: Vec<Fn<'a>>,
    impls: Vec<Impl<'a>>,
}

impl<'a> SymbolTable<'a> {
    /* pub fn add_enum(&mut self, ident: &'a Ident) -> syn::Result<&mut Enum<'a>> {
        self.enums.try_insert(ident, EnumInner::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            syn::Error::new(adt.span(), CheckError::SymbolAlreadyDeclared(SymbolKind::Enum, adt))
        })
    } */
    pub fn add_enum(&mut self, ident: Ident<'a>) -> RPLMetaResult<&mut Enum<'a>> {
        self.enums.try_insert(ident, EnumInner::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            RPLMetaError::SymbolAlreadyDeclared { span: adt.span }
        })
    }
    /* pub fn add_struct(&mut self, ident: &'a Ident) -> syn::Result<&mut Struct<'a>> {
        self.structs.try_insert(ident, Variant::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            syn::Error::new(adt.span(), CheckError::SymbolAlreadyDeclared(SymbolKind::Struct, adt))
        })
    } */
    pub fn add_struct(&mut self, ident: Ident<'a>) -> RPLMetaResult<&mut Struct<'a>> {
        self.structs.try_insert(ident, Variant::new().into()).map_err(|entry| {
            let adt = entry.entry.key();
            RPLMetaError::SymbolAlreadyDeclared { span: adt.span }
        })
    }

    pub fn add_fn(
        &mut self,
        ident: &'a pairs::FnName,
        self_ty: Option<&'a pairs::Type<'a>>,
    ) -> RPLMetaResult<&mut Fn<'a>> {
        match ident.deref() {
            Choice3::_0(_) => {
                self.unnamed_fns.push(FnInner::new(ident.span, self_ty).into());
                Ok(self.unnamed_fns.last_mut().unwrap())
            },
            Choice3::_1(ident) => Err(RPLMetaError::FnIdentMissingDollar { span: ident.span }),
            Choice3::_2(ident) => self
                .fns
                .try_insert(ident.into(), FnInner::new(ident.span, self_ty).into())
                .map_err(|entry| RPLMetaError::SymbolAlreadyDeclared { span: ident.span }),
        }
    }

    #[expect(unused)]
    pub fn add_impl(&mut self, impl_pat: &'a pairs::Impl) -> &mut Impl<'a> {
        self.impls.push(ImplInner::new(impl_pat).into());
        self.impls.last_mut().unwrap()
    }

    pub fn contains_adt(&self, ident: &Ident) -> bool {
        self.structs.contains_key(ident) || self.enums.contains_key(ident)
    }
}

pub(crate) type Enum<'a> = WithMetaTable<'a, EnumInner<'a>>;

pub(crate) struct EnumInner<'a> {
    variants: FxHashMap<Ident<'a>, Variant<'a>>,
}

impl<'a> EnumInner<'a> {
    fn new() -> Self {
        Self {
            variants: FxHashMap::default(),
        }
    }
    pub fn add_variant(&mut self, ident: Ident<'a>) -> RPLMetaResult<&mut Variant<'a>> {
        self.variants.try_insert(ident, Variant::new()).map_err(|entry| {
            let variant = entry.entry.key();
            RPLMetaError::SymbolAlreadyDeclared { span: ident.span }
        })
    }
}

pub(crate) struct Variant<'a> {
    fields: FxHashMap<Ident<'a>, &'a pairs::Type<'a>>,
}

impl<'a> Variant<'a> {
    fn new() -> Self {
        Self {
            fields: FxHashMap::default(),
        }
    }
    pub fn add_field(&mut self, ident: Ident<'a>, ty: &'a pairs::Type<'a>) -> RPLMetaResult<()> {
        self.fields.try_insert(ident, ty).map_err(|entry| {
            let field = entry.entry.key();
            RPLMetaError::SymbolAlreadyDeclared { span: field.span }
        })?;
        Ok(())
    }
}

pub(crate) type Struct<'a> = WithMetaTable<'a, Variant<'a>>;

pub(crate) type Fn<'a> = WithMetaTable<'a, FnInner<'a>>;

pub(crate) struct FnInner<'a> {
    span: Span<'a>,
    types: FxHashMap<Ident<'a>, TypeOrPath<'a>>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<&'a pairs::Type<'a>>,
    self_param: Option<&'a pairs::SelfParam<'a>>,
    self_ty: Option<&'a pairs::Type<'a>>,
    params: FxHashMap<Ident<'a>, &'a pairs::Type<'a>>,
    locals: FxHashMap<Ident<'a>, Vec<&'a pairs::Type<'a>>>,
}

impl<'a> FnInner<'a> {
    fn new(span: Span<'a>, self_ty: Option<&'a pairs::Type<'a>>) -> Self {
        Self {
            span,
            types: FxHashMap::default(),
            self_value: None,
            self_param: None,
            self_ty,
            params: FxHashMap::default(),
            locals: FxHashMap::default(),
        }
    }
    fn add_type_impl(&mut self, ident: Ident<'a>, ty: TypeOrPath<'a>) -> RPLMetaResult<()> {
        self.types
            .try_insert(ident, ty)
            .map_err(|entry| RPLMetaError::TypeOrPathAlreadyDeclared { span: ty.span() })?;
        Ok(())
    }
    pub fn add_type(&mut self, ident: Ident<'a>, ty: &'a pairs::Type<'a>) -> RPLMetaResult<()> {
        self.add_type_impl(ident, ty.into())
    }
    pub fn get_type(&self, ident: &Ident<'a>) -> RPLMetaResult<TypeOrPath<'a>> {
        self.types
            .get(ident)
            .copied()
            .ok_or_else(|| RPLMetaError::TypeOrPathNotDeclared { span: ident.span })
    }
    pub fn add_path(&mut self, path: &'a pairs::Path<'a>) -> RPLMetaResult<()> {
        let ty_or_path = path.into();
        let path: Path<'a> = path.into();
        let ident = path.segments.last().unwrap();
        self.add_type_impl(*ident, ty_or_path)
    }
}

impl<'a> FnInner<'a> {
    pub fn add_self_param(&mut self, self_param: &'a pairs::SelfParam<'a>) -> RPLMetaResult<()> {
        if self.self_param.is_some() {
            return Err(RPLMetaError::SelfAlreadyDeclared { span: self_param.span });
        }
        self.self_param = Some(self_param);
        Ok(())
    }
    pub fn add_param(&mut self, ident: Ident<'a>, ty: &'a pairs::Type<'a>) -> RPLMetaResult<()> {
        self.params.try_insert(ident, ty).map_err(|entry| {
            let param = entry.entry.key();
            RPLMetaError::SymbolAlreadyDeclared { span: param.span }
        })?;
        Ok(())
    }
    pub fn add_local(&mut self, ident: Ident<'a>, ty: &'a pairs::Type<'a>) -> RPLMetaResult<()> {
        self.locals.entry(ident).or_default().push(ty);
        Ok(())
    }
    pub fn add_place_local(&mut self, local: &'a pairs::MirPlaceLocal, ty: &'a pairs::Type<'a>) -> RPLMetaResult<()> {
        match local.deref() {
            Choice3::_0(_self_value) => {
                self.self_value = Some(ty);
                Ok(())
            },
            Choice3::_1(ident) => self.add_local(ident.into(), ty),
            Choice3::_2(_) => {
                todo!("place_holder in place_local");
            },
        }
    }
    fn get_local_impl(&self, ident: Ident<'a>) -> Option<&'a pairs::Type<'a>> {
        self.locals
            .get(&ident)
            .and_then(|types| types.last())
            .or_else(|| self.params.get(&ident))
            .copied()
    }
    pub fn get_local(&self, ident: Ident<'a>) -> RPLMetaResult<&'a pairs::Type<'a>> {
        self.get_local_impl(ident)
            .ok_or_else(|| RPLMetaError::SymbolNotDeclared { span: ident.span })
    }
    pub fn get_place_local(&self, local: &'a pairs::MirPlaceLocal) -> RPLMetaResult<&'a pairs::Type<'a>> {
        /* match local {
            PlaceLocal::Local(ident) => self.get_local(ident),
            PlaceLocal::SelfValue(self_value) if self.self_value.is_none() && self.self_param.is_none() => {
                Err(syn::Error::new(self_value.span, CheckError::SelfNotDeclared))
            },
            PlaceLocal::SelfValue(self_value) => self
                .self_value
                .map(|(_, ty)| ty)
                .or(self.self_ty)
                .ok_or_else(|| syn::Error::new(self_value.span, CheckError::SelfTypeOutsideImpl)),
        } */
        match local.deref() {
            Choice3::_0(_self_value) if self.self_value.is_none() && self.self_param.is_none() => {
                Err(RPLMetaError::SelfNotDeclared {})
            },
            Choice3::_0(_self_value) => self
                .self_value
                .or(self.self_ty)
                .ok_or_else(|| RPLMetaError::SelfTypeOutsideImpl {}),
            Choice3::_1(ident) => self.get_local(ident.into()),
            Choice3::_2(_) => {
                todo!("place_holder in place_local");
            },
        }
    }
}

pub(crate) type Impl<'a> = WithMetaTable<'a, ImplInner<'a>>;

pub(crate) struct ImplInner<'a> {
    #[expect(unused)]
    trait_: Option<&'a pairs::Path<'a>>,
    ty: &'a pairs::Type<'a>,
    fns: FxHashMap<Ident<'a>, FnInner<'a>>,
}

impl<'a> ImplInner<'a> {
    pub fn new(impl_pat: &'a pairs::Impl) -> Self {
        let impl_pat = impl_pat.get_matched();
        let trait_ = match impl_pat.1 {
            Some(trait_) => Some(trait_.get_matched().0),
            None => None,
        };
        Self {
            trait_,
            ty: impl_pat.2,
            fns: FxHashMap::default(),
        }
    }
}

impl<'a> ImplInner<'a> {
    pub fn add_fn(&mut self, ident: Ident<'a>, fn_def: FnInner<'a>) -> RPLMetaResult<&mut FnInner<'a>> {
        self.fns
            .try_insert(ident, fn_def)
            .map_err(|entry| RPLMetaError::MethodAlreadyDeclared { span: ident.span })
    }
}

static PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool", "str",
];

pub(crate) fn is_primitive(ident: &Ident) -> bool {
    PRIMITIVES.contains(&ident.name.to_string().as_str())
}
