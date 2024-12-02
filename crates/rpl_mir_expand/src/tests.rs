use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syntax::*;

#[track_caller]
fn test_pass<T: Parse + std::panic::RefUnwindSafe>(input: TokenStream, output: TokenStream)
where
    for<'ecx, 'a> crate::Expand<'ecx, &'a T>: ToTokens,
{
    let value: T = syn::parse2(input).unwrap();
    let patterns = syn::parse_quote!(patterns);
    let expanded = std::panic::catch_unwind(|| {
        let mut tokens = TokenStream::new();
        crate::expand_impl(&value, &patterns, &mut tokens);
        tokens
    })
    .unwrap();
    assert_eq!(
        expanded.to_string().replace(";", ";\n"),
        output.to_string().replace(";", ";\n")
    );
}

macro_rules! pass {
    ($test_struct:ident!( $( $tt:tt )* ), $($output:tt)*) => {
        test_pass::<$test_struct>(quote!($($tt)*), $($output)*)
    };
    ($test_struct:ident!{ $( $tt:tt )* }, $($output:tt)*) => {
        pass!($test_struct!( $($tt)* ), $($output)*)
    };
    ($test_struct:ident![ $( $tt:tt )* ], $($output:tt)*) => {
        pass!($test_struct!( $($tt)* ), $($output)*)
    };
}

#[test]
fn test_ty_var() {
    pass!(
        MetaItem!( $T:ty ),
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);
        },
    );
}

#[test]
fn test_cve_2020_25016() {
    pass!(
        Mir! {
            meta!($T:ty);
            type SliceT = [$T];
            type RefSliceT = &SliceT;
            type PtrSliceT = *const SliceT;
            type PtrU8 = *const u8;
            type SliceU8 = [u8];
            type PtrSliceU8 = *const SliceU8;
            type RefSliceU8 = &SliceU8;

            let from_slice: SliceT = _;
            let from_raw_slice: PtrSliceT = &raw const *from_slice;
            let from_len: usize = Len(from_slice);
            let ty_size: usize = SizeOf($T);
            let to_ptr: PtrU8 = copy from_ptr as PtrU8 (PtrToPtr);
            let to_len: usize = Mul(copy from_len, copy ty_size);
            let to_raw_slice: PtrSliceU8 = *const SliceU8 from (copy to_ptr, copy t_len);
            let to_slice: RefSliceU8 = &*to_raw_slice;
        },
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);
            #[allow(non_snake_case)]
            let SliceT_ty = patterns.mk_slice_ty(T_ty);
            #[allow(non_snake_case)]
            let RefSliceT_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                SliceT_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let PtrSliceT_ty = patterns.mk_raw_ptr_ty(SliceT_ty, ::rustc_middle::mir::Mutability::Not);
            #[allow(non_snake_case)]
            let PtrU8_ty = patterns.mk_raw_ptr_ty(
                patterns.primitive_types.u8, ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let SliceU8_ty = patterns.mk_slice_ty(patterns.primitive_types.u8);
            #[allow(non_snake_case)]
            let PtrSliceU8_ty = patterns.mk_raw_ptr_ty(SliceU8_ty, ::rustc_middle::mir::Mutability::Not);
            #[allow(non_snake_case)]
            let RefSliceU8_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                SliceU8_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            let from_slice_local = patterns.mk_local(SliceT_ty);
            let from_slice_stmt = patterns.mk_assign(from_slice_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let from_raw_slice_local = patterns.mk_local(PtrSliceT_ty);
            let from_raw_slice_stmt = patterns.mk_assign(
                from_raw_slice_local.into_place(),
                ::rpl_mir::pat::Rvalue::RawPtr(
                    ::rustc_middle::mir::Mutability::Not,
                    ::rpl_mir::pat::Place::new(
                        from_slice_local,
                        patterns.mk_projection(&[::rpl_mir::pat::PlaceElem::Deref,])
                    )
                )
            );
            let from_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_len_stmt = patterns.mk_assign(
                from_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Len(from_slice_local.into_place())
            );
            let ty_size_local = patterns.mk_local(patterns.primitive_types.usize);
            let ty_size_stmt = patterns.mk_assign(
                ty_size_local.into_place(),
                ::rpl_mir::pat::Rvalue::NullaryOp(::rustc_middle::mir::NullOp::SizeOf, T_ty)
            );
            let to_ptr_local = patterns.mk_local(PtrU8_ty);
            let to_ptr_stmt = patterns.mk_assign(
                to_ptr_local.into_place(),
                ::rpl_mir::pat::Rvalue::Cast(
                    ::rustc_middle::mir::CastKind::PtrToPtr,
                    ::rpl_mir::pat::Operand::Copy(from_ptr_local.into_place()),
                    PtrU8_ty
                )
            );
            let to_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let to_len_stmt = patterns.mk_assign(
                to_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Mul,
                    Box::new([
                        ::rpl_mir::pat::Operand::Copy(from_len_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(ty_size_local.into_place())
                    ])
                )
            );
            let to_raw_slice_local = patterns.mk_local(PtrSliceU8_ty);
            let to_raw_slice_stmt = patterns.mk_assign(
                to_raw_slice_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::RawPtr(SliceU8_ty, ::rustc_middle::mir::Mutability::Not),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Copy(to_ptr_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(t_len_local.into_place())
                    ])
                )
            );
            let to_slice_local = patterns.mk_local(RefSliceU8_ty);
            let to_slice_stmt = patterns.mk_assign(
                to_slice_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Shared,
                    ::rpl_mir::pat::Place::new(
                        to_raw_slice_local,
                        patterns.mk_projection(&[::rpl_mir::pat::PlaceElem::Deref,])
                    )
                )
            );
        },
    );
}

#[test]
fn test_cve_2020_35892_revised() {
    pass!(
        Mir! {
            meta!($T:ty, $SlabT:ty);

            let self: &mut $SlabT;
            let len: usize;
            let x1: usize;
            let x2: usize;
            let opt: #[lang = "Option"]<usize>;
            let discr: isize;
            let x: usize;
            let start_ref: &usize;
            let end_ref: &usize;
            let start: usize;
            let end: usize;
            let range: core::ops::range::Range<usize>;
            let mut iter: core::ops::range::Range<usize>;
            let mut iter_mut: &mut core::ops::range::Range<usize>;
            let mut base: *mut $T;
            let offset: isize;
            let elem_ptr: *mut $T;
            let cmp: bool;

            len = copy (*self).len;
            range = core::ops::range::Range { start: const 0_usize, end: move len };
            iter = move range;
            loop {
                iter_mut = &mut iter;
                start_ref = &(*iter_mut).start;
                start = copy *start_ref;
                end_ref = &(*iter_mut).end;
                end = copy *end;
                cmp = Lt(move start, copy end);
                switchInt(move cmp) {
                    false => opt = #[lang = "None"],
                    _ => {
                        x1 = copy (*iter_mut).start;
                        x2 = core::iter::range::Step::forward_unchecked(copy x1, const 1_usize);
                        (*iter_mut).start = copy x2;
                        opt = #[lang = "Some"](copy x1);
                    }
                }
                discr = discriminant(opt);
                switchInt(move discr) {
                    0_isize => break,
                    1_isize => {
                        x = copy (opt as Some).0;
                        base = copy (*self).mem;
                        offset = copy x as isize (IntToInt);
                        elem_ptr = Offset(copy base, copy offset);
                        _ = core::ptr::drop_in_place(copy elem_ptr);
                    }
                }
            }
        },
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);

            #[allow(non_snake_case)]
            let SlabT_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let SlabT_ty = patterns.mk_var_ty(SlabT_ty_var);

            let self_local = patterns.mk_self(patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                SlabT_ty,
                ::rustc_middle::mir::Mutability::Mut
            ));
            let len_local = patterns.mk_local(patterns.primitive_types.usize);
            let x1_local = patterns.mk_local(patterns.primitive_types.usize);
            let x2_local = patterns.mk_local(patterns.primitive_types.usize);
            let opt_local = patterns.mk_local(patterns.mk_adt_ty(patterns.mk_path_with_args(
                patterns.mk_lang_item("Option"),
                &[patterns.primitive_types.usize.into()]
            )));
            let discr_local = patterns.mk_local(patterns.primitive_types.isize);
            let x_local = patterns.mk_local(patterns.primitive_types.usize);
            let start_ref_local = patterns.mk_local(patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.primitive_types.usize,
                ::rustc_middle::mir::Mutability::Not
            ));
            let end_ref_local = patterns.mk_local(patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.primitive_types.usize,
                ::rustc_middle::mir::Mutability::Not
            ));
            let start_local = patterns.mk_local(patterns.primitive_types.usize);
            let end_local = patterns.mk_local(patterns.primitive_types.usize);
            let range_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["core", "ops", "range", "Range",]),
                &[patterns.primitive_types.usize.into()]
            )));
            let iter_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["core", "ops", "range", "Range",]),
                &[patterns.primitive_types.usize.into()]
            )));
            let iter_mut_local = patterns.mk_local(patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_path_ty(patterns.mk_path_with_args(
                    patterns.mk_item_path(&["core", "ops", "range", "Range",]),
                    &[patterns.primitive_types.usize.into()]
                )),
                ::rustc_middle::mir::Mutability::Mut
            ));
            let base_local =
                patterns.mk_local(patterns.mk_raw_ptr_ty(T_ty, ::rustc_middle::mir::Mutability::Mut));
            let offset_local = patterns.mk_local(patterns.primitive_types.isize);
            let elem_ptr_local =
                patterns.mk_local(patterns.mk_raw_ptr_ty(T_ty, ::rustc_middle::mir::Mutability::Mut));
            let cmp_local = patterns.mk_local(patterns.primitive_types.bool);
            let len_stmt = patterns.mk_assign(
                len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(::rpl_mir::pat::Place::new(
                    self_local,
                    patterns.mk_projection(&[
                        ::rpl_mir::pat::PlaceElem::Deref,
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(
                            ::rustc_span::Symbol::intern("len")
                        )),
                    ])
                )))
            );
            let range_stmt = patterns.mk_assign(
                range_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["core", "ops", "range", "Range",]),
                            &[]
                        ),
                        patterns.mk_list([
                            ::rustc_span::Symbol::intern("start"),
                            ::rustc_span::Symbol::intern("end")
                        ]).into()
                    ),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Constant(
                            ::rpl_mir::pat::ConstOperand::ScalarInt(0_usize.into())
                        ),
                        ::rpl_mir::pat::Operand::Move(len_local.into_place())
                    ]),
                )
            );
            let iter_stmt = patterns.mk_assign(
                iter_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Move(range_local.into_place()))
            );
            patterns.mk_loop(|patterns| {
                let iter_mut_stmt = patterns.mk_assign(
                    iter_mut_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Ref(
                        ::rpl_mir::pat::RegionKind::ReAny,
                        ::rustc_middle::mir::BorrowKind::Mut {
                            kind: ::rustc_middle::mir::MutBorrowKind::Default
                        },
                        iter_local.into_place()
                    ));
                let start_ref_stmt = patterns.mk_assign(
                    start_ref_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Ref(
                        ::rpl_mir::pat::RegionKind::ReAny,
                        ::rustc_middle::mir::BorrowKind::Shared,
                        ::rpl_mir::pat::Place::new(iter_mut_local, patterns.mk_projection(&[
                            ::rpl_mir::pat::PlaceElem::Deref,
                            ::rpl_mir::pat::PlaceElem::Field(
                                ::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("start"))
                            ),
                        ]))
                    )
                );
                let start_stmt = patterns.mk_assign(
                    start_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                        ::rpl_mir::pat::Place::new(
                            start_ref_local,
                            patterns.mk_projection(&[::rpl_mir::pat::PlaceElem::Deref,])
                        )
                    ))
                );
                let end_ref_stmt = patterns.mk_assign(
                    end_ref_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Ref(
                        ::rpl_mir::pat::RegionKind::ReAny,
                        ::rustc_middle::mir::BorrowKind::Shared,
                        ::rpl_mir::pat::Place::new(
                            iter_mut_local,
                            patterns.mk_projection(&[
                                ::rpl_mir::pat::PlaceElem::Deref,
                                ::rpl_mir::pat::PlaceElem::Field(
                                    ::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("end"))
                                ),
                            ])
                        )
                    )
                );
                let end_stmt = patterns.mk_assign(
                    end_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                        ::rpl_mir::pat::Place::new(
                            end_local,
                            patterns.mk_projection(&[::rpl_mir::pat::PlaceElem::Deref,])
                        )
                    ))
                );
                let cmp_stmt = patterns.mk_assign(
                    cmp_local.into_place(),
                    ::rpl_mir::pat::Rvalue::BinaryOp(
                        ::rustc_middle::mir::BinOp::Lt,
                        Box::new([
                            ::rpl_mir::pat::Operand::Move(start_local.into_place()),
                            ::rpl_mir::pat::Operand::Copy(end_local.into_place())
                        ])
                    )
                );
                patterns.mk_switch_int(::rpl_mir::pat::Operand::Move(cmp_local.into_place()), |mut patterns| {
                    patterns.mk_switch_target(false, |patterns| {
                        let opt_stmt = patterns.mk_assign(
                            opt_local.into_place(),
                            ::rpl_mir::pat::Rvalue::Aggregate(
                                ::rpl_mir::pat::AggKind::Adt(
                                    patterns.mk_path_with_args(patterns.mk_lang_item("None"), &[]),
                                    ::rpl_mir::pat::AggAdtKind::Unit
                                ),
                                Box::new([])
                            )
                        );
                    });
                    patterns.mk_otherwise(|patterns| {
                        let x1_stmt = patterns.mk_assign(
                            x1_local.into_place(),
                            ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                ::rpl_mir::pat::Place::new(
                                    iter_mut_local,patterns.mk_projection(&[
                                        ::rpl_mir::pat::PlaceElem::Deref,
                                        ::rpl_mir::pat::PlaceElem::Field(
                                            ::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("start"))
                                        ),
                                    ])
                                )
                            ))
                        );
                        let x2_stmt = patterns.mk_fn_call(
                            ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(patterns.mk_path_with_args(
                                patterns.mk_item_path(&["core", "iter", "range", "Step", "forward_unchecked",]),
                                &[]
                            ))),
                            patterns.mk_list([
                                ::rpl_mir::pat::Operand::Copy(x1_local.into_place()),
                                ::rpl_mir::pat::Operand::Constant(
                                    ::rpl_mir::pat::ConstOperand::ScalarInt(1_usize.into())
                                )
                            ]),
                            Some(x2_local.into_place())
                        );
                        let iter_mut_stmt = patterns.mk_assign(
                            ::rpl_mir::pat::Place::new(
                                iter_mut_local,
                                patterns.mk_projection(&[
                                    ::rpl_mir::pat::PlaceElem::Deref,
                                    ::rpl_mir::pat::PlaceElem::Field(
                                        ::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("start"))
                                    ),
                                ])
                            ),
                            ::rpl_mir::pat::Rvalue::Use(
                                ::rpl_mir::pat::Operand::Copy(x2_local.into_place())
                            )
                        );
                        let opt_stmt = patterns.mk_fn_call(
                            ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                                patterns.mk_path_with_args(patterns.mk_lang_item("Some"), &[])
                            )),
                            patterns.mk_list([
                                ::rpl_mir::pat::Operand::Copy(x1_local.into_place())
                            ]),
                            Some(opt_local.into_place())
                        );
                    });
                });
                let discr_stmt = patterns.mk_assign(
                    discr_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Discriminant(opt_local.into_place())
                );
                patterns.mk_switch_int(
                    ::rpl_mir::pat::Operand::Move(discr_local.into_place()),
                    |mut patterns| {
                        patterns.mk_switch_target(0_isize, |patterns| { patterns.mk_break(); });
                        patterns.mk_switch_target(1_isize, |patterns| {
                            let x_stmt = patterns.mk_assign(
                                x_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                    ::rpl_mir::pat::Place::new(
                                        opt_local,
                                        patterns.mk_projection(&[
                                            ::rpl_mir::pat::PlaceElem::Downcast(
                                                ::rustc_span::Symbol::intern("Some")
                                            ),
                                            ::rpl_mir::pat::PlaceElem::Field(
                                                ::rpl_mir::pat::Field::Unnamed(0u32.into())
                                            ),
                                        ])
                                    )
                                ))
                            );
                            let base_stmt = patterns.mk_assign(
                                base_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                    ::rpl_mir::pat::Place::new(
                                        self_local,
                                        patterns.mk_projection(&[
                                            ::rpl_mir::pat::PlaceElem::Deref,
                                            ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(
                                                ::rustc_span::Symbol::intern("mem")
                                            )),
                                        ])
                                    )
                                ))
                            );
                            let offset_stmt = patterns.mk_assign(
                                offset_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Cast(
                                    ::rustc_middle::mir::CastKind::IntToInt,
                                    ::rpl_mir::pat::Operand::Copy(x_local.into_place()),
                                    patterns.primitive_types.isize
                                )
                            );
                            let elem_ptr_stmt = patterns.mk_assign(
                                elem_ptr_local.into_place(),
                                ::rpl_mir::pat::Rvalue::BinaryOp(
                                    ::rustc_middle::mir::BinOp::Offset,
                                    Box::new([
                                        ::rpl_mir::pat::Operand::Copy(base_local.into_place()),
                                        ::rpl_mir::pat::Operand::Copy(offset_local.into_place())
                                    ])
                                )
                            );
                            patterns.mk_fn_call(
                                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                                    patterns.mk_path_with_args(
                                        patterns.mk_item_path(&["core", "ptr", "drop_in_place",]),
                                        &[]
                                    )
                                )),
                                patterns.mk_list([::rpl_mir::pat::Operand::Copy(
                                    elem_ptr_local.into_place()
                                )]),
                                None
                            );
                        });
                    }
                );
            });
        }
    );
}

#[test]
fn test_cve_2020_35892() {
    pass!(
        Mir! {
            meta!($T:ty, $SlabT:ty);

            let len: usize; // _2
            let mut x0: usize; // _17
            let x1: usize; // _14
            let x2: usize; // _15
            let x3: #[lang = "Option"]<usize>; // _3
            let x: usize; // _4
            let mut base: *mut T; // _6
            let offset: isize; // _7
            let elem_ptr: *mut T; // _5
            let x_cmp: usize; // _16
            let cmp: bool; // _13

            len = copy (*self).len;
            x0 = const 0_usize;
            loop {
                x_cmp = copy x0;
                cmp = Lt(move x_cmp, copy len);
                switchInt(move cmp) {
                    false => break,
                    _ => {
                        x1 = copy x0;
                        x2 = forward_unchecked(copy x1, const 1_usize);
                        x0 = move x2;
                        x3 = #[lang = "Some"](copy x1);
                        x = copy (x3 as Some).0;
                        base = copy (*self).mem;
                        offset = copy x as isize (IntToInt);
                        elem_ptr = Offset(copy base, copy offset);
                        _ = drop_in_place(copy elem_ptr);
                    }
                }
            }
        },
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);

            #[allow(non_snake_case)]
            let SlabT_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let SlabT_ty = patterns.mk_var_ty(SlabT_ty_var);

            let len_local = patterns.mk_local(patterns.primitive_types.usize);
            let x0_local = patterns.mk_local(patterns.primitive_types.usize);
            let x1_local = patterns.mk_local(patterns.primitive_types.usize);
            let x2_local = patterns.mk_local(patterns.primitive_types.usize);
            let x3_local = patterns.mk_local(patterns.mk_adt_ty(patterns.mk_path_with_args(
                patterns.mk_lang_item("Option"),
                &[patterns.primitive_types.usize.into()]
            )));
            let x_local = patterns.mk_local(patterns.primitive_types.usize);
            let base_local =
                patterns.mk_local(patterns.mk_raw_ptr_ty(T_ty, ::rustc_middle::mir::Mutability::Mut));
            let offset_local = patterns.mk_local(patterns.primitive_types.isize);
            let elem_ptr_local =
                patterns.mk_local(patterns.mk_raw_ptr_ty(T_ty, ::rustc_middle::mir::Mutability::Mut));
            let x_cmp_local = patterns.mk_local(patterns.primitive_types.usize);
            let cmp_local = patterns.mk_local(patterns.primitive_types.bool);
            let len_stmt = patterns.mk_assign(
                len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(::rpl_mir::pat::Place::new(
                    self_local,
                    patterns.mk_projection(&[
                        ::rpl_mir::pat::PlaceElem::Deref,
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(
                            ::rustc_span::Symbol::intern("len")
                        )),
                    ])
                )))
            );
            let x0_stmt = patterns.mk_assign(
                x0_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Constant(
                    ::rpl_mir::pat::ConstOperand::ScalarInt(0_usize.into())
                ))
            );
            patterns.mk_loop(|patterns| {
                let x_cmp_stmt = patterns.mk_assign(
                    x_cmp_local.into_place(),
                    ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(x0_local.into_place()))
                );
                let cmp_stmt = patterns.mk_assign(
                    cmp_local.into_place(),
                    ::rpl_mir::pat::Rvalue::BinaryOp(
                        ::rustc_middle::mir::BinOp::Lt,
                        Box::new([
                            ::rpl_mir::pat::Operand::Move(x_cmp_local.into_place()),
                            ::rpl_mir::pat::Operand::Copy(len_local.into_place())
                        ])
                    )
                );
                patterns.mk_switch_int(
                    ::rpl_mir::pat::Operand::Move(cmp_local.into_place()),
                    |mut patterns| {
                        patterns.mk_switch_target(false, |patterns| {
                            patterns.mk_break();
                        });
                        patterns.mk_otherwise(|patterns| {
                            let x1_stmt = patterns.mk_assign(
                                x1_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                    x0_local.into_place()
                                ))
                            );
                            let x2_stmt = patterns.mk_fn_call(
                                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                                    patterns.mk_path_with_args(
                                        patterns.mk_item_path(&["forward_unchecked",]),
                                        &[]
                                    )
                                )),
                                patterns.mk_list([
                                    ::rpl_mir::pat::Operand::Copy(x1_local.into_place()),
                                    ::rpl_mir::pat::Operand::Constant(
                                        ::rpl_mir::pat::ConstOperand::ScalarInt(1_usize.into())
                                    )
                                ]),
                                Some(x2_local.into_place())
                            );
                            let x0_stmt = patterns.mk_assign(
                                x0_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Move(
                                    x2_local.into_place()
                                ))
                            );
                            let x3_stmt = patterns.mk_fn_call(
                                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                                    patterns.mk_path_with_args(patterns.mk_lang_item("Some"), &[])
                                )),
                                patterns.mk_list([::rpl_mir::pat::Operand::Copy(
                                    x1_local.into_place()
                                )]),
                                Some(x3_local.into_place())
                            );
                            let x_stmt = patterns.mk_assign(
                                x_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                    ::rpl_mir::pat::Place::new(
                                        x3_local,
                                        patterns.mk_projection(&[
                                            ::rpl_mir::pat::PlaceElem::Downcast(
                                                ::rustc_span::Symbol::intern("Some")
                                            ),
                                            ::rpl_mir::pat::PlaceElem::Field(
                                                ::rpl_mir::pat::Field::Unnamed(0u32.into())
                                            ),
                                        ])
                                    )
                                ))
                            );
                            let base_stmt = patterns.mk_assign(
                                base_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                                    ::rpl_mir::pat::Place::new(
                                        self_local,
                                        patterns.mk_projection(&[
                                            ::rpl_mir::pat::PlaceElem::Deref,
                                            ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(
                                                ::rustc_span::Symbol::intern("mem")
                                            )),
                                        ])
                                    )
                                ))
                            );
                            let offset_stmt = patterns.mk_assign(
                                offset_local.into_place(),
                                ::rpl_mir::pat::Rvalue::Cast(
                                    ::rustc_middle::mir::CastKind::IntToInt,
                                    ::rpl_mir::pat::Operand::Copy(x_local.into_place()),
                                    patterns.primitive_types.isize
                                )
                            );
                            let elem_ptr_stmt = patterns.mk_assign(
                                elem_ptr_local.into_place(),
                                ::rpl_mir::pat::Rvalue::BinaryOp(
                                    ::rustc_middle::mir::BinOp::Offset,
                                    Box::new([
                                        ::rpl_mir::pat::Operand::Copy(base_local.into_place()),
                                        ::rpl_mir::pat::Operand::Copy(offset_local.into_place())
                                    ])
                                )
                            );
                            patterns.mk_fn_call(
                                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                                    patterns.mk_path_with_args(
                                        patterns.mk_item_path(&["drop_in_place",]),
                                        &[]
                                    )
                                )),
                                patterns.mk_list([::rpl_mir::pat::Operand::Copy(
                                    elem_ptr_local.into_place()
                                )]),
                                None
                            );
                        });
                    }
                );
            });
        },
    );
}

#[test]
fn test_cve_2018_21000() {
    pass!(
        Mir! {
            meta!($T1:ty, $T2:ty, $T3:ty);

            type VecT1 = std::vec::Vec<$T1>;
            type VecT2 = std::vec::Vec<$T2>;
            type VecT3 = std::vec::Vec<$T3>;
            type PtrT1 = *mut $T1;
            type PtrT3 = *mut $T3;

            let from_vec: VecT1 = _;
            let size: usize = SizeOf($T2);
            let from_cap: usize = Vec::capacity(move from_vec);
            let to_cap: usize = Mul(copy from_cap, copy size);
            let from_len: usize = Len(from_vec);
            let to_len: usize = Mul(copy from_len, copy size);
            let from_vec_ptr: PtrT1 = Vec::as_mut_ptr(move from_vec);
            let to_vec_ptr: PtrT3 = copy from_vec_ptr as PtrT3 (PtrToPtr);
            let _tmp: () = std::mem::forget(move from_vec);
            let res: VecT3 = Vec::from_raw_parts(copy to_vec_ptr, copy to_cap, copy to_len);
        },
        quote! {
            #[allow(non_snake_case)]
            let T1_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T1_ty = patterns.mk_var_ty(T1_ty_var);
            #[allow(non_snake_case)]
            let T2_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T2_ty = patterns.mk_var_ty(T2_ty_var);
            #[allow(non_snake_case)]
            let T3_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T3_ty = patterns.mk_var_ty(T3_ty_var);
            #[allow(non_snake_case)]
            let VecT1_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                &[T1_ty.into()]
            ));
            #[allow(non_snake_case)]
            let VecT2_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                &[T2_ty.into()]
            ));
            #[allow(non_snake_case)]
            let VecT3_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                &[T3_ty.into()]
            ));
            #[allow(non_snake_case)]
            let PtrT1_ty = patterns.mk_raw_ptr_ty(T1_ty, ::rustc_middle::mir::Mutability::Mut);
            #[allow(non_snake_case)]
            let PtrT3_ty = patterns.mk_raw_ptr_ty(T3_ty, ::rustc_middle::mir::Mutability::Mut);
            let from_vec_local = patterns.mk_local(VecT1_ty);
            let from_vec_stmt = patterns.mk_assign(from_vec_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let size_local = patterns.mk_local(patterns.primitive_types.usize);
            let size_stmt = patterns.mk_assign(
                size_local.into_place(),
                ::rpl_mir::pat::Rvalue::NullaryOp(::rustc_middle::mir::NullOp::SizeOf, T2_ty)
            );
            let from_cap_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_cap_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["Vec", "capacity",]),
                        &[]
                    )
                )),
                patterns.mk_list([::rpl_mir::pat::Operand::Move(from_vec_local.into_place())]),
                Some(from_cap_local.into_place())
            );
            let to_cap_local = patterns.mk_local(patterns.primitive_types.usize);
            let to_cap_stmt = patterns.mk_assign(
                to_cap_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Mul,
                    Box::new([
                        ::rpl_mir::pat::Operand::Copy(from_cap_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(size_local.into_place())
                    ])
                )
            );
            let from_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_len_stmt = patterns.mk_assign(
                from_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Len(from_vec_local.into_place())
            );
            let to_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let to_len_stmt = patterns.mk_assign(
                to_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Mul,
                    Box::new([
                        ::rpl_mir::pat::Operand::Copy(from_len_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(size_local.into_place())
                    ])
                )
            );
            let from_vec_ptr_local = patterns.mk_local(PtrT1_ty);
            let from_vec_ptr_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["Vec", "as_mut_ptr",]),
                        &[]
                    )
                )),
                patterns.mk_list([::rpl_mir::pat::Operand::Move(from_vec_local.into_place())]),
                Some(from_vec_ptr_local.into_place())
            );
            let to_vec_ptr_local = patterns.mk_local(PtrT3_ty);
            let to_vec_ptr_stmt = patterns.mk_assign(
                to_vec_ptr_local.into_place(),
                ::rpl_mir::pat::Rvalue::Cast(
                    ::rustc_middle::mir::CastKind::PtrToPtr,
                    ::rpl_mir::pat::Operand::Copy(from_vec_ptr_local.into_place()),
                    PtrT3_ty
                )
            );
            let _tmp_local = patterns.mk_local(patterns.mk_tuple_ty(&[]));
            let _tmp_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(patterns.mk_path_with_args(
                    patterns.mk_item_path(&["std", "mem", "forget",]),
                    &[]
                ))),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Move(from_vec_local.into_place())
                ]),
                Some(_tmp_local.into_place())
            );
            let res_local = patterns.mk_local(VecT3_ty);
            let res_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["Vec", "from_raw_parts",]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Copy(to_vec_ptr_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(to_cap_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(to_len_local.into_place())
                ]),
                Some(res_local.into_place())
            );
        }
    );
}

#[test]
fn test_cve_2020_35881_const() {
    pass!(
        Mir! {
            meta!{
                $T1:ty,
            };

            type PtrT1 = *const $T1;
            type PtrPtrT1 = *const *const $T1;
            type DerefPtrT1 = &*const $T1;
            type PtrT2 = *const ();
            type PtrPtrT2 = *const *const ();

            let ptr_to_data: PtrT1 = _;
            let data: DerefPtrT1 = &ptr_to_data;
            let ptr_to_ptr_to_data: PtrPtrT1 = &raw const (*data);
            let ptr_to_ptr_to_res: PtrPtrT2 = move ptr_to_ptr_to_data as *const *const () (Transmute);
            let ptr_to_res: PtrT2 = copy* ptr_to_ptr_to_res;
            // neglected the type-size-equivalence check
        },
        quote! {
            #[allow(non_snake_case)]
            let T1_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T1_ty = patterns.mk_var_ty(T1_ty_var);
            #[allow(non_snake_case)]
            let PtrT1_ty = patterns.mk_raw_ptr_ty(
                T1_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let PtrPtrT1_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_raw_ptr_ty(
                    T1_ty,
                    ::rustc_middle::mir::Mutability::Not
                ),
                ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let DerefPtrT1_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_raw_ptr_ty(
                    T1_ty,
                    ::rustc_middle::mir::Mutability::Not
                ),
                ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let PtrT2_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_tuple_ty(&[]),
                ::rustc_middle::mir::Mutability::Not
            );
            #[allow(non_snake_case)]
            let PtrPtrT2_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_raw_ptr_ty(
                    patterns.mk_tuple_ty(&[]),
                    ::rustc_middle::mir::Mutability::Not
                ),
                ::rustc_middle::mir::Mutability::Not
            );
            let ptr_to_data_local = patterns.mk_local(PtrT1_ty);
            let ptr_to_data_stmt = patterns.mk_assign(ptr_to_data_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let data_local = patterns.mk_local(DerefPtrT1_ty);
            let data_stmt = patterns.mk_assign(
                data_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Shared,
                    ptr_to_data_local.into_place()
                )
            );
            let ptr_to_ptr_to_data_local = patterns.mk_local(PtrPtrT1_ty);
            let ptr_to_ptr_to_data_stmt = patterns.mk_assign(
                ptr_to_ptr_to_data_local.into_place(),
                ::rpl_mir::pat::Rvalue::RawPtr(
                    ::rustc_middle::mir::Mutability::Not,
                    ::rpl_mir::pat::Place::new(
                        data_local,
                        patterns.mk_projection(
                            &[::rpl_mir::pat::PlaceElem::Deref,]
                        )
                    )
                )
            );
            let ptr_to_ptr_to_res_local = patterns.mk_local(PtrPtrT2_ty);
            let ptr_to_ptr_to_res_stmt = patterns.mk_assign(
                ptr_to_ptr_to_res_local.into_place(),
                ::rpl_mir::pat::Rvalue::Cast(
                    ::rustc_middle::mir::CastKind::Transmute,
                    ::rpl_mir::pat::Operand::Move(
                        ptr_to_ptr_to_data_local.into_place()
                    ),
                    patterns.mk_raw_ptr_ty(
                        patterns.mk_raw_ptr_ty(
                            patterns.mk_tuple_ty(&[]),
                            ::rustc_middle::mir::Mutability::Not
                        ),
                        ::rustc_middle::mir::Mutability::Not
                    )
                )
            );
            let ptr_to_res_local = patterns.mk_local(PtrT2_ty);
            let ptr_to_res_stmt = patterns.mk_assign(
                ptr_to_res_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(
                    ::rpl_mir::pat::Operand::Copy(
                        ::rpl_mir::pat::Place::new(
                            ptr_to_ptr_to_res_local,
                            patterns.mk_projection(
                                &[::rpl_mir::pat::PlaceElem::Deref,]
                            )
                        )
                    )
                )
            );
        }
    )
}

#[test]
fn test_cve_2020_35881_mut() {
    pass!(
        Mir! {
            meta!{
                $T1:ty,
            };

            type PtrT1 = *mut $T1;
            type PtrPtrT1 = *mut *mut $T1;
            type DerefPtrT1 = &mut *mut $T1;
            type PtrT2 = *mut ();
            type PtrPtrT2 = *mut *mut ();

            let ptr_to_data: PtrT1 = _;
            let data: DerefPtrT1 = &mut ptr_to_data;
            let ptr_to_ptr_to_data: PtrPtrT1 = &raw mut (*data);
            let ptr_to_ptr_to_res: PtrPtrT2 = move ptr_to_ptr_to_data as *mut *mut () (Transmute);
            let ptr_to_res: PtrT2 = copy *ptr_to_ptr_to_res;
        },
        quote! {
            #[allow(non_snake_case)]
            let T1_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T1_ty = patterns.mk_var_ty(T1_ty_var);
            #[allow(non_snake_case)]
            let PtrT1_ty = patterns.mk_raw_ptr_ty(
                T1_ty,
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let PtrPtrT1_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_raw_ptr_ty(
                    T1_ty,
                    ::rustc_middle::mir::Mutability::Mut
                ),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let DerefPtrT1_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_raw_ptr_ty(
                    T1_ty,
                    ::rustc_middle::mir::Mutability::Mut
                ),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let PtrT2_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_tuple_ty(&[]),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let PtrPtrT2_ty = patterns.mk_raw_ptr_ty(
                patterns.mk_raw_ptr_ty(
                    patterns.mk_tuple_ty(&[]),
                    ::rustc_middle::mir::Mutability::Mut
                ),
                ::rustc_middle::mir::Mutability::Mut
            );
            let ptr_to_data_local = patterns.mk_local(PtrT1_ty);
            let ptr_to_data_stmt = patterns.mk_assign(ptr_to_data_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let data_local = patterns.mk_local(DerefPtrT1_ty);
            let data_stmt = patterns.mk_assign(
                data_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Mut {
                        kind: ::rustc_middle::mir::MutBorrowKind::Default
                    },
                    ptr_to_data_local.into_place()
                )
            );
            let ptr_to_ptr_to_data_local = patterns.mk_local(PtrPtrT1_ty);
            let ptr_to_ptr_to_data_stmt = patterns.mk_assign(
                ptr_to_ptr_to_data_local.into_place(),
                ::rpl_mir::pat::Rvalue::RawPtr(
                    ::rustc_middle::mir::Mutability::Mut,
                    ::rpl_mir::pat::Place::new(
                        data_local,
                        patterns.mk_projection(
                            &[::rpl_mir::pat::PlaceElem::Deref,]
                        )
                    )
                )
            );
            let ptr_to_ptr_to_res_local = patterns.mk_local(PtrPtrT2_ty);
            let ptr_to_ptr_to_res_stmt = patterns.mk_assign(
                ptr_to_ptr_to_res_local.into_place(),
                ::rpl_mir::pat::Rvalue::Cast(
                    ::rustc_middle::mir::CastKind::Transmute,
                    ::rpl_mir::pat::Operand::Move(
                        ptr_to_ptr_to_data_local.into_place()
                    ),
                    patterns.mk_raw_ptr_ty(
                        patterns.mk_raw_ptr_ty(
                            patterns.mk_tuple_ty(&[]),
                            ::rustc_middle::mir::Mutability::Mut
                        ),
                        ::rustc_middle::mir::Mutability::Mut
                    )
                )
            );
            let ptr_to_res_local = patterns.mk_local(PtrT2_ty);
            let ptr_to_res_stmt = patterns.mk_assign(
                ptr_to_res_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(
                    ::rpl_mir::pat::Operand::Copy(
                        ::rpl_mir::pat::Place::new(
                            ptr_to_ptr_to_res_local,
                            patterns.mk_projection(
                                &[::rpl_mir::pat::PlaceElem::Deref,]
                            )
                        )
                    )
                )
            );
        }
    )
}

#[test]
fn test_cve_2021_29941_2() {
    pass!(
        Mir! {
            meta! {
                $T:ty,
            }

            // type ExactSizeIterT = impl std::iter::ExactSizeIterator<Item = $T>;
            // let's use a std::ops::Range<$T> instead temporarily
            type RangeT = std::ops::Range<$T>;
            type VecT = std::vec::Vec<$T>;
            type RefMutVecT = &mut std::vec::Vec<$T>;
            type PtrMutT = *mut $T;
            type RefMutSliceT = &mut [$T];
            type EnumerateRangeT = std::iter::Enumerate<RangeT>;
            type RefMutEnumerateRangeT = &mut std::iter::Enumerate<RangeT>;
            type OptionUsizeT = std::option::Option<(usize, $T)>;

            let iter: RangeT = _;
            // let len: usize = <RangeT as std::iter::ExactSizeIterator>::len(move iter);
            let len: usize = RangeT::len(move iter);
            let mut vec: VecT = std::vec::Vec::with_capacity(copy len);
            let mut ref_to_vec: RefMutVecT = &mut vec;
            let mut ptr_to_vec: PtrMutT = Vec::as_mut_ptr(move ref_to_vec);
            let mut slice: RefMutSliceT = std::slice::from_raw_parts_mut(copy ptr_to_vec, copy len);
            // let mut enumerate: EnumerateRangeT = <RangeT as std::iter::Iterator>::enumerate(move iter);
            let mut enumerate: EnumerateRangeT = RangeT::enumerate(move iter);
            let mut enumerate: RefMutEnumerateRangeT = &mut enumerate;
            let next: OptionUsizeT;
            let cmp: isize;
            let first: usize;
            let second_t: $T;
            let second_usize: usize;
            let _tmp: ();
            loop {
                // next = <EnumerateRangeT as std::iter::Iterator>::next(move enumerate);
                next = EnumerateRangeT::next(move enumerate);
                // in `cmp = discriminant(copy next);`
                // which discriminant should be used?
                cmp = balabala::discriminant(copy next);
                switchInt(move cmp) {
                    // true or 1 here?
                    true => {
                        first = copy (next as Some).0;
                        second_t = copy (next as Some).1;
                        second_usize = copy second_t as usize (IntToInt);
                        (*slice)[second_usize] = copy first as $T (IntToInt);
                    }
                    _ => break,
                }
            }
            // variable shadowing?
            // There cannnot be two mutable references to `vec` in the same scope
            ref_to_vec = &mut vec;
            _tmp = Vec::set_len(move ref_to_vec, copy len);
        },
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);
            #[allow(non_snake_case)]
            let RangeT_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "ops", "Range",]),
                &[T_ty.into()]
            ));
            #[allow(non_snake_case)]
            let VecT_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                &[T_ty.into()]
            ));
            #[allow(non_snake_case)]
            let RefMutVecT_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_path_ty(patterns.mk_path_with_args(
                    patterns.mk_item_path(&["std", "vec", "Vec",]),
                    &[T_ty.into()]
                )),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let PtrMutT_ty = patterns.mk_raw_ptr_ty(
                T_ty,::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let RefMutSliceT_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_slice_ty(T_ty),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let EnumerateRangeT_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "iter", "Enumerate",]),
                &[RangeT_ty.into()]
            ));
            #[allow(non_snake_case)]
            let RefMutEnumerateRangeT_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                patterns.mk_path_ty(patterns.mk_path_with_args(
                    patterns.mk_item_path(&["std", "iter", "Enumerate",]),
                    &[RangeT_ty.into()]
                )),
                ::rustc_middle::mir::Mutability::Mut
            );
            #[allow(non_snake_case)]
            let OptionUsizeT_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["std", "option", "Option",]),
                &[patterns.mk_tuple_ty(&[patterns.primitive_types.usize, T_ty]).into()]
            ));
            let iter_local = patterns.mk_local(RangeT_ty);
            let iter_stmt = patterns.mk_assign(iter_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let len_local = patterns.mk_local(patterns.primitive_types.usize);
            let len_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(
                    patterns.mk_zeroed(
                        patterns.mk_path_with_args(patterns.mk_item_path(&["RangeT", "len",]), &[])
                    )
                ),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Move(iter_local.into_place())
                ]),
                Some(len_local.into_place())
            );
            let vec_local = patterns.mk_local(VecT_ty);
            let vec_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["std", "vec", "Vec", "with_capacity",]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Copy(len_local.into_place())
                ]),
                Some(vec_local.into_place())
            );
            let ref_to_vec_local = patterns.mk_local(RefMutVecT_ty);
            let ref_to_vec_stmt = patterns.mk_assign(
                ref_to_vec_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Mut{
                        kind : ::rustc_middle::mir::MutBorrowKind::Default
                    },
                    vec_local.into_place()
                )
            );
            let ptr_to_vec_local = patterns.mk_local(PtrMutT_ty);
            let ptr_to_vec_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["Vec", "as_mut_ptr",]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    :: rpl_mir::pat::Operand::Move(ref_to_vec_local.into_place())
                ]),
                Some(ptr_to_vec_local.into_place())
            );
            let slice_local = patterns.mk_local(RefMutSliceT_ty);
            let slice_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["std", "slice", "from_raw_parts_mut",]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    :: rpl_mir::pat::Operand::Copy(ptr_to_vec_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(len_local.into_place())
                ]),
                Some(slice_local.into_place())
            );
            let enumerate_local = patterns.mk_local(EnumerateRangeT_ty);
            let enumerate_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["RangeT", "enumerate" ,]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Move(iter_local.into_place())
                ]),
                Some(enumerate_local.into_place())
            );
            let enumerate_local = patterns.mk_local(RefMutEnumerateRangeT_ty);
            let enumerate_stmt = patterns.mk_assign(
                enumerate_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Mut{
                        kind : ::rustc_middle::mir::MutBorrowKind::Default
                    },
                    enumerate_local.into_place()
                )
            );
            let next_local = patterns.mk_local(OptionUsizeT_ty);
            let cmp_local = patterns.mk_local(patterns.primitive_types.isize);
            let first_local = patterns.mk_local(patterns.primitive_types.usize);
            let second_t_local = patterns.mk_local(T_ty);
            let second_usize_local = patterns.mk_local(patterns.primitive_types.usize);
            let _tmp_local = patterns.mk_local(patterns.mk_tuple_ty(&[]));
            patterns.mk_loop(
                |patterns| {
                    let next_stmt = patterns.mk_fn_call(
                        ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                            patterns.mk_path_with_args(
                                patterns.mk_item_path(&["EnumerateRangeT", "next",]),
                                &[]
                            )
                        )),
                        patterns.mk_list([
                            :: rpl_mir::pat::Operand::Move(enumerate_local.into_place())
                        ]),
                        Some(next_local.into_place())
                    );
                    let cmp_stmt = patterns.mk_fn_call(
                        ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                            patterns.mk_path_with_args(
                                patterns.mk_item_path(&["balabala", "discriminant",]),
                                &[]
                            )
                        )),
                        patterns.mk_list([
                            ::rpl_mir::pat::Operand::Copy(next_local.into_place())
                        ]),
                        Some(cmp_local.into_place())
                    );
                    patterns.mk_switch_int(
                        ::rpl_mir::pat::Operand::Move(cmp_local.into_place()),
                        |mut patterns| {
                            patterns.mk_switch_target(
                                true, |patterns| {
                                    let first_stmt = patterns.mk_assign(
                                        first_local.into_place(),
                                        ::rpl_mir::pat::Rvalue::Use(
                                            ::rpl_mir::pat::Operand::Copy(
                                                ::rpl_mir::pat::Place::new(
                                                    next_local,
                                                    patterns.mk_projection(&[
                                                        ::rpl_mir::pat::PlaceElem::Downcast(
                                                            ::rustc_span::Symbol::intern("Some")
                                                        ),
                                                        ::rpl_mir::pat::PlaceElem::Field(
                                                            ::rpl_mir::pat::Field::Unnamed(0u32.into())
                                                        ),
                                                    ])
                                                )
                                            )
                                        )
                                    );
                                    let second_t_stmt = patterns.mk_assign(
                                        second_t_local.into_place(),
                                        ::rpl_mir::pat::Rvalue::Use(
                                            ::rpl_mir::pat::Operand::Copy(
                                                ::rpl_mir::pat::Place::new(
                                                    next_local,
                                                    patterns.mk_projection(&[
                                                        ::rpl_mir::pat::PlaceElem::Downcast(
                                                            ::rustc_span::Symbol::intern("Some")
                                                        ),
                                                        ::rpl_mir::pat::PlaceElem::Field(
                                                            ::rpl_mir::pat::Field::Unnamed(1u32.into())
                                                        ),
                                                    ])
                                                )
                                            )
                                        )
                                    );
                                    let second_usize_stmt = patterns.mk_assign(
                                        second_usize_local.into_place(),
                                        ::rpl_mir::pat::Rvalue::Cast(
                                            ::rustc_middle::mir::CastKind::IntToInt,
                                            ::rpl_mir::pat::Operand::Copy(
                                                second_t_local.into_place()
                                            ),
                                            patterns.primitive_types.usize
                                        )
                                    );
                                    let slice_stmt = patterns.mk_assign(
                                        ::rpl_mir::pat::Place::new(
                                            slice_local,
                                            patterns.mk_projection(&[
                                                ::rpl_mir::pat::PlaceElem::Deref,
                                                ::rpl_mir::pat::PlaceElem::Index(second_usize_local),
                                            ])
                                        ),
                                        ::rpl_mir::pat::Rvalue::Cast(
                                            ::rustc_middle::mir::CastKind::IntToInt,
                                            ::rpl_mir::pat::Operand::Copy(first_local.into_place()),
                                            T_ty
                                        )
                                    );
                                }
                            );
                            patterns.mk_otherwise(|patterns| { patterns.mk_break(); });
                        }
                    );
                }
            );
            let ref_to_vec_stmt = patterns.mk_assign(
                ref_to_vec_local.into_place(),
                ::rpl_mir::pat::Rvalue::Ref(
                    ::rpl_mir::pat::RegionKind::ReAny,
                    ::rustc_middle::mir::BorrowKind::Mut {
                        kind: ::rustc_middle::mir::MutBorrowKind::Default
                    },
                    vec_local.into_place()
                )
            );
            let _tmp_stmt = patterns.mk_fn_call(
                ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(
                    patterns.mk_path_with_args(
                        patterns.mk_item_path(&["Vec", "set_len",]),
                        &[]
                    )
                )),
                patterns.mk_list([
                    ::rpl_mir::pat::Operand::Move(ref_to_vec_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(len_local.into_place())
                ]),
                Some(_tmp_local.into_place())
            );
        }
    );
}

#[test]
fn test_cve_2018_21000_inlined() {
    pass!(
        Mir! {
            meta!{$T:ty}

            type Global = alloc::alloc::Global;

            let from_vec: alloc::vec::Vec<u8, Global> = _;
            let to_vec: alloc::vec::Vec<$T, Global>;
            let to_vec_cap: usize;
            let mut from_vec_cap: usize;
            let mut tsize: usize;
            let to_vec_len: usize;
            let mut from_vec_len: usize;
            let mut from_vec_ptr: core::ptr::non_null::NonNull<u8>;
            let mut to_raw_vec: alloc::raw_vec::RawVec<$T, Global>;
            let mut to_raw_vec_inner: alloc::raw_vec::RawVecInner<Global>;
            let mut to_vec_wrapped_len: alloc::raw_vec::Cap;
            let mut from_vec_unique_ptr: core::ptr::unique::Unique<u8>;

            from_vec_ptr = copy from_vec.buf.inner.ptr.pointer;
            from_vec_cap = copy from_vec.buf.inner.cap.0;
            tsize = SizeOf($T);
            to_vec_cap = Div(move from_vec_cap, copy tsize);
            from_vec_len = copy from_vec.len;
            to_vec_len = Div(move from_vec_len, copy tsize);
            to_vec_wrapped_len = #[ctor] alloc::raw_vec::Cap(copy to_vec_len);
            from_vec_unique_ptr = core::ptr::unique::Unique::<u8> {
                pointer: copy from_vec_ptr,
                _marker: const core::marker::PhantomData::<u8>,
            };
            to_raw_vec_inner = alloc::raw_vec::RawVecInner::<Global> {
                ptr: move from_vec_unique_ptr,
                cap: copy to_vec_wrapped_len,
                alloc: const alloc::alloc::Global,
            };
            to_raw_vec = alloc::raw_vec::RawVec::<$T, Global> {
                inner: move to_raw_vec_inner,
                _marker: const core::marker::PhantomData::<$T>,
            };
            to_vec = alloc::vec::Vec::<$T, Global> {
                buf: move to_raw_vec,
                len: copy to_vec_cap,
            };

        },
        quote! {
            #[allow(non_snake_case)]
            let T_ty_var = patterns.new_ty_var();
            #[allow(non_snake_case)]
            let T_ty = patterns.mk_var_ty(T_ty_var);
            #[allow(non_snake_case)]
            let Global_ty = patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["alloc", "alloc", "Global",]),
                &[]
            ));
            let from_vec_local = patterns.mk_local(patterns.mk_path_ty(
                patterns.mk_path_with_args(
                    patterns.mk_item_path(&["alloc", "vec", "Vec",]),
                    &[patterns.primitive_types.u8.into(), Global_ty.into()]
                )
            ));
            let from_vec_stmt = patterns.mk_assign(from_vec_local.into_place(), ::rpl_mir::pat::Rvalue::Any);
            let to_vec_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["alloc", "vec", "Vec",]),
                &[T_ty.into(), Global_ty.into()]
            )));
            let to_vec_cap_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_vec_cap_local = patterns.mk_local(patterns.primitive_types.usize);
            let tsize_local = patterns.mk_local(patterns.primitive_types.usize);
            let to_vec_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_vec_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_vec_ptr_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["core", "ptr", "non_null", "NonNull",]),
                &[patterns.primitive_types.u8.into()]
            )));
            let to_raw_vec_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["alloc", "raw_vec", "RawVec",]),
                &[T_ty.into(), Global_ty.into()]
            )));
            let to_raw_vec_inner_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["alloc", "raw_vec", "RawVecInner",]),
                &[Global_ty.into()]
            )));
            let to_vec_wrapped_len_local = patterns.mk_local(patterns.mk_path_ty(
                patterns.mk_path_with_args(patterns.mk_item_path(&["alloc", "raw_vec", "Cap",]), &[])
            ));
            let from_vec_unique_ptr_local = patterns.mk_local(patterns.mk_path_ty(patterns.mk_path_with_args(
                patterns.mk_item_path(&["core", "ptr", "unique", "Unique",]),
                &[patterns.primitive_types.u8.into()]
            )));
            let from_vec_ptr_stmt = patterns.mk_assign(
                from_vec_ptr_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                    ::rpl_mir::pat::Place::new(from_vec_local, patterns.mk_projection(&[
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("buf"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("inner"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("ptr"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("pointer"))),
                    ]))
                ))
            );
            let from_vec_cap_stmt = patterns.mk_assign(
                from_vec_cap_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                    ::rpl_mir::pat::Place::new(from_vec_local, patterns.mk_projection(&[
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("buf"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("inner"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("cap"))),
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Unnamed(0u32.into())),
                    ]))
                ))
            );
            let tsize_stmt = patterns.mk_assign(
                tsize_local.into_place(),
                ::rpl_mir::pat::Rvalue::NullaryOp(::rustc_middle::mir::NullOp::SizeOf, T_ty)
            );
            let to_vec_cap_stmt = patterns.mk_assign(
                to_vec_cap_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Div, Box::new([
                        ::rpl_mir::pat::Operand::Move(from_vec_cap_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(tsize_local.into_place())
                    ])
                )
            );
            let from_vec_len_stmt = patterns.mk_assign(
                from_vec_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Use(::rpl_mir::pat::Operand::Copy(
                    ::rpl_mir::pat::Place::new(from_vec_local, patterns.mk_projection(&[
                        ::rpl_mir::pat::PlaceElem::Field(::rpl_mir::pat::Field::Named(::rustc_span::Symbol::intern("len"))),
                    ]))
                ))
            );
            let to_vec_len_stmt = patterns.mk_assign(
                to_vec_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Div,
                    Box::new([
                        ::rpl_mir::pat::Operand::Move(from_vec_len_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(tsize_local.into_place())
                    ])
                )
            );
            let to_vec_wrapped_len_stmt = patterns.mk_assign(
                to_vec_wrapped_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["alloc", "raw_vec", "Cap",]),
                            &[]
                        ),
                        ::rpl_mir::pat::AggAdtKind::Tuple
                    ),
                    patterns.mk_list([::rpl_mir::pat::Operand::Copy(to_vec_len_local.into_place())]),
                )
            );
            let from_vec_unique_ptr_stmt = patterns.mk_assign(
                from_vec_unique_ptr_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["core", "ptr", "unique", "Unique",]),
                            &[patterns.primitive_types.u8.into()]
                        ),
                        patterns.mk_list([
                            ::rustc_span::Symbol::intern("pointer"),
                            ::rustc_span::Symbol::intern("_marker"),
                        ])
                        .into()
                    ),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Copy(from_vec_ptr_local.into_place()),
                        ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(patterns.mk_path_with_args(
                            patterns.mk_item_path(&["core", "marker", "PhantomData",]),
                            &[patterns.primitive_types.u8.into()]
                        ))),
                    ]),
                )
            );
            let to_raw_vec_inner_stmt = patterns.mk_assign(
                to_raw_vec_inner_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["alloc", "raw_vec", "RawVecInner",]),
                            &[Global_ty.into()]
                        ),
                        patterns.mk_list([
                            ::rustc_span::Symbol::intern("ptr"),
                            ::rustc_span::Symbol::intern("cap"),
                            ::rustc_span::Symbol::intern("alloc"),
                        ]).into()
                    ),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Move(from_vec_unique_ptr_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(to_vec_wrapped_len_local.into_place()),
                        ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(patterns.mk_path_with_args(
                            patterns.mk_item_path(&["alloc", "alloc", "Global",]),
                            &[]
                        ))),
                    ]),
                )
            );
            let to_raw_vec_stmt = patterns.mk_assign(
                to_raw_vec_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["alloc", "raw_vec", "RawVec",]),
                            &[T_ty.into(), Global_ty.into()]
                        ),
                        patterns.mk_list([
                            ::rustc_span::Symbol::intern("inner"),
                            ::rustc_span::Symbol::intern("_marker"),
                        ]).into()
                    ),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Move(to_raw_vec_inner_local.into_place()),
                        ::rpl_mir::pat::Operand::Constant(patterns.mk_zeroed(patterns.mk_path_with_args(
                            patterns.mk_item_path(&["core", "marker", "PhantomData",]),
                            &[T_ty.into()]
                        ))),
                    ]),
                )
            );
            let to_vec_stmt = patterns.mk_assign(
                to_vec_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::Adt(
                        patterns.mk_path_with_args(
                            patterns.mk_item_path(&["alloc", "vec", "Vec",]),
                            &[T_ty.into(), Global_ty.into()]
                        ),
                        patterns.mk_list([
                            ::rustc_span::Symbol::intern("buf"),
                            ::rustc_span::Symbol::intern("len"),
                        ]).into()
                    ),
                    patterns.mk_list([
                        ::rpl_mir::pat::Operand::Move(to_raw_vec_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(to_vec_cap_local.into_place()),
                    ]),
                )
            );
        }
    );
}
