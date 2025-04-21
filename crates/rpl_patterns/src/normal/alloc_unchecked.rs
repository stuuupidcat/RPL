
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter::All;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

use rpl_context::{PatCtxt, pat};
use rpl_mir::CheckMirCtxt;

use crate::lints::UNCHECKED_ALLOCATED_POINTER;

#[instrument(level = "info", skip_all)]
pub fn check_item(tcx: TyCtxt<'_>, pcx: PatCtxt<'_>, item_id: hir::ItemId) {
    let item = tcx.hir().item(item_id);
    // let def_id = item_id.owner_id.def_id;
    let mut check_ctxt = CheckFnCtxt { tcx, pcx };
    check_ctxt.visit_item(item);
}

struct CheckFnCtxt<'pcx, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pcx: PatCtxt<'pcx>,
}

impl<'tcx> Visitor<'tcx> for CheckFnCtxt<'_, 'tcx> {
    type NestedFilter = All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
        match item.kind {
            hir::ItemKind::Trait(hir::IsAuto::No, hir::Safety::Safe, ..)
            | hir::ItemKind::Impl(_)
            | hir::ItemKind::Fn { .. } => {},
            _ => return,
        }
        intravisit::walk_item(self, item);
    }

    #[instrument(level = "info", skip(self, kind, decl, _span))]
    fn visit_fn(
        &mut self,
        kind: intravisit::FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body_id: hir::BodyId,
        _span: Span,
        def_id: LocalDefId,
    ) -> Self::Result {
        // let attrs: Vec<_> = self
        //     .tcx
        //     .get_attrs_by_path(def_id.to_def_id(), &[Symbol::intern("rpl"), Symbol::intern("check")])
        //     .collect();
        // info!("attrs: {:?}", attrs);
        // if attrs.is_empty() {
        //     return;
        // }

        if self.tcx.is_mir_available(def_id) {
            let body = self.tcx.optimized_mir(def_id);

            let pattern = alloc_misaligned_cast(self.pcx);

            for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                let alloc = matches[pattern.alloc].span_no_inline(body);
                let write = matches[pattern.cast].span_no_inline(body);
                let ty = matches[pattern.ty.idx];

                self.tcx.emit_node_span_lint(
                    UNCHECKED_ALLOCATED_POINTER,
                    self.tcx.local_def_id_to_hir_id(def_id),
                    write,
                    crate::errors::UncheckedAllocatedPointer { alloc, write, ty },
                );
            }
        }
        intravisit::walk_fn(self, kind, decl, body_id, def_id);
    }
}

struct Pattern2<'pcx> {
    pattern: &'pcx pat::Pattern<'pcx>,
    fn_pat: &'pcx pat::Fn<'pcx>,
    alloc: pat::Location,
    cast: pat::Location,
    ty: pat::TyVar,
}

#[rpl_macros::pattern_def]
fn alloc_misaligned_cast(pcx: PatCtxt<'_>) -> Pattern2<'_> {
    let alloc;
    let cast;
    let ty;
    let pattern = rpl! {
        #[meta(#[export(ty)] $T:ty, $alignment: const(usize))]
        fn $pattern(..) -> _ = mir! {
            let $layout_result: core::result::Result<core::alloc::Layout, _> = alloc::alloc::Layout::from_size_align(_, const $alignment);
            let $layout: core::alloc::Layout = _;
            #[export(alloc)]
            let $ptr_1: *mut u8 = alloc::alloc::alloc(move $layout);
            #[export(cast)]
            let $ptr_2: *mut $T = move $ptr_1 as *mut $T (PtrToPtr);
        }
    };
    let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

    Pattern2 {
        pattern,
        fn_pat,
        alloc,
        cast,
        ty,
    }
}
