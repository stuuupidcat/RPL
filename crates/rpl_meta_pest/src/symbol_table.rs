use crate::check::CheckCtxt;
use crate::collect_elems_separated_by_comma;
use crate::context::RPLMetaContext;
use crate::error::{RPLMetaError, RPLMetaResult};
use crate::utils::{Ident, Path};
use derive_more::derive::From;
use parser::generics::{Choice3, Choice4};
use parser::{pairs, SpanWrapper};
use pest_typed::Span;
use rustc_hash::FxHashMap;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Copy, From)]
pub(crate) enum TypeOrPath<'i> {
    Type(&'i pairs::Type<'i>),
    Path(&'i pairs::Path<'i>),
}

impl<'i> TypeOrPath<'i> {
    #[expect(unused)]
    pub fn span(&self) -> Span<'i> {
        match self {
            Self::Type(ty) => ty.span,
            Self::Path(path) => path.span,
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct MetaVarTable<'i> {
    meta_vars: FxHashMap<Ident<'i>, Ident<'i>>,
}

impl<'i> MetaVarTable<'i> {
    pub fn add_ty_var(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        meta_var: Ident<'i>,
        meta_var_ty: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        let _ = self.meta_vars.try_insert(meta_var, meta_var_ty).map_err(|entry| {
            let err = RPLMetaError::TypeMetaVariableAlreadyDeclared {
                meta_var: meta_var.name,
                span: SpanWrapper::new(entry.entry.key().span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
    pub fn get_ty_var(
        &self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<Ident<'i>> {
        self.meta_vars.get(&ident).copied().or_else(|| {
            let err = RPLMetaError::TypeMetaVariableNotDeclared {
                meta_var: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
            None
        })
    }
}

#[derive(Clone, Copy)]
#[expect(unused)]
pub(crate) enum ExportKind {
    Meta,
    Statement,
    SwitchTarget,
}

#[derive(Default)]
pub(crate) struct ExportTable<'i> {
    #[allow(unused)]
    exports: FxHashMap<Ident<'i>, ExportKind>,
}

impl<'i> ExportTable<'i> {
    #[expect(unused)]
    pub fn add_export(&mut self, export: Ident<'i>, kind: ExportKind) -> RPLMetaResult<()> {
        self.exports.try_insert(export, kind).map_err(|entry| {
            let ident = entry.entry.key();
            RPLMetaError::ExportAlreadyDeclared { _span: ident.span }
        })?;
        Ok(())
    }
}

pub(crate) struct WithMetaTable<'i, T> {
    pub(crate) meta_vars: Arc<MetaVarTable<'i>>,
    pub(crate) exports: ExportTable<'i>, // FIXME
    pub(crate) inner: T,
}

impl<'i, T> From<(T, Arc<MetaVarTable<'i>>)> for WithMetaTable<'i, T> {
    fn from(inner: (T, Arc<MetaVarTable<'i>>)) -> Self {
        Self {
            meta_vars: inner.1,
            exports: ExportTable::default(),
            inner: inner.0,
        }
    }
}

#[derive(Default)]
pub(crate) struct SymbolTable<'i> {
    // meta variables in p[$T: ty]
    pub(crate) meta_vars: Arc<MetaVarTable<'i>>,
    structs: FxHashMap<Ident<'i>, Struct<'i>>,
    enums: FxHashMap<Ident<'i>, Enum<'i>>,
    fns: FxHashMap<Ident<'i>, Fn<'i>>,
    unnamed_fns: Vec<Fn<'i>>,
    impls: Vec<Impl<'i>>,
}

impl<'i> SymbolTable<'i> {
    pub fn add_enum(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Enum<'i>> {
        self.enums
            .try_insert(ident, (EnumInner::new(), self.meta_vars.clone()).into())
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: adt.name,
                    span: SpanWrapper::new(adt.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }

    pub fn add_struct(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Struct<'i>> {
        self.structs
            .try_insert(ident, (Variant::new(), self.meta_vars.clone()).into())
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: adt.name,
                    span: SpanWrapper::new(adt.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }

    pub fn add_fn(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: &'i pairs::FnName<'i>,
        self_ty: Option<&'i pairs::Type<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Fn<'i>> {
        match ident.deref() {
            Choice3::_0(_) => {
                self.unnamed_fns
                    .push((FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into());
                Some(self.unnamed_fns.last_mut().unwrap())
            },
            Choice3::_1(ident) => self
                .fns
                .try_insert(
                    ident.into(),
                    (FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into(),
                )
                .map_err(|entry| {
                    let ident = entry.entry.key();
                    let err = RPLMetaError::SymbolAlreadyDeclared {
                        ident: ident.name,
                        span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                })
                .ok(),
            Choice3::_2(ident) => self
                .fns
                .try_insert(
                    ident.into(),
                    (FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into(),
                )
                .map_err(|entry| {
                    let ident = entry.entry.key();
                    let err = RPLMetaError::SymbolAlreadyDeclared {
                        ident: ident.name,
                        span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                })
                .ok(),
        }
    }

    #[expect(unused)]
    pub fn add_impl(&mut self, impl_pat: &'i pairs::Impl<'i>) -> &mut Impl<'i> {
        self.impls
            .push((ImplInner::new(impl_pat), self.meta_vars.clone()).into());
        self.impls.last_mut().unwrap()
    }

    #[expect(unused)]
    pub fn contains_adt(&self, ident: &Ident) -> bool {
        self.structs.contains_key(ident) || self.enums.contains_key(ident)
    }
}

impl<'i> SymbolTable<'i> {
    pub fn collect_symbol_tables(
        mctx: &RPLMetaContext<'i>,
        pat_items: impl Iterator<Item = &'i pairs::pattBlockItem<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> FxHashMap<&'i str, Self> {
        let mut symbol_tables = FxHashMap::default();
        for pat_item in pat_items {
            let CheckCtxt {
                name,
                symbol_table: symbols,
                errors: error_vec,
            } = Self::collect_symbol_table(mctx, pat_item);
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

    fn collect_symbol_table(mctx: &RPLMetaContext<'i>, pat_item: &'i pairs::pattBlockItem<'i>) -> CheckCtxt<'i> {
        let pat_item_name = pat_item.Identifier().span.as_str();
        let mut cctx = CheckCtxt::new(pat_item_name);

        cctx.check_pat_item(mctx, pat_item);
        cctx
    }
}

pub(crate) type Enum<'i> = WithMetaTable<'i, EnumInner<'i>>;

pub(crate) struct EnumInner<'i> {
    variants: FxHashMap<Ident<'i>, Variant<'i>>,
}

impl<'i> EnumInner<'i> {
    fn new() -> Self {
        Self {
            variants: FxHashMap::default(),
        }
    }
    pub fn add_variant(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Variant<'i>> {
        self.variants
            .try_insert(ident, Variant::new())
            .map_err(|entry| {
                let variant = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: variant.name,
                    span: SpanWrapper::new(variant.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }
}

pub(crate) struct Variant<'i> {
    fields: FxHashMap<Ident<'i>, &'i pairs::Type<'i>>,
}

impl<'i> Variant<'i> {
    fn new() -> Self {
        Self {
            fields: FxHashMap::default(),
        }
    }
    pub fn add_field(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.fields.try_insert(ident, ty).map_err(|entry| {
            let field = entry.entry.key();
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: field.name,
                span: SpanWrapper::new(field.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
}

pub(crate) type Struct<'i> = WithMetaTable<'i, Variant<'i>>;

pub(crate) type Fn<'i> = WithMetaTable<'i, FnInner<'i>>;

pub(crate) struct FnInner<'i> {
    #[expect(unused)]
    span: Span<'i>,
    types: FxHashMap<Ident<'i>, TypeOrPath<'i>>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<&'i pairs::Type<'i>>,
    ret_value: Option<&'i pairs::Type<'i>>,
    self_param: Option<&'i pairs::SelfParam<'i>>,
    self_ty: Option<&'i pairs::Type<'i>>,
    params: FxHashMap<Ident<'i>, &'i pairs::Type<'i>>,
    locals: FxHashMap<Ident<'i>, Vec<&'i pairs::Type<'i>>>,
}

impl<'i> FnInner<'i> {
    fn new(span: Span<'i>, self_ty: Option<&'i pairs::Type<'i>>) -> Self {
        Self {
            span,
            types: FxHashMap::default(),
            self_value: None,
            ret_value: None,
            self_param: None,
            self_ty,
            params: FxHashMap::default(),
            locals: FxHashMap::default(),
        }
    }
    fn add_type_impl(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        ty: TypeOrPath<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.types.try_insert(ident, ty).map_err(|entry| {
            let ident = entry.entry.key();
            let err = RPLMetaError::TypeOrPathAlreadyDeclared {
                type_or_path: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
    pub fn add_type(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        self.add_type_impl(mctx, ident, TypeOrPath::Type(ty), errors);
    }
    pub fn get_type(
        &self,
        mctx: &RPLMetaContext<'i>,
        ident: &Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<TypeOrPath<'i>> {
        self.types.get(ident).copied().or_else(|| {
            let err = RPLMetaError::TypeOrPathNotDeclared {
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                type_or_path: ident.name,
            };
            errors.push(err);
            None
        })
    }
    pub fn add_path(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        path: &'i pairs::Path<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        let ty_or_path = path.into();
        let path: Path<'i> = path.into();
        let ident = path.ident();
        if let Some(ident) = ident {
            self.add_type_impl(mctx, ident, ty_or_path, errors);
        }
    }
}

impl<'i> FnInner<'i> {
    pub fn add_self_param(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        self_param: &'i pairs::SelfParam<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        if self.self_param.is_some() {
            errors.push(RPLMetaError::SelfAlreadyDeclared {
                span: SpanWrapper::new(self_param.span, mctx.get_active_path()),
            });
        }
        self.self_param = Some(self_param);
    }
    pub fn add_param(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.params.try_insert(ident, ty).map_err(|entry| {
            let param = entry.entry.key();
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: param.name,
                span: SpanWrapper::new(param.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
    pub fn add_local(&mut self, ident: Ident<'i>, ty: &'i pairs::Type<'i>) {
        self.locals.entry(ident).or_default().push(ty);
    }
    pub fn add_place_local(&mut self, local: &'i pairs::MirPlaceLocal<'i>, ty: &'i pairs::Type<'i>) {
        match local.deref() {
            Choice4::_0(_place_holder) => {},
            Choice4::_1(_self_value) => self.self_value = Some(ty),
            Choice4::_2(_ret_value) => self.ret_value = Some(ty),
            Choice4::_3(ident) => self.add_local(ident.into(), ty),
        }
    }
    fn get_local_impl(&self, ident: Ident<'i>) -> Option<&'i pairs::Type<'i>> {
        self.locals
            .get(&ident)
            .and_then(|types| types.last())
            .or_else(|| self.params.get(&ident))
            .copied()
    }
    pub fn get_local(
        &self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&'i pairs::Type<'i>> {
        self.get_local_impl(ident).or_else(|| {
            let err = RPLMetaError::SymbolNotDeclared {
                ident: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
            None
        })
    }
    pub fn get_place_local(
        &self,
        mctx: &RPLMetaContext<'i>,
        local: &'i pairs::MirPlaceLocal<'i>,
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

            Choice4::_3(ident) => self.get_local(mctx, ident.into(), errors),
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

pub(crate) type Impl<'i> = WithMetaTable<'i, ImplInner<'i>>;

pub(crate) struct ImplInner<'i> {
    #[expect(unused)]
    trait_: Option<&'i pairs::Path<'i>>,
    #[expect(unused)]
    ty: &'i pairs::Type<'i>,
    #[allow(unused)]
    fns: FxHashMap<Ident<'i>, FnInner<'i>>,
}

impl<'i> ImplInner<'i> {
    pub fn new(impl_pat: &'i pairs::Impl<'i>) -> Self {
        let impl_pat = impl_pat.get_matched();
        let trait_ = impl_pat.1.as_ref().map(|trait_| trait_.get_matched().0);
        Self {
            trait_,
            ty: impl_pat.2,
            fns: FxHashMap::default(),
        }
    }
}

impl<'i> ImplInner<'i> {
    #[expect(unused)]
    pub fn add_fn(&mut self, ident: Ident<'i>, fn_def: FnInner<'i>) -> RPLMetaResult<&mut FnInner<'i>> {
        self.fns
            .try_insert(ident, fn_def)
            .map_err(|entry| RPLMetaError::MethodAlreadyDeclared { _span: ident.span })
    }
}

#[derive(Default)]
pub(crate) struct DiagSymbolTable<'i> {
    diags: FxHashMap<Ident<'i>, String>,
}

impl<'i> DiagSymbolTable<'i> {
    pub fn collect_symbol_tables(
        mctx: &RPLMetaContext<'i>,
        diags: impl Iterator<Item = &'i pairs::diagBlockItem<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> FxHashMap<&'i str, DiagSymbolTable<'i>> {
        let mut diag_symbols = FxHashMap::default();
        for diag in diags {
            let name = diag.Identifier();
            let symbol_table = Self::collect_diag_symbol_table(mctx, diag, errors);
            _ = diag_symbols
                .try_insert(name.span.as_str(), symbol_table)
                .map_err(|entry| {
                    let ident = entry.entry.key();
                    let err = RPLMetaError::SymbolAlreadyDeclared {
                        ident,
                        span: SpanWrapper::new(name.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                });
        }
        diag_symbols
    }

    fn collect_diag_symbol_table(
        mctx: &RPLMetaContext<'i>,
        diag: &'i pairs::diagBlockItem<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> DiagSymbolTable<'i> {
        let mut diag_symbol_table = DiagSymbolTable::default();
        let (_, _, _, _tldr, messages, _) = diag.get_matched();
        if let Some(messages) = messages {
            let messages = collect_elems_separated_by_comma!(messages);
            for message in messages {
                let (ident, _, string) = message.get_matched();
                diag_symbol_table.add_diag(mctx, ident.into(), string.span.to_string(), errors);
            }
        }
        diag_symbol_table
    }

    pub fn add_diag(
        &mut self,
        mctx: &RPLMetaContext<'i>,
        ident: Ident<'i>,
        message: String,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.diags.try_insert(ident, message).map_err(|entry| {
            let ident = entry.entry.key();
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
}

static PRIMITIVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "bool", "str",
];

pub(crate) fn ident_is_primitive(ident: &Ident) -> bool {
    PRIMITIVES.contains(&ident.name.to_string().as_str())
}

pub(crate) fn str_is_primitive(ident: &str) -> bool {
    PRIMITIVES.contains(&ident)
}
