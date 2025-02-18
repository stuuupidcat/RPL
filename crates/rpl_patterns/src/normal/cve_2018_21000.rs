pub mod u8_to_t {
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

        fn visit_item(&mut self, i: &'tcx rustc_hir::Item<'tcx>) -> Self::Result {
            match i.kind {
                hir::ItemKind::Fn { .. } => {},
                _ => return,
            }
            intravisit::walk_item(self, i);
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
                let pattern = pattern_misordered_params(self.pcx);
                for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                    let span = matches[pattern.from_raw_parts].span_no_inline(body);
                    debug!(?span);
                    self.tcx.dcx().emit_err(crate::errors::MisorderedParameters { span });
                }
            }
        }
    }

    struct PatternMisorderedParams<'pcx> {
        pattern: &'pcx pat::Pattern<'pcx>,
        fn_pat: &'pcx pat::Fn<'pcx>,
        from_raw_parts: pat::Location,
    }

    #[rpl_macros::pattern_def]
    fn pattern_misordered_params(pcx: PatCtxt<'_>) -> PatternMisorderedParams<'_> {
        let from_raw_parts;
        let pattern = rpl! {
            #[meta($T:ty)]
            fn $pattern(..) -> _ = mir! {

                type VecU8 = std::vec::Vec::<u8>;
                type VecT = std::vec::Vec::<$T>;

                let $from_vec: VecU8 = _;
                let $from_vec_mut_borrow: &mut VecU8 = &mut $from_vec;
                let $from_vec_mut_ptr: *mut u8 = std::vec::Vec::as_mut_ptr(move $from_vec_mut_borrow); // FIXME: std::vec::Vec::<u8>::as_mut_ptr ?;
                let $from_vec_borrow1: &VecU8 = &$from_vec;
                let $from_vec_capacity: usize = std::vec::Vec::capacity(move $from_vec_borrow1);
                let $size_of_t1: usize = std::mem::size_of::<$T>();
                let $to_vec_capacity: usize = Div(move $from_vec_capacity, move $size_of_t1);
                let $from_vec_borrow2: &VecU8 = &$from_vec;
                let $from_vec_len: usize = std::vec::Vec::len(move $from_vec_borrow2);
                let $size_of_t2: usize = std::mem::size_of::<$T>();
                let $to_vec_len: usize = Div(move $from_vec_len, move $size_of_t2);
                let $to_vec_ptr: *mut $T = copy $from_vec_mut_ptr as *mut $T (PtrToPtr);
                #[export(from_raw_parts)]
                let $ret: VecT =
                    std::vec::Vec::from_raw_parts(move $to_vec_ptr, copy $to_vec_capacity, copy $to_vec_len);
            }
        };
        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        PatternMisorderedParams {
            pattern,
            fn_pat,
            from_raw_parts,
        }
    }
}

pub mod t_to_u8 {
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

        fn visit_item(&mut self, i: &'tcx rustc_hir::Item<'tcx>) -> Self::Result {
            match i.kind {
                hir::ItemKind::Fn { .. } => {},
                _ => return,
            }
            intravisit::walk_item(self, i);
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
                let pattern = pattern_misordered_params(self.pcx);
                for matches in CheckMirCtxt::new(self.tcx, self.pcx, body, pattern.pattern, pattern.fn_pat).check() {
                    let span = matches[pattern.from_raw_parts].span_no_inline(body);
                    debug!(?span);
                    self.tcx.dcx().emit_err(crate::errors::MisorderedParameters { span });
                }
            }
        }
    }

    struct PatternMisorderedParams<'pcx> {
        pattern: &'pcx pat::Pattern<'pcx>,
        fn_pat: &'pcx pat::Fn<'pcx>,
        from_raw_parts: pat::Location,
    }

    #[rpl_macros::pattern_def]
    fn pattern_misordered_params(pcx: PatCtxt<'_>) -> PatternMisorderedParams<'_> {
        let from_raw_parts;
        let pattern = rpl! {
            #[meta($T:ty)]
            fn $pattern(..) -> _ = mir! {

                type VecT = std::vec::Vec::<$T>;
                type VecU8 = std::vec::Vec::<u8>;

                let $from_vec: VecT = _;

                // FIXME: Part3 needs to be after Part1/2

                /* Part3 */
                let $from_vec_mut_borrow: &mut VecT = &mut $from_vec;
                let $from_vec_mut_ptr: *mut $T = std::vec::Vec::as_mut_ptr(move $from_vec_mut_borrow);

                /* Part1 */
                let $from_vec_borrow2: &VecT = &$from_vec;
                let $from_vec_len: usize = std::vec::Vec::len(move $from_vec_borrow2);
                let $size_of_t2: usize = std::mem::size_of::<$T>();
                let $to_vec_len: usize = Mul(move $from_vec_len, move $size_of_t2);

                /* Part2 */
                let $from_vec_borrow1: &VecT = &$from_vec;
                let $from_vec_capacity: usize = std::vec::Vec::capacity(move $from_vec_borrow1);
                let $size_of_t1: usize = std::mem::size_of::<$T>();
                let $to_vec_capacity: usize = Mul(move $from_vec_capacity, move $size_of_t1);



                let $to_vec_ptr: *mut u8 = copy $from_vec_mut_ptr as *mut u8 (PtrToPtr);
                #[export(from_raw_parts)]
                let $ret: VecU8 =
                    std::vec::Vec::from_raw_parts(move $to_vec_ptr, copy $to_vec_capacity, copy $to_vec_len);
            }
        };
        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        PatternMisorderedParams {
            pattern,
            fn_pat,
            from_raw_parts,
        }
    }
}
