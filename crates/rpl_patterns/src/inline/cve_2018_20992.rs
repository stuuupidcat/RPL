pub mod extend {
    use rpl_context::PatCtxt;
    use rustc_hir as hir;
    use rustc_hir::def_id::LocalDefId;
    use rustc_hir::intravisit::{self, Visitor};
    use rustc_middle::hir::nested_filter::All;
    use rustc_middle::ty::TyCtxt;
    use rustc_span::{Span, Symbol};

    use rpl_mir::{pat, CheckMirCtxt};

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

        #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
        fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
            match item.kind {
                hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn{..} => {},
                _ => return,
            }
            intravisit::walk_item(self, item);
        }

        fn visit_fn(
            &mut self,
            _kind: intravisit::FnKind<'tcx>,
            _decl: &'tcx hir::FnDecl<'tcx>,
            _body_id: hir::BodyId,
            _span: Span,
            def_id: LocalDefId,
        ) -> Self::Result {
            if self.tcx.visibility(def_id).is_public() && self.tcx.is_mir_available(def_id) {
                let body = self.tcx.optimized_mir(def_id);
                let pattern = pattern_vec_set_len_to_extend(self.pcx);
                for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                    let set_len = matches[pattern.set_len_use].span_no_inline(body);
                    let vec = matches[pattern.vec].span_no_inline(body);
                    self.tcx
                        .dcx()
                        .emit_err(crate::errors::VecSetLenToExtend { set_len, vec });
                }
            }
        }
    }

    struct VecSetLenToExtend<'pcx> {
        pattern: &'pcx pat::Pattern<'pcx>,
        fn_pat: &'pcx pat::Fn<'pcx>,
        vec: pat::Location,
        set_len_use: pat::Location,
    }

    #[rpl_macros::pattern_def]
    fn pattern_vec_set_len_to_extend(pcx: PatCtxt<'_>) -> VecSetLenToExtend<'_> {
        let vec;
        let set_len_use;
        let pattern = rpl! {
            #[meta($T:ty)]
            fn $pattern(..) -> _ = mir! {

                type VecT = alloc::vec::Vec::<$T>;
                // type VecTRef = &alloc::vec::Vec::<$T>;
                type VecTMutRef = &mut alloc::vec::Vec::<$T>;


                let $vec: VecT;   // _1;
                // let vec_ref: VecTRef; // _5;
                let $new_len: usize; // _2;
                // let old_len: usize; // _4; ..unused
                let $vec_mut_ref: VecTMutRef; // _10;

                $new_len = _;
                #[export(vec)]
                $vec = _;
                // _5 = &_1;
                // vec_ref = &vec;
                // _4 = copy ((*_5).1: usize);
                // old_len = copy ((*vec_ref).len);
                // _10 = &mut _1;
                $vec_mut_ref = &mut $vec;
                // ((*10).1: usize) = _2;
                #[export(set_len_use)]
                (*$vec_mut_ref).len = copy $new_len;
            }
        };
        // FIXME
        /* let pattern = rpl! {
            fn $pattern(..) -> _ = mir! {
                meta!{$T:ty}

                type VecT = alloc::vec::Vec::<$T>;
                type VecTRef = &alloc::vec::Vec::<$T>;
                type VecTMutRef = &mut alloc::vec::Vec::<$T>;

                let vec: VecT;
                let new_len: usize;
                let vec_ref: VecTRef;
                let old_len: usize;
                let vec_mut_ref: VecTMutRef;
                let cmp: bool;

                vec = _;
                new_len = _;
                vec_ref = &vec;
                old_len = copy ((*vec_ref).len);
                cmp = Lt(move old_len, copy new_len);
                #[export(set_len_use)]
                switchInt(move cmp) {
                    0_usize => {}
                    _ => {
                        vec_mut_ref = &mut vec;
                        (*vec_mut_ref).len = copy new_len;
                    }
                }
            }
        }; */

        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        VecSetLenToExtend {
            pattern,
            fn_pat,
            vec,
            set_len_use,
        }
    }
}

pub mod truncate {
    use rpl_context::PatCtxt;
    use rustc_hir as hir;
    use rustc_hir::def_id::LocalDefId;
    use rustc_hir::intravisit::{self, Visitor};
    use rustc_middle::hir::nested_filter::All;
    use rustc_middle::ty::TyCtxt;
    use rustc_span::{Span, Symbol};

    use rpl_mir::{pat, CheckMirCtxt};

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

        #[instrument(level = "debug", skip_all, fields(?item.owner_id))]
        fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) -> Self::Result {
            match item.kind {
                hir::ItemKind::Trait(hir::IsAuto::No, ..) | hir::ItemKind::Impl(_) | hir::ItemKind::Fn{..} => {},
                _ => return,
            }
            intravisit::walk_item(self, item);
        }

        fn visit_fn(
            &mut self,
            _kind: intravisit::FnKind<'tcx>,
            _decl: &'tcx hir::FnDecl<'tcx>,
            _body_id: hir::BodyId,
            _span: Span,
            def_id: LocalDefId,
        ) -> Self::Result {
            if self.tcx.visibility(def_id).is_public() && self.tcx.is_mir_available(def_id) {
                let body = self.tcx.optimized_mir(def_id);
                let pattern = pattern_vec_set_len_to_extend(self.pcx);
                for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                    let span = matches[pattern.set_len_use].span_no_inline(body);
                    self.tcx.dcx().emit_err(crate::errors::VecSetLenToTruncate { span });
                }
            }
        }
    }

    struct VecSetLenToTruncate<'pcx> {
        pattern: &'pcx pat::Pattern<'pcx>,
        fn_pat: &'pcx pat::Fn<'pcx>,
        set_len_use: pat::Location,
    }

    #[rpl_macros::pattern_def]
    fn pattern_vec_set_len_to_extend(pcx: PatCtxt<'_>) -> VecSetLenToTruncate<'_> {
        let set_len_use;
        let pattern = rpl! {
            #[meta($T:ty)]
            fn $pattern(..) -> _ = mir! {

                type VecT = alloc::vec::Vec::<$T>;
                type VecTRef = &alloc::vec::Vec::<$T>;
                type VecTMutRef = &mut alloc::vec::Vec::<$T>;

                let $vec: VecT;
                let $new_len: usize;
                let $vec_ref: VecTRef;
                let $old_len: usize;
                let $vec_mut_ref: VecTMutRef;
                let $cmp: bool;

                $vec = _;
                $new_len = _;
                $vec_ref = &$vec;
                $old_len = copy ((*$vec_ref).len);
                $cmp = Ge(move $old_len, copy $new_len);
                #[export(set_len_use)]
                switchInt(move $cmp) {
                    0_usize => {}
                    _ => {
                        $vec_mut_ref = &mut $vec;
                        (*$vec_mut_ref).len = copy $new_len;
                    }
                }
            }
        };

        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        VecSetLenToTruncate {
            pattern,
            fn_pat,
            set_len_use,
        }
    }
}
