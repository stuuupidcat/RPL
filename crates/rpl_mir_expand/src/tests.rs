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
        test_pass::<$test_struct>(quote!($($tt)*), $($output)*);
    };
    ($test_struct:ident!{ $( $tt:tt )* }, $($output:tt)*) => {
        pass!($test_struct!( $($tt)* ), $($output)*);
    };
    ($test_struct:ident![ $( $tt:tt )* ], $($output:tt)*) => {
        pass!($test_struct!( $($tt)* ), $($output)*);
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
            let SliceT_ty = patterns.mk_slice_ty(T_ty);
            let RefSliceT_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                SliceT_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            let PtrSliceT_ty = patterns.mk_raw_ptr_ty(SliceT_ty, ::rustc_middle::mir::Mutability::Not);
            let PtrU8_ty = patterns.mk_raw_ptr_ty(
                patterns.primitive_types.u8, ::rustc_middle::mir::Mutability::Not
            );
            let SliceU8_ty = patterns.mk_slice_ty(patterns.primitive_types.u8);
            let PtrSliceU8_ty = patterns.mk_raw_ptr_ty(SliceU8_ty, ::rustc_middle::mir::Mutability::Not);
            let RefSliceU8_ty = patterns.mk_ref_ty(
                ::rpl_mir::pat::RegionKind::ReAny,
                SliceU8_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            let from_slice_local = patterns.mk_local(SliceT_ty);
            let from_slice_stmt = patterns.mk_init(from_slice_local);
            let from_raw_slice_local = patterns.mk_local(PtrSliceT_ty);
            let from_raw_slice_stmt = patterns.mk_assign(
                from_raw_slice_local.into_place(),
                ::rpl_mir::pat::Rvalue::AddressOf(
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
                    [
                        ::rpl_mir::pat::Operand::Copy(to_ptr_local.into_place()),
                        ::rpl_mir::pat::Operand::Copy(t_len_local.into_place())
                    ]
                    .into_iter()
                    .collect()
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
            let x3_local = patterns.mk_local(patterns.mk_adt_ty(
                patterns.mk_lang_item("Option"),
                patterns.mk_generic_args(&[patterns.primitive_types.usize.into()]),
            ));
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
                                patterns.mk_zeroed(
                                    patterns.mk_fn(
                                        patterns.mk_item_path(&["forward_unchecked",]),
                                        patterns.mk_generic_args(&[]),
                                    )
                                ),
                                ::rpl_mir::pat::List::ordered([
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
                                patterns.mk_zeroed(
                                    patterns.mk_fn(
                                        patterns.mk_lang_item("Some"),
                                        patterns.mk_generic_args(&[]),
                                    )
                                ),
                                ::rpl_mir::pat::List::ordered([::rpl_mir::pat::Operand::Copy(
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
                                patterns.mk_zeroed(
                                    patterns.mk_fn(
                                        patterns.mk_item_path(&["drop_in_place",]),
                                        patterns.mk_generic_args(&[]),
                                    )
                                ),
                                ::rpl_mir::pat::List::ordered([::rpl_mir::pat::Operand::Copy(
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
            meta!{
                $T1:ty,
                $T2:ty,
                $T3:ty,
            }
    
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
            // tuple: not implemented yet
            // let tmp: () = std::mem::forget(move from_vec);
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
            let VecT1_ty = patterns.mk_adt_ty(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                patterns.mk_generic_args(&[T1_ty.into()]),
            );
            let VecT2_ty = patterns.mk_adt_ty(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                patterns.mk_generic_args(&[T2_ty.into()]),
            );
            let VecT3_ty = patterns.mk_adt_ty(
                patterns.mk_item_path(&["std", "vec", "Vec",]),
                patterns.mk_generic_args(&[T3_ty.into()]),
            );
            let PtrT1_ty = patterns.mk_raw_ptr_ty(T1_ty, ::rustc_middle::mir::Mutability::Mut);
            let PtrT3_ty = patterns.mk_raw_ptr_ty(T3_ty, ::rustc_middle::mir::Mutability::Mut);
            let from_vec_local = patterns.mk_local(VecT1_ty);
            let from_vec_stmt = patterns.mk_init(from_vec_local);
            let size_local = patterns.mk_local(patterns.primitive_types.usize);
            let size_stmt = patterns.mk_assign(
                size_local.into_place(),
                ::rpl_mir::pat::Rvalue::NullaryOp(::rustc_middle::mir::NullOp::SizeOf, T2_ty)
            );
            let from_cap_local = patterns.mk_local(patterns.primitive_types.usize);
            let from_cap_stmt = patterns.mk_fn_call(
                patterns.mk_zeroed(
                    patterns.mk_fn(
                        patterns.mk_item_path(&["Vec", "capacity",]),
                        patterns.mk_generic_args(&[]),
                    )
                ),
                ::rpl_mir::pat::List::ordered([::rpl_mir::pat::Operand::Move(from_vec_local.into_place())]),
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
                patterns.mk_zeroed(
                    patterns.mk_fn(
                        patterns.mk_item_path(&["Vec", "as_mut_ptr",]),
                        patterns.mk_generic_args(&[]),
                    )
                ),
                ::rpl_mir::pat::List::ordered([::rpl_mir::pat::Operand::Move(from_vec_local.into_place())]),
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
            let res_local = patterns.mk_local(VecT3_ty);
            let res_stmt = patterns.mk_fn_call(
                patterns.mk_zeroed(
                    patterns.mk_fn(
                        patterns.mk_item_path(&["Vec", "from_raw_parts",]),
                        patterns.mk_generic_args(&[]),
                    )
                ),
                ::rpl_mir::pat::List::ordered([
                    ::rpl_mir::pat::Operand::Copy(to_vec_ptr_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(to_cap_local.into_place()),
                    ::rpl_mir::pat::Operand::Copy(to_len_local.into_place())
                ]),
                Some(res_local.into_place())
            );
        }
    );
}
