use std::ops::Deref;
use std::sync::Arc;

use derive_more::derive::{AsRef, Debug, From};
pub use diag::DiagSymbolTable;
use either::Either;
use parser::generics::{Choice3, Choice4};
use parser::{SpanWrapper, pairs};
use pest_typed::{Span, Spanned};
use rpl_constraints::predicates::PredicateConjunction;
use rustc_data_structures::fx::FxHashMap;
use rustc_middle::mir;

use crate::FlatMap;
use crate::check::CheckCtxt;
use crate::context::MetaContext;
use crate::error::{RPLMetaError, RPLMetaResult};
use crate::utils::{Path, self_param_ty};

pub(crate) mod diag;

#[derive(Clone, Copy, From, Debug)]
pub enum TypeOrPath<'i> {
    Type(&'i pairs::Type<'i>),
    Path(&'i pairs::Path<'i>),
}

impl<'i> TypeOrPath<'i> {
    pub fn span(&self) -> Span<'i> {
        match self {
            Self::Type(ty) => ty.span,
            Self::Path(path) => path.span,
        }
    }

    pub fn try_as_path(&self) -> Option<&'i pairs::Path<'i>> {
        match &self {
            Self::Type(ty) if let Some(type_path) = ty.TypePath() => {
                // FIXME: Qself is dropped
                Some(type_path.Path())
            },
            Self::Path(path) => Some(path),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaVariableType {
    Type,
    Const,
    Place,
}

#[derive(Clone, Copy, Debug)]
pub enum AdtPatType {
    Struct,
    Enum,
}

pub type Type<'i> = &'i pairs::Type<'i>;

// the usize in the hashmap is the *-index of a non-local meta variable
// PredicateConjunction is the predicates after the meta variable declaration
// (These predicates should have only one parameter, which is the meta variable itself)
// like `$T: type where is_all_safe_trait(self) && !is_primitive(self)`
#[derive(Default, Debug)]
pub struct NonLocalMetaSymTab<'i> {
    type_vars: FlatMap<&'i str, (usize, PredicateConjunction)>,
    const_vars: FlatMap<&'i str, (usize, Type<'i>, PredicateConjunction)>,
    place_vars: FlatMap<&'i str, (usize, Type<'i>, PredicateConjunction)>,
}

impl<'i> NonLocalMetaSymTab<'i> {
    pub fn type_vars(&self) -> impl Iterator<Item = (&'i str, usize)> {
        self.type_vars.iter().map(|(symbol, (idx, _))| (*symbol, *idx))
    }
    pub fn const_vars(&self) -> impl Iterator<Item = (&'i str, usize)> {
        self.const_vars.iter().map(|(symbol, (idx, _, _))| (*symbol, *idx))
    }
    pub fn place_vars_map(&self) -> &FlatMap<&'i str, (usize, &pairs::Type<'_>, PredicateConjunction)> {
        &self.place_vars
    }
    pub fn place_vars(&self) -> impl Iterator<Item = (&'i str, usize)> {
        self.place_vars.iter().map(|(symbol, (idx, _, _))| (*symbol, *idx))
    }
}

impl<'i> NonLocalMetaSymTab<'i> {
    pub fn add_non_local_meta_var(
        &mut self,
        mctx: &MetaContext<'i>,
        meta_var: &pairs::MetaVariable<'i>,
        meta_var_ty: &'i pairs::MetaVariableType<'i>,
        preds: PredicateConjunction,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        match meta_var_ty.deref() {
            Choice3::_0(_) => {
                let existed = self
                    .type_vars
                    .insert(meta_var.span.as_str(), (self.type_vars.len(), preds));
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.span.as_str(),
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
            Choice3::_1(kind) => {
                let (_, _, ty, _) = kind.get_matched();
                let existed = self
                    .const_vars
                    .insert(meta_var.span.as_str(), (self.const_vars.len(), ty, preds));
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.span.as_str(),
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
            Choice3::_2(kind) => {
                let (_, _, ty, _) = kind.get_matched();
                let existed = self
                    .place_vars
                    .insert(meta_var.span.as_str(), (self.place_vars.len(), ty, preds));
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.span.as_str(),
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
        }
    }

    pub fn get_non_local_meta_var(
        &self,
        mctx: &MetaContext<'i>,
        meta_var: &pairs::MetaVariable<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<Either<MetaVariableType, AdtPatType>> {
        self.get_meta_var_from_name(meta_var.span.as_str())
            .map(|var| var.ty())
            .or_else(|| {
                let err = RPLMetaError::NonLocalMetaVariableNotDeclared {
                    meta_var: meta_var.span.as_str(),
                    span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                };
                errors.push(err);
                None
            })
    }

    pub fn force_non_local_meta_var(&self, meta_var: WithPath<'i, &'i pairs::MetaVariable<'i>>) -> MetaVariable<'i> {
        let symbol = meta_var.span.as_str();
        self.get_meta_var_from_name(symbol).unwrap_or_else(|| {
            let err = RPLMetaError::NonLocalMetaVariableNotDeclared {
                meta_var: symbol,
                span: SpanWrapper::new(meta_var.span, meta_var.path),
            };
            panic!("{err}");
        })
    }

    #[allow(clippy::manual_map)]
    pub fn get_meta_var_from_name(&self, name: &str) -> Option<MetaVariable<'i>> {
        if let Some((idx, preds)) = self.type_vars.get(&name) {
            Some(MetaVariable::Type(*idx, preds.clone()))
        } else if let Some((idx, ty, preds)) = self.const_vars.get(&name) {
            Some(MetaVariable::Const(*idx, ty, preds.clone()))
        } else if let Some((idx, ty, preds)) = self.place_vars.get(&name) {
            Some(MetaVariable::Place(*idx, ty, preds.clone()))
        } else {
            None
        }
    }
}

#[derive(Debug, AsRef)]
pub struct WithMetaTable<'i, T> {
    #[as_ref]
    pub meta_vars: Arc<NonLocalMetaSymTab<'i>>,
    /// Don't modify this field directly, use [`SymbolTable::adt_pats`] instead.
    /// FIXME
    pub adt_pats: Box<AdtPats<'i>>,
    pub inner: T,
}

impl<'i, T> From<(T, Arc<NonLocalMetaSymTab<'i>>, &AdtPats<'i>)> for WithMetaTable<'i, T> {
    fn from(inner: (T, Arc<NonLocalMetaSymTab<'i>>, &AdtPats<'i>)) -> Self {
        Self {
            meta_vars: inner.1,
            adt_pats: Box::from(inner.2.clone()),
            inner: inner.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[debug("{inner:?}")]
#[debug(bound(T: std::fmt::Debug))]
pub struct WithPath<'i, T> {
    pub path: &'i std::path::Path,
    pub inner: T,
}

impl<T> Deref for WithPath<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'i, T> WithPath<'i, T> {
    pub fn new(path: &'i std::path::Path, inner: T) -> Self {
        Self { path, inner }
    }
    pub(crate) fn with_ctx(ctx: &MetaContext<'i>, inner: T) -> Self {
        Self {
            path: ctx.get_active_path(),
            inner,
        }
    }
    pub fn map<U, F>(&self, f: F) -> WithPath<'i, U>
    where
        F: FnOnce(&T) -> U,
    {
        let path = self.path;
        let inner = f(&self.inner);
        WithPath { path, inner }
    }
}

impl<'i, T: Spanned<'i, parser::Rule>> From<WithPath<'i, &T>> for SpanWrapper<'i> {
    fn from(with_path: WithPath<'i, &T>) -> Self {
        SpanWrapper::new(with_path.inner.span(), with_path.path)
    }
}

impl<'i, T: Spanned<'i, parser::Rule>> From<&WithPath<'i, &T>> for SpanWrapper<'i> {
    fn from(with_path: &WithPath<'i, &T>) -> Self {
        SpanWrapper::new(with_path.inner.span(), with_path.path)
    }
}

#[macro_export]
macro_rules! map_inner {
    ($with_path:ident => $expr:expr) => {
        $crate::symbol_table::WithPath::map($with_path, |with_path| $expr)
    };
}

pub type Imports<'i> = FxHashMap<&'i str, &'i pairs::Path<'i>>;
pub type AdtPats<'i> = FlatMap<&'i str, AdtPatType>;

#[derive(Default, AsRef)]
pub struct SymbolTable<'i> {
    // meta variables in p[$T: ty]
    #[as_ref]
    pub meta_vars: Arc<NonLocalMetaSymTab<'i>>,
    pub adt_pats: AdtPats<'i>,
    /// Should be inserted into [`FnInner::types`].
    ///
    /// See [`SymbolTable::imports`].
    pub(crate) imports: Imports<'i>,
    structs: FxHashMap<&'i str, Struct<'i>>,
    enums: FxHashMap<&'i str, Enum<'i>>,
    fns: FxHashMap<&'i str, Fn<'i>>,
    unnamed_fns: Vec<Fn<'i>>,
    impls: FxHashMap<(&'i pairs::Type<'i>, Option<&'i pairs::ImplKind<'i>>), Impl<'i>>,
}

impl<'i> SymbolTable<'i> {
    pub fn add_enum(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &pairs::MetaVariable<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Enum<'i>> {
        self.enums
            .try_insert(
                ident.span.as_str(),
                (EnumInner::new(), self.meta_vars.clone(), &self.adt_pats).into(),
            )
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: adt,
                    span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }

    pub fn add_struct(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &pairs::MetaVariable<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Struct<'i>> {
        self.structs
            .try_insert(
                ident.span.as_str(),
                (StructInner::new(), self.meta_vars.clone(), &self.adt_pats).into(),
            )
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: adt,
                    span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }

    pub fn add_fn(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &'i pairs::FnName<'i>,
        self_ty: Option<&'i pairs::Type<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<(&mut Fn<'i>, &Imports<'i>, &AdtPats<'i>)> {
        let (fn_name, fn_def) = FnInner::parse_from(mctx, ident, self_ty);
        let imports = &self.imports;
        if let Some(fn_name) = fn_name {
            self.fns
                .try_insert(
                    fn_name.span().as_str(),
                    (fn_def, self.meta_vars.clone(), &self.adt_pats).into(),
                )
                .map_err(|entry| {
                    let ident = entry.entry.key();
                    let err = RPLMetaError::SymbolAlreadyDeclared {
                        ident,
                        span: SpanWrapper::new(fn_name.span(), mctx.get_active_path()),
                    };
                    errors.push(err);
                })
                .ok()
        } else {
            self.unnamed_fns
                .push((fn_def, self.meta_vars.clone(), &self.adt_pats).into());
            Some(self.unnamed_fns.last_mut().unwrap())
        }
        //FIXME: this is a hack to borrow the imports from the symbol table
        .map(|fn_inner| (fn_inner, imports, &self.adt_pats))
    }

    /// See [`SymbolTable::get_impl`].
    pub fn add_impl(
        &mut self,
        mctx: &MetaContext<'i>,
        impl_pat: &'i pairs::Impl<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<(&mut Impl<'i>, &Imports<'i>, &AdtPats<'i>)> {
        self.impls
            .try_insert(
                (impl_pat.Type(), impl_pat.ImplKind()),
                (ImplInner::new(impl_pat), self.meta_vars.clone(), &self.adt_pats).into(),
            )
            .map_err(|_| {
                let err = RPLMetaError::ImplAlreadyDeclared {
                    span: SpanWrapper::new(impl_pat.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
            //FIXME: this is a hack to borrow the imports from the symbol table
            .map(|impl_inner| (impl_inner, &self.imports, &self.adt_pats))
    }

    // pub fn contains_adt(&self, ident: &Ident<'_>) -> bool {
    //     self.structs.contains_key(&ident.name) || self.enums.contains_key(&ident.name)
    // }

    pub fn add_adt_pat(
        &mut self,
        mctx: &MetaContext<'i>,
        meta_var: &pairs::MetaVariable<'i>,
        adt_pat_ty: AdtPatType,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<()> {
        self.adt_pats
            .try_insert(meta_var.span.as_str(), adt_pat_ty)
            .map_err(|_| {
                let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                    meta_var: meta_var.span.as_str(),
                    span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .map(|_| ())
            .ok()
    }

    pub fn get_adt(&self, symbol: &'i str) -> Option<(AdtPatType, &'i str)> {
        if self.structs.contains_key(&symbol) {
            Some((AdtPatType::Struct, symbol))
        } else if self.enums.contains_key(&symbol) {
            Some((AdtPatType::Enum, symbol))
        } else {
            None
        }
    }

    pub fn labels(&self) -> impl Iterator<Item = &'i str> {
        self.fns
            .values()
            .chain(self.unnamed_fns.iter())
            .flat_map(|f| f.inner.locals.iter().filter_map(|l| l.1.0))
    }
}

impl<'i> SymbolTable<'i> {
    pub fn collect_symbol_tables(
        mctx: &MetaContext<'i>,
        pat_imports: &[&'i pairs::UsePath<'i>],
        pat_items: impl Iterator<Item = &'i pairs::RPLPatternItem<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> FlatMap<&'i str, Self> {
        let mut symbol_tables = FlatMap::default();
        for pat_item in pat_items {
            //FIXME: maybe check whether the key exists before collecting the symbol table?
            let CheckCtxt {
                name,
                symbol_table: symbols,
                errors: error_vec,
            } = Self::collect_symbol_table(mctx, pat_imports, pat_item);
            debug!(?name, imports = ?symbols.imports.keys(), meta = ?symbols.meta_vars);
            errors.extend(error_vec);
            _ = symbol_tables.try_insert(name, symbols).map_err(|entry| {
                let name = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: name,
                    span: SpanWrapper::new(pat_item.Identifier().span, mctx.get_active_path()),
                };
                errors.push(err);
            });
        }
        symbol_tables
    }

    fn collect_symbol_table(
        mctx: &MetaContext<'i>,
        imports: &[&'i pairs::UsePath<'i>],
        pat_item: &'i pairs::RPLPatternItem<'i>,
    ) -> CheckCtxt<'i> {
        let pat_item_name = pat_item.Identifier().span.as_str();
        let mut cctx = CheckCtxt::new(pat_item_name);

        for import in imports {
            cctx.check_import(mctx, import);
        }
        cctx.check_pat_item(mctx, pat_item);
        cctx
    }
}

impl<'i> SymbolTable<'i> {
    pub fn get_fn(&self, name: &'i str) -> Option<&Fn<'i>> {
        // FIXME
        if name == "_" {
            return self.unnamed_fns.last();
        }
        self.fns.get(&name)
    }
    /// See [`SymbolTable::add_impl`].
    pub fn get_impl(&self, ty: &'i pairs::Type<'i>, impl_kind: Option<&'i pairs::ImplKind<'i>>) -> Option<&Impl<'i>> {
        // FIXME: how to identify an impl?
        self.impls.get(&(ty, impl_kind))
    }
}

pub type Enum<'i> = WithMetaTable<'i, EnumInner<'i>>;

pub struct EnumInner<'i> {
    variants: FxHashMap<&'i str, Variant<'i>>,
}

impl<'i> EnumInner<'i> {
    fn new() -> Self {
        Self {
            variants: FxHashMap::default(),
        }
    }
    pub fn add_variant(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &pairs::Identifier<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Variant<'i>> {
        self.variants
            .try_insert(ident.span.as_str(), Variant::new())
            .map_err(|_entry| {
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: ident.span.as_str(),
                    span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }
}

pub struct Variant<'i> {
    fields: FxHashMap<&'i str, &'i pairs::Type<'i>>,
}

impl<'i> Variant<'i> {
    fn new() -> Self {
        Self {
            fields: FxHashMap::default(),
        }
    }
    pub fn add_field(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &pairs::MetaVariable<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.fields.try_insert(ident.span.as_str(), ty).map_err(|_entry| {
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: ident.span.as_str(),
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
}

pub type Fn<'i> = WithMetaTable<'i, FnInner<'i>>;

#[derive(Clone, Copy)]
pub enum LocalSpecial {
    None,
    // FIXME: handle this
    Arg,
    Self_,
    Return,
}

/// Implicit `self` parameter type in a function.
#[derive(Clone, Copy, Debug)]
pub enum SelfType {
    Value(mir::Mutability),
    Ref(mir::Mutability),
}

pub(crate) enum FnName<'i> {
    MetaVariable(&'i pairs::MetaVariable<'i>),
    Identifier(&'i pairs::Identifier<'i>),
}

impl<'i> Spanned<'i, parser::Rule> for FnName<'i> {
    fn span(&self) -> Span<'i> {
        match self {
            Self::MetaVariable(mv) => mv.span,
            Self::Identifier(ident) => ident.span,
        }
    }
}

pub struct FnInner<'i> {
    #[expect(unused)]
    span: Span<'i>,
    path: &'i std::path::Path,
    /// - Type aliases declared in the function scope.
    /// - Paths imported into the function scope.
    types: FlatMap<&'i str, TypeOrPath<'i>>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<&'i pairs::Type<'i>>,
    ret_value: Option<&'i pairs::Type<'i>>,
    self_param: Option<&'i pairs::SelfParam<'i>>,
    self_ty: Option<&'i pairs::Type<'i>>,
    params: FlatMap<&'i str, (usize, &'i pairs::Type<'i>)>,
    locals: FlatMap<&'i str, (Option<&'i str>, usize, &'i pairs::Type<'i>, LocalSpecial)>,
    pub symbol_to_local_idx: FlatMap<&'i str, usize>,
}

impl<'i> FnInner<'i> {
    fn new(span: Span<'i>, path: &'i std::path::Path, self_ty: Option<&'i pairs::Type<'i>>) -> Self {
        Self {
            span,
            path,
            // types: imports.iter().map(|(&k, v)| (k, TypeOrPath::Path(v))).collect(),
            types: FlatMap::default(),
            self_value: None,
            ret_value: None,
            self_param: None,
            self_ty,
            params: FlatMap::default(),
            locals: FlatMap::default(),
            symbol_to_local_idx: FlatMap::default(),
        }
    }
    pub(crate) fn parse_from(
        mctx: &MetaContext<'i>,
        fn_name: &'i pairs::FnName<'i>,
        self_ty: Option<&'i pairs::Type<'i>>,
    ) -> (Option<FnName<'i>>, Self) {
        match fn_name.deref() {
            Choice3::_0(_) => (None, Self::new(fn_name.span, mctx.get_active_path(), self_ty)),
            Choice3::_1(meta_var) => (
                Some(FnName::MetaVariable(meta_var)),
                FnInner::new(meta_var.span, mctx.get_active_path(), self_ty),
            ),
            Choice3::_2(ident) => (
                Some(FnName::Identifier(ident)),
                FnInner::new(ident.span, mctx.get_active_path(), self_ty),
            ),
        }
    }
    #[instrument(level = "trace", skip_all, fields(types = ?self.types.keys(), ident = ?ident.span.as_str(), ty = ?ty.span().as_str()))]
    pub(crate) fn add_type_impl(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &pairs::Identifier<'i>,
        ty: TypeOrPath<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.types.try_insert(ident.span.as_str(), ty).map_err(|entry| {
            let err = RPLMetaError::TypeOrPathAlreadyDeclared {
                type_or_path: ident.span.as_str(),
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                span_previous: SpanWrapper::new(entry.entry.get().span(), mctx.get_active_path()),
            };
            errors.push(err);
        });
    }

    /// Resolve an identifier to a type or path.
    #[instrument(level = "trace", skip(self, path), fields(types = ?self.types.keys()))]
    fn get_type_or_path(
        &self,
        path: &'i std::path::Path,
        ident: &pairs::Identifier<'i>,
    ) -> Result<TypeOrPath<'i>, RPLMetaError<'i>> {
        self.types
            .get(&ident.span.as_str())
            .copied()
            .ok_or_else(|| RPLMetaError::TypeOrPathNotDeclared {
                span: SpanWrapper::new(ident.span, path),
                type_or_path: ident.span.as_str(),
                declared: self.types.keys().cloned().collect(),
            })
    }

    pub fn add_import(
        &mut self,
        mctx: &MetaContext<'i>,
        path: &'i pairs::Path<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        let ty_or_path = path.into();
        let path: Path<'i> = path.into();
        let ident = path.ident();
        self.add_type_impl(mctx, ident, ty_or_path, errors);
    }

    pub fn get_params(&self) -> WithPath<'i, Vec<(&'i str, &'i pairs::Type<'i>)>> {
        let mut params = self
            .params
            .iter()
            .map(|(ident, (idx, ty))| (ident, (idx, ty)))
            .collect::<Vec<_>>();
        params.sort_by_key(|(_, (idx, _))| *idx);
        WithPath::new(
            self.path,
            params.into_iter().map(|(ident, (_, ty))| (*ident, *ty)).collect(),
        )
    }

    #[expect(clippy::type_complexity)]
    pub fn get_sorted_locals(
        &self,
    ) -> WithPath<'i, Vec<(Option<&'i str>, &'i str, &'i pairs::Type<'i>, LocalSpecial)>> {
        let mut locals = self
            .locals
            .iter()
            .map(|(ident, (label, idx, ty, s))| (ident, (label, idx, ty, s)))
            .collect::<Vec<_>>();
        locals.sort_by_key(|(_, (_, idx, _, _))| *idx);
        WithPath::new(
            self.path,
            locals
                .into_iter()
                .map(|(ident, (label, _, ty, s))| (*label, *ident, *ty, *s))
                .collect(),
        )
    }

    pub fn try_get_local_idx(&self, symbol: &'i str) -> Option<usize> {
        self.symbol_to_local_idx.get(&symbol).copied()
    }
    pub fn get_local_idx(&self, symbol: &'i str) -> usize {
        self.symbol_to_local_idx.get(&symbol).copied().unwrap_or_else(|| {
            panic!("Local variable `{}` not found", symbol);
        }) // Should not panic
    }
}

impl<'i> FnInner<'i> {
    pub fn add_self_param(
        &mut self,
        mctx: &MetaContext<'i>,
        self_param: &'i pairs::SelfParam<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        if self.self_param.is_some() {
            errors.push(RPLMetaError::SelfAlreadyDeclared {
                span: SpanWrapper::new(self_param.span, mctx.get_active_path()),
            });
        }
        self.self_param = Some(self_param);
        let ty = self_param_ty(self_param).0;
        self.add_param(mctx, Some(self_param.span().as_str()), self_param, ty, errors);
    }
    pub fn add_param(
        &mut self,
        mctx: &MetaContext<'i>,
        label: Option<&'i str>,
        ident: &impl Spanned<'i, parser::Rule>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        let idx = self.add_local(mctx, label, ident, ty, LocalSpecial::Arg, errors);
        _ = self
            .params
            .try_insert(ident.span().as_str(), (idx, ty))
            .map_err(|_entry| {
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: ident.span().as_str(),
                    span: SpanWrapper::new(ident.span(), mctx.get_active_path()),
                };
                errors.push(err);
            });
    }
    pub fn add_local(
        &mut self,
        mctx: &MetaContext<'i>,
        label: Option<&'i str>,
        ident: &impl Spanned<'i, parser::Rule>,
        ty: &'i pairs::Type<'i>,
        special: LocalSpecial,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> usize {
        let len = self.locals.len();
        match self.locals.try_insert(ident.span().as_str(), (label, len, ty, special)) {
            Ok(_) => {
                self.symbol_to_local_idx.insert(ident.span().as_str(), len);
            },
            Err(_) => {
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: ident.span().as_str(),
                    span: SpanWrapper::new(ident.span(), mctx.get_active_path()),
                };
                errors.push(err);
            },
        }
        // FIXME: this is a hack to return the index of the local variable
        len
    }
    pub fn add_place_local(
        &mut self,
        mctx: &MetaContext<'i>,
        label: Option<&'i str>,
        local: &'i pairs::MirPlaceLocal<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        match local.deref() {
            Choice4::_0(_place_holder) => {},
            Choice4::_1(self_value) => {
                if self.self_value.is_some() {
                    errors.push(RPLMetaError::SelfAlreadyDeclared {
                        span: SpanWrapper::new(local.span, mctx.get_active_path()),
                    });
                } else {
                    self.self_value = Some(ty);
                    self.add_local(mctx, label, self_value, ty, LocalSpecial::Self_, errors);
                }
            },
            Choice4::_2(ret_value) => {
                if self.self_value.is_some() {
                    errors.push(RPLMetaError::RetAlreadyDeclared {
                        span: SpanWrapper::new(local.span, mctx.get_active_path()),
                    });
                } else {
                    self.ret_value = Some(ty);
                    self.add_local(mctx, label, ret_value, ty, LocalSpecial::Return, errors);
                }
            },
            Choice4::_3(meta_var) => {
                self.add_local(mctx, label, meta_var, ty, LocalSpecial::None, errors);
            },
        }
    }
    fn get_place_impl(
        &self,
        ident: &impl Spanned<'i, parser::Rule>,
        meta_vars: &NonLocalMetaSymTab<'i>,
    ) -> Option<&'i pairs::Type<'i>> {
        meta_vars
            .place_vars
            .get(&ident.span().as_str())
            .map(|(_, ty, _)| ty)
            .copied()
    }
    fn get_local_impl(&self, ident: &impl Spanned<'i, parser::Rule>) -> Option<&'i pairs::Type<'i>> {
        self.locals
            .get(&ident.span().as_str())
            .map(|(_label, _idx, ty, _)| ty)
            // .or_else(|| self.params.get(&ident.name).map(|(_idx, ty)| ty))
            .copied()
    }
    pub fn get_place_or_local(
        &self,
        mctx: &MetaContext<'i>,
        ident: &impl Spanned<'i, parser::Rule>,
        meta_vars: &NonLocalMetaSymTab<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&'i pairs::Type<'i>> {
        self.get_local_impl(ident)
            .or_else(|| self.get_place_impl(ident, meta_vars))
            .or_else(|| {
                let err = RPLMetaError::SymbolNotDeclared {
                    ident: ident.span().as_str(),
                    span: SpanWrapper::new(ident.span(), mctx.get_active_path()),
                };
                errors.push(err);
                None
            })
    }
    pub fn get_place_local(
        &self,
        mctx: &MetaContext<'i>,
        local: &'i pairs::MirPlaceLocal<'i>,
        meta_vars: &NonLocalMetaSymTab<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&'i pairs::Type<'i>> {
        match local.deref() {
            Choice4::_0(_place_holder) => None,
            Choice4::_2(_ret_value) => self.ret_value.or_else(|| {
                errors.push(RPLMetaError::RetNotDeclared {
                    span: SpanWrapper::new(local.span, mctx.get_active_path()),
                });
                None
            }),

            Choice4::_3(meta_var) => self.get_place_or_local(mctx, meta_var, meta_vars, errors),
            Choice4::_1(_) if self.self_value.is_none() && self.self_param.is_none() => {
                errors.push(RPLMetaError::SelfNotDeclared {
                    span: SpanWrapper::new(local.span, mctx.get_active_path()),
                });
                None
            },
            Choice4::_1(_) => self.self_value.or(self.self_ty).or_else(|| {
                errors.push(RPLMetaError::SelfTypeOutsideImpl {
                    span: SpanWrapper::new(local.span, mctx.get_active_path()),
                });
                None
            }),
        }
    }
}

pub type Struct<'i> = WithMetaTable<'i, StructInner<'i>>;

pub type StructInner<'pcx> = Variant<'pcx>;

pub type Impl<'i> = WithMetaTable<'i, ImplInner<'i>>;

pub struct ImplInner<'i> {
    #[allow(dead_code)]
    trait_: Option<&'i pairs::Path<'i>>,
    #[allow(dead_code)]
    ty: &'i pairs::Type<'i>,
    fns: FxHashMap<&'i str, Fn<'i>>,
}

impl<'i> ImplInner<'i> {
    pub fn new(impl_pat: &'i pairs::Impl<'i>) -> Self {
        let impl_pat = impl_pat.get_matched();
        let trait_ = impl_pat.2.as_ref().map(|trait_| trait_.get_matched().0);
        Self {
            trait_,
            ty: impl_pat.3,
            fns: FxHashMap::default(),
        }
    }
}

impl<'i> ImplInner<'i> {
    pub fn add_fn(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: &impl Spanned<'i, parser::Rule>,
        fn_def: Fn<'i>,
    ) -> RPLMetaResult<'i, &mut Fn<'i>> {
        self.fns
            .try_insert(ident.span().as_str(), fn_def)
            .map_err(|_entry| RPLMetaError::MethodAlreadyDeclared {
                span: SpanWrapper::new(ident.span(), mctx.get_active_path()),
            })
    }
}

impl<'i> ImplInner<'i> {
    pub fn get_fn(&self, name: &'i str) -> Option<&Fn<'i>> {
        self.fns.get(&name)
    }
}

static PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool", "str",
];

pub(crate) fn ident_is_primitive(ident: &str) -> bool {
    PRIMITIVES.contains(&ident)
}

pub(crate) fn str_is_primitive(ident: &str) -> bool {
    PRIMITIVES.contains(&ident)
}

pub enum MetaVariable<'i> {
    Type(usize, PredicateConjunction),
    Const(usize, &'i pairs::Type<'i>, PredicateConjunction),
    Place(usize, &'i pairs::Type<'i>, PredicateConjunction),
    AdtPat(AdtPatType, &'i str),
}

impl<'i> MetaVariable<'i> {
    pub fn ty(&self) -> Either<MetaVariableType, AdtPatType> {
        match self {
            MetaVariable::Type(_, _) => Either::Left(MetaVariableType::Type),
            MetaVariable::Const(_, _, _) => Either::Left(MetaVariableType::Const),
            MetaVariable::Place(_, _, _) => Either::Left(MetaVariableType::Place),
            MetaVariable::AdtPat(kind, _) => Either::Right(*kind),
        }
    }
    pub fn expect_const(self) -> (usize, &'i pairs::Type<'i>, PredicateConjunction) {
        match self {
            MetaVariable::Type(_, _) => panic!("Expected type meta variable, found ADT"),
            MetaVariable::Const(idx, ty, pred) => (idx, ty, pred),
            MetaVariable::Place(_, _, _) => panic!("Expected place meta variable, found ADT"),
            MetaVariable::AdtPat(_, _) => panic!("Expected const meta variable, found ADT"),
        }
    }
    pub fn expect_non_adt(self) -> (MetaVariableType, usize, PredicateConjunction) {
        match self {
            MetaVariable::Type(idx, pred) => (MetaVariableType::Type, idx, pred),
            MetaVariable::Const(idx, _, pred) => (MetaVariableType::Const, idx, pred),
            MetaVariable::Place(idx, _, pred) => (MetaVariableType::Place, idx, pred),
            MetaVariable::AdtPat(_, _) => panic!("Expected non-ADT meta variable, found ADT"),
        }
    }
}

impl<'i> From<(AdtPatType, &'i str)> for MetaVariable<'i> {
    fn from((kind, symbol): (AdtPatType, &'i str)) -> Self {
        MetaVariable::AdtPat(kind, symbol)
    }
}

#[instrument(level = "trace", skip(ident), fields(ident = ident.inner.span.as_str()))]
fn force_get_meta_var<'i>(
    meta_vars: &NonLocalMetaSymTab<'i>,
    adt_pats: &AdtPats<'i>,
    ident: WithPath<'i, &pairs::MetaVariable<'i>>,
) -> MetaVariable<'i> {
    let name = ident.inner.span.as_str();
    meta_vars
        .get_meta_var_from_name(name)
        .or_else(|| adt_pats.get(&name).map(|kind| MetaVariable::AdtPat(*kind, name)))
        .unwrap_or_else(|| {
            panic!(
                "Meta variable `{}` not found in symbol table at {}",
                ident.inner.span.as_str(),
                ident.path.display()
            )
        })
}

pub trait GetType<'i> {
    fn get_type_or_path(
        &self,
        ident: &WithPath<'i, &pairs::Identifier<'i>>,
    ) -> Result<TypeOrPath<'i>, RPLMetaError<'i>>;

    fn force_get_meta_var(&self, ident: WithPath<'i, &pairs::MetaVariable<'i>>) -> MetaVariable<'i>;
    fn force_get_ty_meta_var(&self, ident: WithPath<'i, &pairs::TypeMetaVariable<'i>>) -> MetaVariable<'i> {
        self.force_get_meta_var(ident.map(|m| m.MetaVariable()))
    }
}

impl<'i> GetType<'i> for Fn<'i> {
    fn get_type_or_path(
        &self,
        ident: &WithPath<'i, &pairs::Identifier<'i>>,
    ) -> Result<TypeOrPath<'i>, RPLMetaError<'i>> {
        FnInner::get_type_or_path(&self.inner, ident.path, ident.inner)
    }
    #[inline]
    fn force_get_meta_var(&self, ident: WithPath<'i, &pairs::MetaVariable<'i>>) -> MetaVariable<'i> {
        force_get_meta_var(&self.meta_vars, &self.adt_pats, ident)
    }
}

impl<'i> GetType<'i> for WithMetaTable<'i, &'_ FnInner<'i>> {
    fn get_type_or_path(
        &self,
        ident: &WithPath<'i, &pairs::Identifier<'i>>,
    ) -> Result<TypeOrPath<'i>, RPLMetaError<'i>> {
        FnInner::get_type_or_path(self.inner, ident.path, ident.inner)
    }
    #[inline]
    fn force_get_meta_var(&self, ident: WithPath<'i, &pairs::MetaVariable<'i>>) -> MetaVariable<'i> {
        force_get_meta_var(&self.meta_vars, &self.adt_pats, ident)
    }
}

impl<'i> GetType<'i> for SymbolTable<'i> {
    #[instrument(level = "trace", skip(self), fields(imports = ?self.imports.keys()))]
    fn get_type_or_path(
        &self,
        ident: &WithPath<'i, &pairs::Identifier<'i>>,
    ) -> Result<TypeOrPath<'i>, RPLMetaError<'i>> {
        self.imports
            .get(&ident.span.as_str())
            .copied()
            .map(TypeOrPath::Path)
            .ok_or_else(move || RPLMetaError::TypeOrPathNotDeclared {
                span: ident.into(),
                type_or_path: ident.span.as_str(),
                declared: self.imports.keys().cloned().collect(),
            })
    }

    #[instrument(level = "trace", skip(self), fields(imports = ?self.imports.keys()))]
    fn force_get_meta_var(&self, ident: WithPath<'i, &pairs::MetaVariable<'i>>) -> MetaVariable<'i> {
        let name = ident.inner.span.as_str();
        self.meta_vars
            .get_meta_var_from_name(name)
            .or_else(|| self.adt_pats.get(&name).map(|kind| MetaVariable::AdtPat(*kind, name)))
            .unwrap_or_else(|| {
                panic!(
                    "Meta variable `{}` not found in symbol table at {}",
                    ident.inner.span.as_str(),
                    ident.path.display()
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::symbol_table::{Fn, FnInner};

    fn _ensure_fn_inner_covariance<'a: 'b, 'b>(value: FnInner<'a>) -> FnInner<'b> {
        value
    }

    fn _ensure_fn_covariance<'a: 'b, 'b>(value: Fn<'a>) -> Fn<'b> {
        value
    }
}
