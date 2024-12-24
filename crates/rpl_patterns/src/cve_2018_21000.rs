pub mod u8_to_t {
    use rpl_context::PatCtxt;
    use rustc_hir as hir;
    use rustc_hir::def_id::LocalDefId;
    use rustc_hir::intravisit::{self, Visitor};
    use rustc_middle::hir::nested_filter::All;
    use rustc_middle::ty::TyCtxt;
    use rustc_span::{Span, Symbol};

    use rpl_mir::{pat, CheckMirCtxt};

    #[instrument(level = "info", skip(tcx, pcx))]
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
                hir::ItemKind::Fn(..) => {},
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
                #[allow(irrefutable_let_patterns)]
                if let pattern_misordered_params = pattern_misordered_params(self.pcx)
                    && let Some(matches) =
                        CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_misordered_params.fn_pat).check()
                    && let Some(from_raw_parts) = matches[pattern_misordered_params.from_raw_parts]
                    && let span = from_raw_parts.span_no_inline(body)
                {
                    debug!(?span);
                    self.tcx.dcx().emit_err(crate::errors::MisorderedParameters { span });
                }
            }
        }
    }

    struct PatternMisorderedParams<'pcx> {
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
                type Cap = alloc::raw_vec::Cap;
                type RawVecInner = alloc::raw_vec::RawVecInner;
                type RawVecT = alloc::raw_vec::RawVec::<$T>;

                let from_vec: VecU8 = _; // _1
                let mut from_vec_mut_borrow: &mut VecU8;         // _3
                let mut from_vec_non_null: NonNullU8; // _17
                let mut from_vec_const_ptr: *const u8; // _16
                let mut from_vec_mut_ptr: *mut u8; // _2
                let mut from_vec_immutable_borrow_1: &VecU8; // _6
                let mut from_vec_immutable_borrow_2: &VecU8; // _11
                let mut from_vec_cap: usize; // _5
                let mut tsize1: usize; // _7
                let mut tsize2: usize; // _12
                let mut to_vec_cap: usize; // _4
                let mut from_vec_len: usize; // _10
                let to_vec_len: usize; // _9
                let mut to_vec_mut_ptr: *mut $T; // _14
                let to_vec_wrong_cap_1: Cap; // _20
                let mut to_vec_wrong_cap_2: Cap; // _22
                let mut to_vec_const_ptr: *const u8; // _25
                let mut to_vec_non_null: NonNullU8; // _24
                let mut to_vec_unique: UniqueU8; // _23
                let mut to_vec_raw_inner: RawVecInner; // _21
                let mut to_vec_raw: RawVecT; // _19
                let mut to_vec: VecT; // _0


                // _3 = &mut _1
                from_vec_mut_borrow = &mut from_vec;
                // _17 = copy (((((*_3).0: alloc::raw_vec::RawVec<u8>).0: alloc::raw_vec::RawVecInner).0: std::ptr::Unique<u8>).0: std::ptr::NonNull<u8>);
                from_vec_non_null = copy (*from_vec_mut_borrow).buf.inner.ptr.pointer;
                // _16 = copy (_17.0: *const u8);
                from_vec_const_ptr = copy (from_vec_non_null.pointer);
                // _2 = copy _16 as *mut u8 (PtrToPtr);
                from_vec_mut_ptr = copy from_vec_const_ptr as *mut u8 (PtrToPtr);
                // _6 = &_1
                from_vec_immutable_borrow_1 = &from_vec;
                // _5 = copy (((((*_6).0: alloc::raw_vec::RawVec<u8>).0: alloc::raw_vec::RawVecInner).1: alloc::raw_vec::Cap).0: usize);
                from_vec_cap = copy (*from_vec_immutable_borrow_1).buf.inner.cap.0;
                // _7 = SizeOf(T)
                tsize1 = SizeOf($T);
                // _4 = Div(move _5, move _7);
                to_vec_cap = Div(move from_vec_cap, move tsize1);
                // _11 = &_1
                from_vec_immutable_borrow_2 = &from_vec;
                // _10 = copy (((*_11).1: usize);
                from_vec_len = copy (*from_vec_immutable_borrow_2).len;
                // _12 = SizeOf(T)
                tsize2 = SizeOf($T);
                // _9 = Div(move _10, move _12);
                to_vec_len = Div(move from_vec_len, move tsize2);
                // _14 = copy _2 as *mut T (PtrToPtr);
                to_vec_mut_ptr = copy from_vec_mut_ptr as *mut $T (PtrToPtr);
                // _20 = #[ctor] alloc::raw_vec::Cap(copy _9);
                to_vec_wrong_cap_1 = #[ctor] alloc::raw_vec::Cap(copy to_vec_len);
                // _22 = move _20;
                to_vec_wrong_cap_2 = move to_vec_wrong_cap_1;
                // _25 = copy _14 as *const u8 (PtrToPtr);
                to_vec_const_ptr = copy to_vec_mut_ptr as *const u8 (PtrToPtr);
                // _24 = std::ptr::NonNull::<u8> { pointer: copy _25 };
                to_vec_non_null = core::ptr::non_null::NonNull::<u8> {
                    pointer: copy to_vec_const_ptr
                };
                // _23 = std::ptr::Unique::<u8> { pointer: move _24, _marker: const std::marker::PhantomData::<u8> };
                to_vec_unique = core::ptr::unique::Unique::<u8> {
                    pointer: move to_vec_non_null,
                    _marker: const core::marker::PhantomData::<u8>
                };
                // _21 = alloc::raw_vec::RawVecInner { ptr: move _23, cap: copy _22, alloc: const std::alloc::Global };
                to_vec_raw_inner = alloc::raw_vec::RawVecInner {
                    ptr: move to_vec_unique,
                    cap: copy to_vec_wrong_cap_2,
                    alloc: const alloc::alloc::Global
                };
                // _19 = alloc::raw_vec::RawVec::<T> { inner: move _21, _marker: const ZeroSized: std::marker::PhantomData<T> };
                to_vec_raw = alloc::raw_vec::RawVec::<$T> {
                    inner: move to_vec_raw_inner,
                    _marker: const core::marker::PhantomData::<$T>
                };
                // _0 = std::vec::Vec::<T> { buf: move _19, len: copy _4 };
                #[export(from_raw_parts)]
                to_vec = alloc::vec::Vec::<$T> {
                    buf: move to_vec_raw,
                    len: copy to_vec_cap
                };

            }
        };
        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        PatternMisorderedParams { fn_pat, from_raw_parts }
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

    #[instrument(level = "info", skip(tcx, pcx))]
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
                hir::ItemKind::Fn(..) => {},
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
                #[allow(irrefutable_let_patterns)]
                if let pattern_misordered_params = pattern_misordered_params(self.pcx)
                    && let Some(matches) =
                        CheckMirCtxt::new(self.tcx, self.pcx, body, pattern_misordered_params.fn_pat).check()
                    && let Some(from_raw_parts) = matches[pattern_misordered_params.from_raw_parts]
                    && let span = from_raw_parts.span_no_inline(body)
                {
                    debug!(?span);
                    self.tcx.dcx().emit_err(crate::errors::MisorderedParameters { span });
                }
            }
        }
    }

    struct PatternMisorderedParams<'pcx> {
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
                type Cap = alloc::raw_vec::Cap;
                type RawVecInner = alloc::raw_vec::RawVecInner;
                type RawVecU8 = alloc::raw_vec::RawVec::<u8>;

                #[export(from_raw_parts)]
                let from_vec: VecT = _; // _1
                let mut from_vec_immutable_borrow_1: &VecT; // _4
                let mut from_vec_immutable_borrow_2: &VecT; // _8
                let mut from_vec_mut_borrow: &mut VecT; // _11
                let from_vec_cap: usize; // _3
                let mut from_vec_len: usize; // _7
                let mut from_vec_non_null: NonNullU8; // _15
                let mut from_vec_const_ptr: *const u8; // _14
                let mut from_vec_mut_ptr: *mut T; // _10
                let mut to_vec_mut_ptr: *mut u8; // _12
                let mut to_vec_const_ptr: *const u8; // _21
                let tsize_1: usize; // _5
                let tsize_2: usize; // _9
                let to_vec_cap: usize; // _2
                let to_vec_len: usize; // _6
                let to_vec_wrong_cap: Cap; // _17
                let to_vec_non_null: NonNullU8; // _20
                let to_vec_unique: UniqueU8; // _19
                let mut to_vec_raw_inner: RawVecInner; // _18
                let mut to_vec_raw: RawVecU8; // _16
                let to_vec: VecU8; // _0

                // _4 = &_1
                from_vec_immutable_borrow_1 = &from_vec;
                // _3 = copy (((*_4).0: alloc::raw_vec::RawVec<$T>).1: alloc::raw_vec::Cap).0;
                from_vec_cap = copy (*from_vec_immutable_borrow_1).buf.inner.cap.0;
                // _5 = SizeOf($T)
                tsize_1 = SizeOf($T);
                // _2 = Mul(move _3, move _5);
                to_vec_cap = Mul(move from_vec_cap, move tsize_1);
                // _8 = &_1
                from_vec_immutable_borrow_2 = &from_vec;
                // _7 = copy ((*_8).1: usize);
                from_vec_len = copy (*from_vec_immutable_borrow_2).len;
                // _9 = SizeOf($T)
                tsize_2 = SizeOf($T);
                // _6 = Mul(move _7, move _9);
                to_vec_len = Mul(move from_vec_len, move tsize_2);
                // _11 = &mut _1
                from_vec_mut_borrow = &mut from_vec;
                // _15 = copy (((((*_11).0: alloc::raw_vec::RawVec<$T>).0: alloc::raw_vec::RawVecInner).0: std::ptr::Unique<u8>).0: std::ptr::NonNull<u8>;
                from_vec_non_null = copy (*from_vec_mut_borrow).buf.inner.ptr.pointer;
                // _14 = copy (_15.0: *const u8);
                from_vec_const_ptr = copy (from_vec_non_null.pointer);
                // _10 = copy _14 as *mut T (PtrToPtr);
                from_vec_mut_ptr = copy from_vec_const_ptr as *mut $T (PtrToPtr);
                // _12 = copy _10 as *mut u8 (PtrToPtr);
                to_vec_mut_ptr = copy from_vec_mut_ptr as *mut u8 (PtrToPtr);
                // _17 = #[ctor] alloc::raw_vec::Cap(copy _6);
                to_vec_wrong_cap = #[ctor] alloc::raw_vec::Cap(copy to_vec_len);
                // _21 = copy _12 as *const u8 (PtrToPtr);
                to_vec_const_ptr = copy to_vec_mut_ptr as *const u8 (PtrToPtr);
                // _20 = std::ptr::NonNull::<u8> { pointer: copy _21 };
                to_vec_non_null = core::ptr::non_null::NonNull::<u8> {
                    pointer: copy to_vec_const_ptr
                };
                // _19 = std::ptr::Unique::<u8> { pointer: move _20, _marker: const std::marker::PhantomData::<u8> };
                to_vec_unique = core::ptr::unique::Unique::<u8> {
                    pointer: move to_vec_non_null,
                    _marker: const core::marker::PhantomData::<u8>
                };
                // _18 = alloc::raw_vec::RawVecInner { ptr: move _19, cap: copy _17, alloc: const std::alloc::Global };
                to_vec_raw_inner = alloc::raw_vec::RawVecInner {
                    ptr: move to_vec_unique,
                    cap: copy to_vec_wrong_cap,
                    alloc: const alloc::alloc::Global
                };
                // _16 = alloc::raw_vec::RawVec::<u8> { inner: move _18, _marker: const std::marker::PhantomData::<u8> };
                to_vec_raw = alloc::raw_vec::RawVec::<u8> {
                    inner: move to_vec_raw_inner,
                    _marker: const core::marker::PhantomData::<u8>
                };
                // _0 = std::vec::Vec::<u8> { buf: move _16, len: copy _2 };
                to_vec = alloc::vec::Vec::<u8> {
                    buf: move to_vec_raw,
                    len: copy to_vec_cap
                };
            }
        };
        let fn_pat = pattern.fns.get_fn_pat(Symbol::intern("pattern")).unwrap();

        PatternMisorderedParams { fn_pat, from_raw_parts }
    }
}
