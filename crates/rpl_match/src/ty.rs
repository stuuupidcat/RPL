use std::cell::RefCell;
use std::iter::zip;

use rpl_context::{pat, PatCtxt};
use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::definitions::DefPathData;
use rustc_index::IndexVec;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::symbol::kw;
use rustc_span::Symbol;

use crate::resolve::{self, lang_item_res, ty_res, PatItemKind};
pub struct MatchTyCtxt<'pcx, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
    param_env: ty::ParamEnv<'tcx>,
    pub ty_vars: IndexVec<pat::TyVarIdx, RefCell<Vec<ty::Ty<'tcx>>>>,
}

impl<'pcx, 'tcx> MatchTyCtxt<'pcx, 'tcx> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        pcx: PatCtxt<'pcx>,
        param_env: ty::ParamEnv<'tcx>,
        meta: &pat::MetaVars<'pcx>,
    ) -> Self {
        Self {
            tcx,
            pcx,
            param_env,
            ty_vars: IndexVec::from_elem(RefCell::new(Vec::new()), &meta.ty_vars),
        }
    }

    pub fn match_ty(&self, ty_pat: pat::Ty<'pcx>, ty: ty::Ty<'tcx>) -> bool {
        let ty_pat_kind = *ty_pat.kind();
        let ty_kind = *ty.kind();
        let matched = match (ty_pat_kind, ty_kind) {
            (pat::TyKind::TyVar(ty_var), _)
                if ty_var.pred.is_none_or(|ty_pred| ty_pred(self.tcx, self.param_env, ty)) =>
            {
                self.ty_vars[ty_var.idx].borrow_mut().push(ty);
                true
            },
            (pat::TyKind::Array(ty_pat, konst_pat), ty::Array(ty, konst)) => {
                self.match_ty(ty_pat, ty) && self.match_const(konst_pat, konst)
            },
            (pat::TyKind::Slice(ty_pat), ty::Slice(ty)) => self.match_ty(ty_pat, ty),
            (pat::TyKind::Tuple(tys_pat), ty::Tuple(tys)) => {
                tys_pat.len() == tys.len() && zip(tys_pat, tys).all(|(&ty_pat, ty)| self.match_ty(ty_pat, ty))
            },
            (pat::TyKind::Ref(region_pat, pat_ty, pat_mutblty), ty::Ref(region, ty, mutblty)) => {
                self.match_region(region_pat, region) && pat_mutblty == mutblty && self.match_ty(pat_ty, ty)
            },
            (pat::TyKind::RawPtr(ty_pat, mutability_pat), ty::RawPtr(ty, mutblty)) => {
                mutability_pat == mutblty && self.match_ty(ty_pat, ty)
            },
            (pat::TyKind::Uint(ty_pat), ty::Uint(ty)) => ty_pat == ty,
            (pat::TyKind::Int(ty_pat), ty::Int(ty)) => ty_pat == ty,
            (pat::TyKind::Float(ty_pat), ty::Float(ty)) => ty_pat == ty,
            // (pat::TyKind::Path(path_with_args), ty::Adt(adt, args)) => {
            //     self.match_path_with_args(path_with_args, adt.did(), args)
            // },
            // (pat::TyKind::Path(path_with_args), ty::FnDef(def_id, args)) => {
            //     self.match_path_with_args(path_with_args, def_id, args)
            // },
            (pat::TyKind::Def(def_id_pat, args_pat), ty::Adt(adt, args)) => {
                let def_id = adt.did();
                // trace!(?def_id_pat, ?def_id, ?args_pat, ?args, "match_ty def");
                self.match_generic_args(args_pat, args, self.tcx.generics_of(def_id)) && def_id_pat == def_id
            },
            (pat::TyKind::Def(def_id_pat, args_pat), ty::FnDef(def_id, args)) => {
                self.match_generic_args(args_pat, args, self.tcx.generics_of(def_id)) && def_id_pat == def_id
            },
            (pat::TyKind::Path(path_with_args), _) => {
                //FIXME: generics args are ignored.
                match path_with_args.path {
                    pat::Path::Item(path) => ty_res(self.pcx, self.tcx, path.0, path_with_args.args),
                    pat::Path::LangItem(item) => lang_item_res(self.pcx, self.tcx, item),
                    pat::Path::TypeRelative(_, _) => todo!(),
                }
                .map(|ty_pat| self.match_ty(ty_pat, ty))
                .unwrap_or(false)
            },
            // (pat::TyKind::Alias(alias_kind_pat, path, args), ty::Alias(alias_kind, alias)) => {
            //     alias_kind_pat == alias_kind
            //         && self.match_path(path, alias.def_id)
            //         && self.match_generic_args(args, alias.args)
            // },
            (pat::TyKind::Bool, ty::Bool) => true,
            (
                pat::TyKind::TyVar(_)
                | pat::TyKind::AdtVar(_)
                | pat::TyKind::Array(..)
                | pat::TyKind::Slice(_)
                | pat::TyKind::Tuple(_)
                | pat::TyKind::Ref(..)
                | pat::TyKind::RawPtr(..)
                | pat::TyKind::Uint(_)
                | pat::TyKind::Int(_)
                | pat::TyKind::Float(_)
                | pat::TyKind::Def(_, _)
                | pat::TyKind::Bool
                | pat::TyKind::Str
                | pat::TyKind::Char
                | pat::TyKind::Any,
                ty::Bool
                | ty::Char
                | ty::Int(_)
                | ty::Uint(_)
                | ty::Float(_)
                | ty::Adt(..)
                | ty::Foreign(..)
                | ty::Str
                | ty::Array(..)
                | ty::Pat(..)
                | ty::Slice(_)
                | ty::RawPtr(..)
                | ty::Ref(..)
                | ty::FnDef(..)
                | ty::FnPtr(..)
                | ty::Dynamic(..)
                | ty::Closure(..)
                | ty::CoroutineClosure(..)
                | ty::Coroutine(..)
                | ty::CoroutineWitness(..)
                | ty::Never
                | ty::Tuple(_)
                | ty::Alias(..)
                | ty::Param(_)
                | ty::Bound(..)
                | ty::Placeholder(_)
                | ty::Infer(_)
                | ty::Error(_),
            ) => false,
        };
        debug!(?ty_pat, ?ty, matched, "match_ty");
        matched
    }

    pub fn match_const(&self, konst_pat: pat::Const<'pcx>, konst: ty::Const<'tcx>) -> bool {
        match (konst_pat, konst.kind()) {
            (pat::Const::ConstVar(const_var), _) => self.match_const_var(const_var, konst),
            (pat::Const::Value(_value_pat), ty::Value(_ty, ty::ValTree::Leaf(_value))) => todo!(),
            (
                // pat::Const::ConstVar(_)
                pat::Const::Value(_),
                ty::ConstKind::Param(_)
                | ty::ConstKind::Infer(_)
                | ty::ConstKind::Bound(..)
                | ty::ConstKind::Placeholder(_)
                | ty::ConstKind::Unevaluated(_)
                | ty::ConstKind::Value(..)
                | ty::ConstKind::Error(_)
                | ty::ConstKind::Expr(_),
            ) => false,
        }
    }

    pub fn match_const_var(&self, const_var: pat::ConstVar<'pcx>, konst: ty::Const<'tcx>) -> bool {
        if let ty::ConstKind::Value(ty, _) = konst.kind()
            && self.match_ty(const_var.ty, ty)
        {
            // self.const_vars[const_var].borrow_mut().push(konst);
            return true;
        }
        false
    }

    pub fn match_region(&self, pat: pat::RegionKind, region: ty::Region<'tcx>) -> bool {
        matches!(
            (pat, region.kind()),
            (pat::RegionKind::ReStatic, ty::RegionKind::ReStatic) | (pat::RegionKind::ReAny, _)
        )
    }

    /// Match type path
    #[instrument(level = "trace", skip(self), ret)]
    pub fn match_path_with_args(
        &self,
        path_with_args: pat::PathWithArgs<'pcx>,
        def_id: DefId,
        args: ty::GenericArgsRef<'tcx>,
    ) -> bool {
        let generics = self.tcx.generics_of(def_id);
        self.match_path(path_with_args.path, def_id) && self.match_generic_args(path_with_args.args, args, generics)
    }

    #[instrument(level = "trace", skip(self), ret)]
    pub fn match_path(&self, path: pat::Path<'pcx>, def_id: DefId) -> bool {
        let matched = match path {
            // pat::Path::Item(path) => matches!(self.match_item_path(path, def_id), Some([])),
            pat::Path::Item(path) => self.match_item_path_by_def_path(path, def_id),
            pat::Path::TypeRelative(ty, name) => {
                self.tcx.item_name(def_id) == name
                    && self
                        .tcx
                        .opt_parent(def_id)
                        .is_some_and(|did| self.match_ty(ty, self.tcx.type_of(did).instantiate_identity()))
            },
            pat::Path::LangItem(lang_item) => self.tcx.is_lang_item(def_id, lang_item),
        };
        debug!(?path, ?def_id, matched, "match_path");
        matched
    }

    /// Resolve definition path from `path`.
    // FIXME: when searching in the same crate, if with the same kind, an item path should always be resolved to the
    // same item, so this can be cached for performance.
    #[instrument(level = "trace", skip(self), ret)]
    pub fn match_item_path_by_def_path(&self, path: pat::ItemPath<'pcx>, def_id: DefId) -> bool {
        let kind = if let Some(kind) = PatItemKind::infer_from_def_kind(self.tcx.def_kind(def_id)) {
            kind
        } else {
            return false;
        };
        let res = resolve::def_path_res(self.tcx, path.0, kind);
        trace!(?res);
        let mut res = res.into_iter().filter_map(|res| match res {
            Res::Def(_, id) => Some(id),
            _ => None,
        });
        let pat_id = if let Some(id) = res.next() { id } else { return false };
        // FIXME: there should be at most one item matching specific item kind
        assert!(res.next().is_none());

        trace!(?pat_id, ?def_id);

        pat_id == def_id
    }

    pub fn match_item_path(&self, path: pat::ItemPath<'pcx>, def_id: DefId) -> Option<&[Symbol]> {
        let &[krate, ref in_crate @ ..] = path.0 else {
            // an empty `ItemPath`
            return None;
        };
        let def_path = self.tcx.def_path(def_id);
        let matched = match def_path.krate {
            LOCAL_CRATE => krate == kw::Crate,
            _ => self.tcx.crate_name(def_path.krate) == krate,
        };
        let mut pat_iter = in_crate.iter();
        use DefPathData::{Impl, TypeNs, ValueNs};
        let mut iter = def_path
            .data
            .iter()
            .filter(|data| matches!(data.data, Impl | TypeNs(_) | ValueNs(_)));
        let matched = matched
            && std::iter::zip(pat_iter.by_ref(), iter.by_ref())
                .all(|(&path, data)| data.data.get_opt_name().is_some_and(|name| name == path));
        // Check that `iter` (from `def_path`) is not longer than `pat_iter` (from `path`)
        let matched = matched && iter.next().is_none();
        debug!(?path, ?def_id, matched, "match_item_path");
        matched.then_some(pat_iter.as_slice())
    }

    pub fn match_generic_args(
        &self,
        args_pat: pat::GenericArgsRef<'pcx>,
        args: ty::GenericArgsRef<'tcx>,
        generics: &'tcx ty::Generics,
    ) -> bool {
        // Is it necessary to call this function?
        let args_all = generics.own_args(args);
        let args_no_default = generics.own_args_no_defaults(self.tcx, args);
        if args_pat.len() < args_no_default.len() || args_pat.len() > args_all.len() {
            false
        } else {
            // FIXME:
            // directly zip args_all[..args_pat.len()]?
            args_pat
                .iter()
                .zip(
                    args_no_default
                        .iter()
                        .chain(args_all[args_no_default.len()..args_pat.len()].iter()),
                )
                .all(|(pat, arg)| self.match_generic_arg(*pat, *arg))
        }
    }

    fn match_generic_arg(&self, arg_pat: pat::GenericArgKind<'pcx>, arg: ty::GenericArg<'tcx>) -> bool {
        match (arg_pat, arg.unpack()) {
            (pat::GenericArgKind::Lifetime(region_pat), ty::GenericArgKind::Lifetime(region)) => {
                self.match_region(region_pat, region)
            },
            (pat::GenericArgKind::Type(ty_pat), ty::GenericArgKind::Type(ty)) => self.match_ty(ty_pat, ty),
            (pat::GenericArgKind::Const(konst_pat), ty::GenericArgKind::Const(konst)) => {
                self.match_const(konst_pat, konst)
            },
            (
                pat::GenericArgKind::Lifetime(_) | pat::GenericArgKind::Type(_) | pat::GenericArgKind::Const(_),
                ty::GenericArgKind::Lifetime(_) | ty::GenericArgKind::Type(_) | ty::GenericArgKind::Const(_),
            ) => false,
        }
    }
}
