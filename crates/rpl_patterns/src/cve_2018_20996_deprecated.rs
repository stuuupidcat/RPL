use rustc_hash::FxHashSet;
use rustc_hir as hir;
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::{self};
use rustc_middle::ty::{self, Ty, TyCtxt, TypeVisitable, TypeVisitableExt, TypeVisitor};
use rustc_span::sym;

pub fn check_owner(tcx: TyCtxt<'_>, owner_id: hir::OwnerId) {
    let owner = tcx.hir_owner_node(owner_id);
    let (Some(_fn_sig), Some(_generics), Some(body_id)) = (owner.fn_sig(), owner.generics(), owner.body_id()) else {
        return;
    };
    let def_id = owner_id.def_id;
    let fn_sig = tcx.normalize_erasing_late_bound_regions(
        tcx.param_env_reveal_all_normalized(def_id),
        tcx.fn_sig(def_id).instantiate_identity(),
    );
    let output_ty = fn_sig.output();
    let generics = tcx.generics_of(def_id);
    for param_index in 0..generics.count() {
        let param = generics.param_at(param_index, tcx);
        if let ty::GenericParamDefKind::Type { .. } = param.kind {
            drops_generic_ty(tcx, output_ty, ty::ParamTy::for_def(param));
        }
    }
}

fn drops_any_generics<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, mut generics: &'tcx ty::Generics) -> bool {
    if !ty.has_type_flags(ty::TypeFlags::HAS_TY_PARAM) {
        return false;
    }
    loop {
        if generics
            .own_params
            .iter()
            .filter(|param| matches!(param.kind, ty::GenericParamDefKind::Type { .. }))
            .any(|param| drops_generic_ty(tcx, ty, ty::ParamTy::for_def(param)))
        {
            return true;
        }

        let Some(parent) = generics.parent else { break };
        generics = tcx.generics_of(parent);
    }
    false
}

struct DropsGenericsTyVisitor<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    drops_generics: &'a mut FxHashSet<ty::ParamTy>,
}

impl<'tcx> DropsGenericsTyVisitor<'_, 'tcx> {
    fn visit_ty(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> FxHashSet<ty::ParamTy> {
        let mut drops_generics = FxHashSet::default();
        Self::visit_ty_with(tcx, ty, &mut drops_generics);
        drops_generics
    }
    fn visit_ty_with(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, drops_generics: &mut FxHashSet<ty::ParamTy>) {
        let mut visitor = DropsGenericsTyVisitor { tcx, drops_generics };
        visitor.visit_ty(ty);
    }
}

impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for DropsGenericsTyVisitor<'_, 'tcx> {
    fn visit_ty(&mut self, ty: Ty<'tcx>) -> Self::Result {
        match ty.kind() {
            ty::Adt(adt, _) if adt.is_manually_drop() || adt.is_union() || adt.is_payloadfree() => {},
            ty::Adt(adt, args) if adt.is_phantom_data() => args.visit_with(self),
            ty::Adt(adt, args) => {
                let drop_trait_did = self.tcx.require_lang_item(hir::LangItem::Drop, None);
                let drop_fn_did = self
                    .tcx
                    .associated_items(drop_trait_did)
                    .filter_by_name_unhygienic(sym::drop)
                    .next()
                    .map(|assoc_item| assoc_item.def_id);
                self.tcx.for_each_relevant_impl(drop_trait_did, ty, |impl_did| {
                    let &drop_fn_did = drop_fn_did
                        .and_then(|drop_fn_did| self.tcx.impl_item_implementor_ids(impl_did).get(&drop_fn_did))
                        .expect("missing `core::ops::Drop::drop` method");
                    let body = self.tcx.optimized_mir(drop_fn_did);
                    DropsGenericsMirVisitor::visit_body_with(self.tcx, body, args, &mut self.drops_generics);
                });
                for field in adt.all_fields() {
                    field.ty(self.tcx, args).visit_with(self);
                }
            },
            ty::Array(ty, _) | ty::Slice(ty) => ty.visit_with(self),
            ty::Tuple(tys) => tys.visit_with(self),
            ty::Pat(..) => {},
            ty::RawPtr(..) | ty::Ref(..) => {},
            ty::FnDef(..) | ty::FnPtr(_) => {},
            ty::Dynamic(..) => {},
            ty::Closure(_, args)
            | ty::CoroutineClosure(_, args)
            | ty::Coroutine(_, args)
            | ty::CoroutineWitness(_, args) => args.visit_with(self),
            ty::Alias(_, _) => todo!(),
            &ty::Param(param) => self.drops_generics.insert(param),
            _ => {},
        }
    }
}

struct DropsGenericsMirVisitor<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'tcx mir::Body<'tcx>,
    args: ty::GenericArgsRef<'tcx>,
    drops_generics: &'a mut FxHashSet<ty::ParamTy>,
}

impl<'tcx> DropsGenericsMirVisitor<'_, 'tcx> {
    fn visit_body_with(
        tcx: TyCtxt<'tcx>,
        body: &'tcx mir::Body<'tcx>,
        args: ty::GenericArgsRef<'tcx>,
        drops_generics: &mut FxHashSet<ty::ParamTy>,
    ) {
        let mut visitor = DropsGenericsMirVisitor {
            tcx,
            body,
            args,
            drops_generics,
        };
        visitor.visit_body(body);
    }
}

impl<'tcx> Visitor<'tcx> for DropsGenericsMirVisitor<'_, 'tcx> {
    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, _location: mir::Location) {
        match &terminator.kind {
            mir::TerminatorKind::Drop { place, .. } => DropsGenericsTyVisitor::visit_ty_with(
                self.tcx,
                place.ty(self.body, self.tcx).ty,
                &mut self.drops_generics,
            ),
            mir::TerminatorKind::Call { func, .. } | mir::TerminatorKind::TailCall { func, .. }
                if let Some(constant) = func.constant()
                    && let &ty::FnDef(fn_did, args) = ty::EarlyBinder::bind(constant.const_.ty())
                        .instantiate(self.tcx, self.args)
                        .kind() =>
            {
                let body = if self.tcx.is_mir_available(fn_did) {
                    self.tcx.optimized_mir(fn_did)
                } else if self.tcx.is_ctfe_mir_available(fn_did) {
                    self.tcx.mir_for_ctfe(fn_did)
                } else {
                    return;
                };
                DropsGenericsMirVisitor::visit_body_with(self.tcx, body, args, &mut self.drops_generics);
            },
            _ => {},
        }
    }
}
