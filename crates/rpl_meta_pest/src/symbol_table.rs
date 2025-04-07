use crate::check::CheckCtxt;
use crate::collect_elems_separated_by_comma;
use crate::context::MetaContext;
use crate::error::{RPLMetaError, RPLMetaResult};
use crate::utils::{Ident, Path};
use derive_more::derive::From;
use parser::generics::{Choice3, Choice4};
use parser::{pairs, SpanWrapper};
use pest_typed::Span;
use rustc_data_structures::fx::FxHashSet;
use rustc_hash::FxHashMap;
use rustc_span::Symbol;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Copy, From)]
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
}

pub enum MetaVariableType {
    Type,
    Const,
    Place,
}

// the usize in the hashmap is the *-index of a non-local meta variable
#[derive(Default, Clone)]
pub struct NonLocalMetaSymTab {
    ty_vars: FxHashMap<Symbol, usize>,
    const_vars: FxHashMap<Symbol, usize>,
    place_vars: FxHashMap<Symbol, usize>,
}

impl NonLocalMetaSymTab {
    pub fn add_non_local_meta_var<'i>(
        &mut self,
        mctx: &MetaContext<'i>,
        meta_var: Ident<'i>,
        meta_var_ty: &pairs::MetaVariableType<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        match meta_var_ty.deref() {
            Choice3::_0(_) => {
                let existed = self.ty_vars.insert(meta_var.name, self.ty_vars.len());
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.name,
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
            Choice3::_1(_) => {
                let existed = self.const_vars.insert(meta_var.name, self.const_vars.len());
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.name,
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
            Choice3::_2(_) => {
                let existed = self.place_vars.insert(meta_var.name, self.place_vars.len());
                if existed.is_some() {
                    let err = RPLMetaError::NonLocalMetaVariableAlreadyDeclared {
                        meta_var: meta_var.name,
                        span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                }
            },
        }
    }

    pub fn get_non_local_meta_var<'i>(
        &self,
        mctx: &MetaContext<'i>,
        meta_var: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<MetaVariableType> {
        if self.ty_vars.contains_key(&meta_var.name) {
            Some(MetaVariableType::Type)
        } else if self.const_vars.contains_key(&meta_var.name) {
            Some(MetaVariableType::Const)
        } else if self.place_vars.contains_key(&meta_var.name) {
            Some(MetaVariableType::Place)
        } else {
            let err = RPLMetaError::NonLocalMetaVariableNotDeclared {
                meta_var: meta_var.name,
                span: SpanWrapper::new(meta_var.span, mctx.get_active_path()),
            };
            errors.push(err);
            None
        }
    }

    pub fn get_type_and_idx_from_symbol(&self, symbol: Symbol) -> Option<(MetaVariableType, usize)> {
        if let Some(idx) = self.ty_vars.get(&symbol) {
            Some((MetaVariableType::Type, *idx))
        } else if let Some(idx) = self.const_vars.get(&symbol) {
            Some((MetaVariableType::Const, *idx))
        } else if let Some(idx) = self.place_vars.get(&symbol) {
            Some((MetaVariableType::Place, *idx))
        } else {
            None
        }
    }
}

pub struct WithMetaTable<T> {
    pub meta_vars: Arc<NonLocalMetaSymTab>,
    pub inner: T,
}

impl<T> From<(T, Arc<NonLocalMetaSymTab>)> for WithMetaTable<T> {
    fn from(inner: (T, Arc<NonLocalMetaSymTab>)) -> Self {
        Self {
            meta_vars: inner.1,
            inner: inner.0,
        }
    }
}

#[derive(Default)]
pub struct SymbolTable<'i> {
    // meta variables in p[$T: ty]
    pub meta_vars: Arc<NonLocalMetaSymTab>,
    structs: FxHashMap<Symbol, Struct<'i>>,
    enums: FxHashMap<Symbol, Enum<'i>>,
    fns: FxHashMap<Symbol, Fn<'i>>,
    unnamed_fns: Vec<Fn<'i>>,
    impls: Vec<Impl<'i>>,
}

impl<'i> SymbolTable<'i> {
    pub fn add_enum(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Enum<'i>> {
        self.enums
            .try_insert(ident.name, (EnumInner::new(), self.meta_vars.clone()).into())
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: *adt,
                    span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }

    pub fn add_struct(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Struct<'i>> {
        self.structs
            .try_insert(ident.name, (Variant::new(), self.meta_vars.clone()).into())
            .map_err(|entry| {
                let adt = entry.entry.key();
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: *adt,
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
    ) -> Option<&mut Fn<'i>> {
        match ident.deref() {
            Choice3::_0(_) => {
                self.unnamed_fns
                    .push((FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into());
                Some(self.unnamed_fns.last_mut().unwrap())
            },
            Choice3::_1(ident) => {
                let ident = Ident::from(ident);
                self.fns
                    .try_insert(
                        ident.name,
                        (FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into(),
                    )
                    .map_err(|_entry| {
                        let err = RPLMetaError::SymbolAlreadyDeclared {
                            ident: ident.name,
                            span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                        };
                        errors.push(err);
                    })
                    .ok()
            },
            Choice3::_2(ident) => {
                let ident = Ident::from(ident);
                self.fns
                    .try_insert(
                        ident.name,
                        (FnInner::new(ident.span, self_ty), self.meta_vars.clone()).into(),
                    )
                    .map_err(|_entry| {
                        let err = RPLMetaError::SymbolAlreadyDeclared {
                            ident: ident.name,
                            span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                        };
                        errors.push(err);
                    })
                    .ok()
            },
        }
    }

    #[expect(unused)]
    pub fn add_impl(&mut self, impl_pat: &'i pairs::Impl<'i>) -> &mut Impl<'i> {
        self.impls
            .push((ImplInner::new(impl_pat), self.meta_vars.clone()).into());
        self.impls.last_mut().unwrap()
    }

    #[expect(unused)]
    pub fn contains_adt(&self, ident: &Ident<'_>) -> bool {
        self.structs.contains_key(&ident.name) || self.enums.contains_key(&ident.name)
    }
}

impl<'i> SymbolTable<'i> {
    pub fn collect_symbol_tables(
        mctx: &MetaContext<'i>,
        pat_items: impl Iterator<Item = &'i pairs::pattBlockItem<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> FxHashMap<Symbol, Self> {
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
                    ident: name.clone(),
                    span: SpanWrapper::new(pat_item.Identifier().span, mctx.get_active_path()),
                };
                errors.push(err);
            });
        }
        symbol_tables
    }

    fn collect_symbol_table(mctx: &MetaContext<'i>, pat_item: &'i pairs::pattBlockItem<'i>) -> CheckCtxt<'i> {
        let pat_item_name = Symbol::intern(pat_item.Identifier().span.as_str());
        let mut cctx = CheckCtxt::new(pat_item_name);

        cctx.check_pat_item(mctx, pat_item);
        cctx
    }
}

impl<'i> SymbolTable<'i> {
    pub fn get_fn(&self, name: Symbol) -> Option<&Fn<'i>> {
        // FIXME
        if name == Symbol::intern("_") {
            return self.unnamed_fns.last();
        }
        self.fns.get(&name)
    }
}

pub type Enum<'i> = WithMetaTable<EnumInner<'i>>;

pub struct EnumInner<'i> {
    variants: FxHashMap<Symbol, Variant<'i>>,
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
        ident: Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<&mut Variant<'i>> {
        self.variants
            .try_insert(ident.name, Variant::new())
            .map_err(|_entry| {
                let err = RPLMetaError::SymbolAlreadyDeclared {
                    ident: ident.name,
                    span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                };
                errors.push(err);
            })
            .ok()
    }
}

pub struct Variant<'i> {
    fields: FxHashMap<Symbol, &'i pairs::Type<'i>>,
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
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.fields.try_insert(ident.name, ty).map_err(|_entry| {
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
}

pub(crate) type Struct<'i> = WithMetaTable<Variant<'i>>;

pub type Fn<'i> = WithMetaTable<FnInner<'i>>;

pub struct FnInner<'i> {
    #[expect(unused)]
    span: Span<'i>,
    types: FxHashMap<Symbol, TypeOrPath<'i>>,
    // FIXME: remove it when `self` parameter is implemented
    self_value: Option<&'i pairs::Type<'i>>,
    ret_value: Option<&'i pairs::Type<'i>>,
    self_param: Option<&'i pairs::SelfParam<'i>>,
    self_ty: Option<&'i pairs::Type<'i>>,
    params: FxHashMap<Symbol, &'i pairs::Type<'i>>,
    locals: FxHashMap<Symbol, (usize, &'i pairs::Type<'i>)>,
    pub symbol_to_local_idx: FxHashMap<Symbol, usize>,
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
            symbol_to_local_idx: FxHashMap::default(),
        }
    }
    fn add_type_impl(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        ty: TypeOrPath<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.types.try_insert(ident.name, ty).map_err(|_entry| {
            let err = RPLMetaError::TypeOrPathAlreadyDeclared {
                type_or_path: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
    pub fn add_type(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        self.add_type_impl(mctx, ident, TypeOrPath::Type(ty), errors);
    }
    pub fn get_type(
        &self,
        mctx: &MetaContext<'i>,
        ident: &Ident<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> Option<TypeOrPath<'i>> {
        self.types.get(&ident.name).copied().or_else(|| {
            let err = RPLMetaError::TypeOrPathNotDeclared {
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
                type_or_path: ident.name,
            };
            errors.push(err);
            None
        })
    }
    pub fn add_path(&mut self, mctx: &MetaContext<'i>, path: &'i pairs::Path<'i>, errors: &mut Vec<RPLMetaError<'i>>) {
        let ty_or_path = path.into();
        let path: Path<'i> = path.into();
        let ident = path.ident();
        if let Some(ident) = ident {
            self.add_type_impl(mctx, ident, ty_or_path, errors);
        }
    }

    pub fn get_sorted_locals(&self) -> Vec<(Symbol, &'i pairs::Type<'i>)> {
        let mut locals = self
            .locals
            .iter()
            .map(|(ident, (idx, ty))| (ident, (idx, ty)))
            .collect::<Vec<_>>();
        locals.sort_by_key(|(_, (idx, _))| *idx);
        locals.into_iter().map(|(ident, (_, ty))| (*ident, *ty)).collect()
    }

    pub fn get_local_idx(&self, symbol: Symbol) -> usize {
        self.symbol_to_local_idx.get(&symbol).copied().expect("local not found") // should not
                                                                                 // panic
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
    }
    pub fn add_param(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.params.try_insert(ident.name, ty).map_err(|_entry| {
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        });
    }
    pub fn add_local(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        let len = self.locals.len();
        if self.locals.contains_key(&ident.name) {
            let err = RPLMetaError::SymbolAlreadyDeclared {
                ident: ident.name,
                span: SpanWrapper::new(ident.span, mctx.get_active_path()),
            };
            errors.push(err);
        } else {
            self.locals.insert(ident.name, (len, ty));
            self.symbol_to_local_idx.insert(ident.name, len);
        }
    }
    pub fn add_place_local(
        &mut self,
        mctx: &MetaContext<'i>,
        local: &'i pairs::MirPlaceLocal<'i>,
        ty: &'i pairs::Type<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        match local.deref() {
            Choice4::_0(_place_holder) => {},
            Choice4::_1(_self_value) => self.self_value = Some(ty),
            Choice4::_2(_ret_value) => self.ret_value = Some(ty),
            Choice4::_3(ident) => self.add_local(mctx, ident.into(), ty, errors),
        }
    }
    fn get_local_impl(&self, ident: Ident<'i>) -> Option<&'i pairs::Type<'i>> {
        self.locals
            .get(&ident.name)
            .and_then(|(_idx, ty)| Some(ty))
            .or_else(|| self.params.get(&ident.name))
            .copied()
    }
    pub fn get_local(
        &self,
        mctx: &MetaContext<'i>,
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
        mctx: &MetaContext<'i>,
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

pub(crate) type Impl<'i> = WithMetaTable<ImplInner<'i>>;

pub(crate) struct ImplInner<'i> {
    #[expect(unused)]
    trait_: Option<&'i pairs::Path<'i>>,
    #[expect(unused)]
    ty: &'i pairs::Type<'i>,
    #[allow(unused)]
    fns: FxHashMap<Symbol, FnInner<'i>>,
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
            .try_insert(ident.name, fn_def)
            .map_err(|entry| RPLMetaError::MethodAlreadyDeclared { _span: ident.span })
    }
}

#[derive(Default)]
pub struct DiagSymbolTable {
    diags: FxHashMap<Symbol, String>,
}

impl DiagSymbolTable {
    pub fn collect_symbol_tables<'i>(
        mctx: &MetaContext<'i>,
        diags: impl Iterator<Item = &'i pairs::diagBlockItem<'i>>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> FxHashMap<Symbol, DiagSymbolTable> {
        let mut diag_symbols = FxHashMap::default();
        for diag in diags {
            let name = diag.Identifier();
            let symbol_table = Self::collect_diag_symbol_table(mctx, diag, errors);
            _ = diag_symbols
                .try_insert(Symbol::intern(name.span.as_str()), symbol_table)
                .map_err(|entry| {
                    let ident = entry.entry.key();
                    let err = RPLMetaError::SymbolAlreadyDeclared {
                        ident: ident.clone(),
                        span: SpanWrapper::new(name.span, mctx.get_active_path()),
                    };
                    errors.push(err);
                });
        }
        diag_symbols
    }

    fn collect_diag_symbol_table<'i>(
        mctx: &MetaContext<'i>,
        diag: &'i pairs::diagBlockItem<'i>,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) -> DiagSymbolTable {
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

    pub fn add_diag<'i>(
        &mut self,
        mctx: &MetaContext<'i>,
        ident: Ident<'i>,
        message: String,
        errors: &mut Vec<RPLMetaError<'i>>,
    ) {
        _ = self.diags.try_insert(ident.name, message).map_err(|_entry| {
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
