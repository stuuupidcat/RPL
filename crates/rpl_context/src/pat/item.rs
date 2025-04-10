use std::ops::Deref;
use std::sync::Arc;

use super::utils::mutability_from_pair_mutability;
use super::{FnSymbolTable, MirPattern, NonLocalMetaVars, Path, RawDecleration, RawStatement, Ty};
use crate::PatCtxt;
use rpl_meta::collect_elems_separated_by_comma;
use rpl_parser::generics::Choice4;
use rpl_parser::pairs;
use rustc_data_structures::fx::{FxHashMap, FxIndexMap};
use rustc_hir::Safety;
use rustc_middle::mir;
use rustc_span::Symbol;

#[derive(Debug)]
pub struct Adt<'pcx> {
    pub meta: NonLocalMetaVars<'pcx>,
    pub kind: AdtKind<'pcx>,
}

impl<'pcx> Adt<'pcx> {
    pub(crate) fn new_struct() -> Self {
        Self {
            meta: NonLocalMetaVars::default(),
            kind: AdtKind::Struct(Default::default()),
        }
    }
    pub(crate) fn new_enum() -> Self {
        Self {
            meta: NonLocalMetaVars::default(),
            kind: AdtKind::Enum(Default::default()),
        }
    }
    pub fn non_enum_variant_mut(&mut self) -> &mut Variant<'pcx> {
        match &mut self.kind {
            AdtKind::Struct(variant) => variant,
            AdtKind::Enum(_) => panic!("cannot mutate non-enum variant of enum"),
        }
    }
    pub fn add_variant(&mut self, name: Symbol) -> &mut Variant<'pcx> {
        match &mut self.kind {
            AdtKind::Struct(_) => panic!("cannot add variant to struct"),
            AdtKind::Enum(variants) => variants.entry(name).or_insert_with(Variant::default),
        }
    }
    pub fn non_enum_variant(&self) -> &Variant<'pcx> {
        match &self.kind {
            AdtKind::Struct(variant) => variant,
            AdtKind::Enum(_) => panic!("cannot access non-enum variant of enum"),
        }
    }
    pub fn variant_and_index(&self, name: Symbol) -> (&Variant<'pcx>, usize) {
        match &self.kind {
            AdtKind::Struct(_) => panic!("expected enum"),
            AdtKind::Enum(variants) => {
                let (index, _, variant) = variants
                    .get_full(&name)
                    .unwrap_or_else(|| panic!("variant `${name}` not found"));
                (variant, index)
            },
        }
    }
    pub fn variant(&self, name: Symbol) -> &Variant<'pcx> {
        self.variant_and_index(name).0
    }
    pub fn variant_index(&self, name: Symbol) -> usize {
        self.variant_and_index(name).1
    }
    pub fn is_enum(&self) -> bool {
        matches!(self.kind, AdtKind::Enum(_))
    }
    pub fn is_struct(&self) -> bool {
        matches!(self.kind, AdtKind::Struct(_))
    }
}

#[derive(Debug)]
pub enum AdtKind<'pcx> {
    Struct(Variant<'pcx>),
    Enum(FxIndexMap<Symbol, Variant<'pcx>>),
}

#[derive(Default, Debug)]
pub struct Variant<'pcx> {
    pub fields: FxIndexMap<Symbol, Field<'pcx>>,
}

impl<'pcx> Variant<'pcx> {
    pub fn add_field(&mut self, name: Symbol, ty: Ty<'pcx>) {
        self.fields.insert(name, Field { ty });
    }
    pub fn field_and_index(&self, name: Symbol) -> (&Field<'pcx>, usize) {
        let (index, _, field) = self
            .fields
            .get_full(&name)
            .unwrap_or_else(|| panic!("field `${name}` not found"));
        (field, index)
    }
    pub fn field(&self, name: Symbol) -> &Field<'pcx> {
        self.field_and_index(name).0
    }
    pub fn field_index(&self, name: Symbol) -> usize {
        self.field_and_index(name).1
    }
}

#[derive(Debug)]
pub struct Field<'pcx> {
    pub ty: Ty<'pcx>,
}

pub struct Impl<'pcx> {
    pub meta: NonLocalMetaVars<'pcx>,
    #[expect(dead_code)]
    ty: Ty<'pcx>,
    #[expect(dead_code)]
    trait_id: Option<Path<'pcx>>,
    #[expect(dead_code)]
    fns: FxHashMap<Symbol, Fn<'pcx>>,
}

#[derive(Default)]
pub struct Fns<'pcx> {
    pub fns: FxHashMap<Symbol, Fn<'pcx>>,
    pub fn_pats: FxHashMap<Symbol, Fn<'pcx>>,
    pub unnamed_fns: Vec<Fn<'pcx>>,
}

pub struct Fn<'pcx> {
    pub safety: Safety,
    pub visibility: Visibility,
    pub name: Symbol,
    pub meta: Arc<NonLocalMetaVars<'pcx>>,
    pub params: Params<'pcx>,
    pub ret: Option<Ty<'pcx>>,
    pub body: Option<FnBody<'pcx>>,
}

#[derive(Clone, Copy)]
pub enum FnBody<'pcx> {
    Mir(&'pcx MirPattern<'pcx>),
}

impl<'pcx> Fn<'pcx> {
    pub fn from(
        pair: &pairs::Fn<'pcx>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
        meta: Arc<NonLocalMetaVars<'pcx>>,
    ) -> Self {
        let (sig, body) = pair.get_matched();
        let (safety, visibility, name, params, ret) = Self::from_sig(sig, pcx, fn_sym_tab);

        let (decls, stmts) = if let Some(body) = body.MirBody() {
            let (decls, stmts) = body.get_matched();
            (decls.iter_matched().collect(), stmts.iter_matched().collect())
        } else {
            (Vec::new(), Vec::new())
        };

        let raw_stmts = stmts.into_iter().map(|stmt| RawStatement::from(stmt, pcx, fn_sym_tab));
        let raw_decls = decls
            .into_iter()
            .map(|decl| RawDecleration::from(decl, pcx, fn_sym_tab));

        let mut builder = MirPattern::builder();
        builder.mk_locals(fn_sym_tab, pcx);
        builder.mk_raw_decls(raw_decls);
        builder.mk_raw_stmts(raw_stmts);
        let mir = builder.build();
        let body = Some(FnBody::Mir(pcx.mk_mir_pattern(mir)));

        Self {
            safety,
            visibility,
            meta,
            name,
            params,
            ret,
            body,
        }
    }

    pub fn from_sig(
        sig: &pairs::FnSig<'_>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> (Safety, Visibility, Symbol, Params<'pcx>, Option<Ty<'pcx>>) {
        let (unsafety, visibility, _, fn_name, _, params_pair, _, ret) = sig.get_matched();
        let safety = if unsafety.is_some() {
            Safety::Unsafe
        } else {
            Safety::Safe
        };
        let visibility = if visibility.is_some() {
            Visibility::Public
        } else {
            Visibility::Private
        };
        let fn_name = Symbol::intern(fn_name.span.as_str());
        let params = if let Some(params_pair) = params_pair {
            Params::from(params_pair, pcx, fn_sym_tab)
        } else {
            Params::default()
        };
        let ret = ret
            .as_ref()
            .map(|ret| Ty::from_fn_ret(ret, pcx, fn_sym_tab.meta_vars.clone()));
        (safety, visibility, fn_name, params, ret)
    }
}

pub enum Visibility {
    Public,
    Private,
}

#[derive(Default)]
pub struct Params<'pcx> {
    params: Vec<Param<'pcx>>,
    pub non_exhaustive: bool,
}

impl<'pcx> std::ops::Deref for Params<'pcx> {
    type Target = [Param<'pcx>];
    fn deref(&self) -> &Self::Target {
        &self.params
    }
}

pub struct Param<'pcx> {
    pub mutability: mir::Mutability,
    pub ident: Symbol,
    pub ty: Ty<'pcx>,
}

impl<'pcx> Param<'pcx> {
    /// The bool indicates whether the parameter is a `..`, which makes params non-
    pub fn from(
        param: &pairs::FnParam<'_>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> (Option<Self>, bool) {
        match param.deref() {
            Choice4::_0(_self_param) => {
                // FIXME: implement self param
                (None, false)
            },
            Choice4::_1(normal) => {
                let (mutability, ident, _, ty) = normal.get_matched();
                let mutability = mutability_from_pair_mutability(mutability);
                let ident = Symbol::intern(ident.span.as_str());
                let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
                (Some(Self { mutability, ident, ty }), false)
            },
            Choice4::_2(place_holder_with_type) => {
                let (mutability, placeholder, _, ty) = place_holder_with_type.get_matched();
                let mutability = mutability_from_pair_mutability(mutability);
                let ty = Ty::from(ty, pcx, fn_sym_tab.meta_vars.clone());
                (
                    Some(Self {
                        mutability,
                        ident: Symbol::intern(placeholder.span.as_str()),
                        ty,
                    }),
                    false,
                )
            },
            Choice4::_3(_ellpisis) => (None, true),
        }
    }
}

pub struct TraitDef {}

impl<'pcx> Fns<'pcx> {
    pub fn get_fn_pat(&self, name: Symbol) -> Option<&Fn<'pcx>> {
        self.fn_pats.get(&name)
    }
    // pub fn new_fn(&mut self, name: Symbol) -> &mut Fn<'pcx> {
    //     self.fns.entry(name).or_insert_with(|| Fn::new(name))
    // }
    // pub fn new_fn_pat(&mut self, name: Symbol) -> &mut Fn<'pcx> {
    //     self.fn_pats.entry(name).or_insert_with(|| Fn::new(name))
    // }
    // pub fn new_unnamed(&mut self) -> &mut Fn<'pcx> {
    //     self.unnamed_fns.push(Fn::new(kw::Underscore));
    //     self.unnamed_fns.last_mut().unwrap()
    // }
}

impl<'pcx> Fn<'pcx> {
    // pub(crate) fn new(name: Symbol) -> Self {
    //     Self {
    //         name,
    //         safety: Safety::Safe,
    //         visibility: Visibility::Public,
    //         meta: MetaVars::default(),
    //         params: Params::default(),
    //         ret: None,
    //         body: None,
    //     }
    // }
    // pub fn set_ret_ty(&mut self, ty: Ty<'pcx>) {
    //     self.ret = Some(ty);
    // }
    // pub fn set_body(&mut self, body: FnBody<'pcx>) {
    //     self.body = Some(body);
    // }
    // FIXME: remove this when all kinds of patterns are implemented
    pub fn expect_mir_body(&self) -> &'pcx MirPattern<'pcx> {
        match self.body {
            Some(FnBody::Mir(mir_body)) => mir_body,
            _ => panic!("expected MIR body"),
        }
    }
}

impl<'pcx> Params<'pcx> {
    pub fn from(
        pair: &pairs::FnParamsSeparatedByComma<'_>,
        pcx: PatCtxt<'pcx>,
        fn_sym_tab: &FnSymbolTable<'pcx>,
    ) -> Self {
        let params = collect_elems_separated_by_comma!(pair);
        let mut non_exhaustive: bool = false;
        let params = params
            .into_iter()
            .filter_map(|param| {
                let (param, ne) = Param::from(param, pcx, fn_sym_tab);
                non_exhaustive |= ne;
                param
            })
            .collect();
        Self { params, non_exhaustive }
    }
}
