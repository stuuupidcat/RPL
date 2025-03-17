pub mod u8_to_t {
    use rpl_context::PatCtxt;
    use rustc_hir as hir;
    use rustc_hir::def_id::LocalDefId;
    use rustc_hir::intravisit::{self, Visitor};
    use rustc_middle::hir::nested_filter::All;
    use rustc_middle::ty::TyCtxt;
    use rustc_span::{Span, Symbol};

    use rpl_mir::{pat, CheckMirCtxt};

    use crate::lints::MISORDERED_PARAMETERS;

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
                    self.tcx.emit_node_span_lint(
                        MISORDERED_PARAMETERS,
                        self.tcx.local_def_id_to_hir_id(def_id),
                        span,
                        crate::errors::MisorderedParameters { span },
                    );
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

                type VecU8 = alloc::vec::Vec::<u8>;
                type VecT = alloc::vec::Vec::<$T>;
                type NonNullU8 = core::ptr::non_null::NonNull::<u8>;
                type UniqueU8 = core::ptr::unique::Unique::<u8>;
                type RawVecInner = alloc::raw_vec::RawVecInner;
                type RawVecT = alloc::raw_vec::RawVec::<$T>;
                type UsizeNoHighBit = core::num::niche_types::UsizeNoHighBit;

                let $from_vec: VecU8 = _; // _1
                let $from_vec_mut_borrow: &mut VecU8; // _3
                let $from_vec_non_null: NonNullU8; // _16
                let $from_vec_mut_ptr: *mut u8; // _2
                let $from_vec_immutable_borrow_1: &VecU8; // _6
                let $from_vec_cap_usize_no_high_bit: UsizeNoHighBit; // _18
                let $from_vec_cap_usize: usize; // _5;
                let $tsize1: usize; // _7
                let $to_vec_cap: usize; // _4
                let $from_vec_immutable_borrow_2: &VecU8; // _11
                let $from_vec_len: usize; // _10
                let $to_vec_len_usize: usize; // _9
                let $to_vec_mut_ptr: *mut $T; // _14
                let $tsize2: usize; // _12
                let $to_vec_len_usize_no_high_bit: UsizeNoHighBit; // _21
                let $to_vec_len_usize_no_high_bit_copy: UsizeNoHighBit; // _23
                let $to_vec_const_ptr: *const u8;//_26
                let $to_vec_non_null: NonNullU8; // _25
                let $to_vec_unique: UniqueU8; // _24
                let $to_vec_raw_inner: RawVecInner; // _22
                let $to_vec_raw: RawVecT; // _20
                let $to_vec: VecT; // _0


                $from_vec_mut_borrow = &mut $from_vec;
                $from_vec_non_null = copy (*$from_vec_mut_borrow).buf.inner.ptr.pointer;
                $from_vec_mut_ptr = copy $from_vec_non_null as *mut u8 (Transmute);
                $from_vec_immutable_borrow_1 = &$from_vec;
                $from_vec_cap_usize_no_high_bit = copy (*$from_vec_immutable_borrow_1).buf.inner.cap;
                $from_vec_cap_usize = copy $from_vec_cap_usize_no_high_bit as usize (Transmute);
                $tsize1 = SizeOf($T);
                $to_vec_cap = Div(move $from_vec_cap_usize, move $tsize1);
                $from_vec_immutable_borrow_2 = &$from_vec;
                $from_vec_len = copy (*$from_vec_immutable_borrow_2).len;
                $tsize2 = SizeOf($T);
                $to_vec_len_usize = Div(move $from_vec_len, move $tsize2);
                $to_vec_mut_ptr = copy $from_vec_mut_ptr as *mut $T (PtrToPtr);
                $to_vec_len_usize_no_high_bit = #[ctor] core::num::niche_types::UsizeNoHighBit(copy $to_vec_len_usize);
                $to_vec_len_usize_no_high_bit_copy = copy $to_vec_len_usize_no_high_bit;
                $to_vec_const_ptr = copy $to_vec_mut_ptr as *const u8 (PtrToPtr);
                $to_vec_non_null = core::ptr::non_null::NonNull::<u8> {
                    pointer: copy $to_vec_const_ptr
                };
                $to_vec_unique = core::ptr::unique::Unique::<u8> {
                    pointer: move $to_vec_non_null,
                    _marker: const core::marker::PhantomData::<u8>
                };
                $to_vec_raw_inner = alloc::raw_vec::RawVecInner {
                    ptr: move $to_vec_unique,
                    cap: copy $to_vec_len_usize_no_high_bit_copy,
                    alloc: const alloc::alloc::Global
                };
                $to_vec_raw = alloc::raw_vec::RawVec::<$T> {
                    inner: move $to_vec_raw_inner,
                    _marker: const core::marker::PhantomData::<$T>
                };
                #[export(from_raw_parts)]
                $to_vec = alloc::vec::Vec::<$T> {
                    buf: move $to_vec_raw,
                    len: copy $to_vec_cap
                };

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

    use crate::lints::MISORDERED_PARAMETERS;

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

                    self.tcx.emit_node_span_lint(
                        MISORDERED_PARAMETERS,
                        self.tcx.local_def_id_to_hir_id(def_id),
                        span,
                        crate::errors::MisorderedParameters { span },
                    );
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
                type VecU8 = alloc::vec::Vec::<u8>;
                type VecT = alloc::vec::Vec::<$T>;
                type NonNullU8 = core::ptr::non_null::NonNull::<u8>;
                type UniqueU8 = core::ptr::unique::Unique::<u8>;
                type RawVecInner = alloc::raw_vec::RawVecInner;
                type RawVecU8 = alloc::raw_vec::RawVec::<u8>;
                type UsizeNoHighBit = core::num::niche_types::UsizeNoHighBit;

                let $from_vec: VecT = _; // _1
                let $from_vec_mut_borrow: &mut VecT; // _3
                let $from_vec_non_null: NonNullU8; // _13
                let $from_vec_mut_ptr: *mut $T; // _2
                let $from_vec_immutable_borrow_1: &VecT; // _6
                let $from_vec_cap_usize_no_high_bit: UsizeNoHighBit; // _18
                let $from_vec_cap_usize: usize; // _5;
                let $tsize1: usize; // _7
                let $to_vec_cap: usize; // _4
                let $from_vec_immutable_borrow_2: &VecT; // _10
                let $from_vec_len: usize; // _9
                let $to_vec_len_usize: usize; // _8
                let $to_vec_mut_ptr: *mut u8; // _12
                let $tsize2: usize; // _11
                let $to_vec_len_usize_no_high_bit: UsizeNoHighBit; // _18
                let $to_vec_const_ptr: *const u8;//_22
                let $to_vec_non_null: NonNullU8; // _21
                let $to_vec_unique: UniqueU8; // _20
                let $to_vec_raw_inner: RawVecInner; // _19
                let $to_vec_raw: RawVecU8; // _17
                let $to_vec: VecU8; // _0


                $from_vec_mut_borrow = &mut $from_vec;
                $from_vec_non_null = copy (*$from_vec_mut_borrow).buf.inner.ptr.pointer;
                $from_vec_mut_ptr = copy $from_vec_non_null as *mut $T (Transmute);
                $from_vec_immutable_borrow_1 = &$from_vec;
                $from_vec_cap_usize_no_high_bit = copy (*$from_vec_immutable_borrow_1).buf.inner.cap;
                $from_vec_cap_usize = copy $from_vec_cap_usize_no_high_bit as usize (Transmute);
                $tsize1 = SizeOf($T);
                $to_vec_cap = Mul(move $from_vec_cap_usize, move $tsize1);
                $from_vec_immutable_borrow_2 = &$from_vec;
                $from_vec_len = copy (*$from_vec_immutable_borrow_2).len;
                $tsize2 = SizeOf($T);
                $to_vec_len_usize = Mul(move $from_vec_len, move $tsize2);
                $to_vec_mut_ptr = copy $from_vec_mut_ptr as *mut u8 (PtrToPtr);
                $to_vec_len_usize_no_high_bit = #[ctor] core::num::niche_types::UsizeNoHighBit(copy $to_vec_len_usize);
                $to_vec_const_ptr = copy $to_vec_mut_ptr as *const u8 (PtrToPtr);
                $to_vec_non_null = core::ptr::non_null::NonNull::<u8> {
                    pointer: copy $to_vec_const_ptr
                };
                $to_vec_unique = core::ptr::unique::Unique::<u8> {
                    pointer: move $to_vec_non_null,
                    _marker: const core::marker::PhantomData::<u8>
                };
                $to_vec_raw_inner = alloc::raw_vec::RawVecInner {
                    ptr: move $to_vec_unique,
                    cap: copy $to_vec_len_usize_no_high_bit,
                    alloc: const alloc::alloc::Global
                };
                $to_vec_raw = alloc::raw_vec::RawVec::<u8> {
                    inner: move $to_vec_raw_inner,
                    _marker: const core::marker::PhantomData::<u8>
                };
                #[export(from_raw_parts)]
                $to_vec = alloc::vec::Vec::<u8> {
                    buf: move $to_vec_raw,
                    len: copy $to_vec_cap
                };
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
