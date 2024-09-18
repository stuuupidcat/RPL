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
    let tcx = syn::parse_quote!(tcx);
    let patterns = syn::parse_quote!(patterns);
    let expanded = std::panic::catch_unwind(|| {
        let mut tokens = TokenStream::new();
        crate::expand_impl(&value, &tcx, &patterns, &mut tokens);
        tokens
    })
    .map_err(|err| err.downcast::<syn::Error>().unwrap())
    .unwrap();
    assert_eq!(expanded.to_string(), output.to_string());
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
            let T_ty_var = patterns.new_ty_var();
            let T_ty = patterns.mk_var_ty(tcx, T_ty_var);
        },
    );
}

#[test]
fn test_mir_pattern() {
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

            let from_slice: SliceT = any!();
            let from_raw_slice: PtrSliceT = &raw const *from_slice;
            let from_len: usize = Len(from_slice);
            let ty_size: usize = SizeOf($T);
            let to_ptr: PtrU8 = from_ptr as PtrU8 (PtrToPtr);
            let to_len: usize = Mul(from_len, ty_size);
            let to_raw_slice: PtrSliceU8 = *const SliceU8 from (to_ptr, t_len);
            let to_slice: RefSliceU8 = &*to_raw_slice;
        },
        quote! {
            let T_ty_var = patterns.new_ty_var();
            let T_ty = patterns.mk_var_ty(tcx, T_ty_var);
            let SliceT_ty = patterns.mk_slice_ty(tcx, T_ty);
            let RefSliceT_ty = patterns.mk_ref_ty(
                tcx,
                ::rpl_mir::pat::RegionKind::ReAny,
                SliceT_ty,
                ::rustc_middle::mir::Mutability::Not
            );
            let PtrSliceT_ty = patterns.mk_raw_ptr_ty(
                tcx, SliceT_ty, ::rustc_middle::mir::Mutability::Not);
            let PtrU8_ty = patterns.mk_raw_ptr_ty(
                tcx, patterns.primitive_types.u8, ::rustc_middle::mir::Mutability::Not);
            let SliceU8_ty = patterns.mk_slice_ty(tcx, patterns.primitive_types.u8);
            let PtrSliceU8_ty = patterns.mk_raw_ptr_ty(
                tcx, SliceU8_ty, ::rustc_middle::mir::Mutability::Not);
            let RefSliceU8_ty = patterns.mk_ref_ty(
                tcx,
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
                    patterns.mk_place(
                        from_slice_local,
                        (tcx, &[::rpl_mir::pat::PlaceElem::Deref,])
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
                    ::rpl_mir::pat::Copy(from_ptr_local.into_place()),
                    PtrU8_ty
                )
            );
            let to_len_local = patterns.mk_local(patterns.primitive_types.usize);
            let to_len_stmt = patterns.mk_assign(
                to_len_local.into_place(),
                ::rpl_mir::pat::Rvalue::BinaryOp(
                    ::rustc_middle::mir::BinOp::Mul,
                    Box::new([
                        ::rpl_mir::pat::Copy(from_len_local.into_place()),
                        ::rpl_mir::pat::Copy(ty_size_local.into_place())
                    ])
                )
            );
            let to_raw_slice_local = patterns.mk_local(PtrSliceU8_ty);
            let to_raw_slice_stmt = patterns.mk_assign(
                to_raw_slice_local.into_place(),
                ::rpl_mir::pat::Rvalue::Aggregate(
                    ::rpl_mir::pat::AggKind::RawPtr(SliceU8_ty, ::rustc_middle::mir::Mutability::Not),
                    [
                        ::rpl_mir::pat::Copy(to_ptr_local.into_place()),
                        ::rpl_mir::pat::Copy(t_len_local.into_place())
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
                    patterns.mk_place(to_raw_slice_local, (tcx, &[::rpl_mir::pat::PlaceElem::Deref,]))
                )
            );
        },
    );
}
