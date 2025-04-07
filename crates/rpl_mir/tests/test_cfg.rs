#![allow(unused)]

#![feature(rustc_private)]
#![feature(macro_metavar_expr_concat)]

extern crate rustc_arena;
extern crate rustc_middle;
extern crate rustc_span;

use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use rpl_context::{PatCtxt, PatternCtxt};
use rpl_mir::pat::MirPattern;
use rustc_span::Symbol;

// macro_rules! test_case {
//     (fn $name:ident() {
//         meta!($($rpl_meta:tt)*);
//         $($input:tt)*
//     } => {$($expected:tt)*} $(,)?) => {
//         #[rpl_macros::pattern_def]
//         fn $name(pcx: PatCtxt<'_>) -> &MirPattern<'_> {
//             let pattern = rpl! {
//                 #[meta($($rpl_meta)*)]
//                 fn $pattern (..) -> _ = mir! {
//                     $($input)*
//                 }
//             };
//             pattern
//                 .fns
//                 .get_fn_pat(Symbol::intern("pattern"))
//                 .unwrap()
//                 .expect_mir_body()
//         }
//         #[test]
//         fn ${concat(test_, $name)}() {
//             PatternCtxt::entered_no_tcx(|pcx| {
//                 assert_eq($name(pcx), quote::quote!($($expected)*));
//             });
//         }
//     };
// }
// 
// #[track_caller]
// fn assert_eq(mir_pattern: &MirPattern<'_>, expected: TokenStream) {
//     assert_eq!(
//         format!("{mir_pattern:?}")
//             .parse::<TokenStream>()
//             .unwrap()
//             .to_string()
//             .replace(";", ";\n"),
//         expected.to_string().replace(';', ";\n"),
//     );
// }
// 
// test_case! {
//     fn cve_2020_25016() {
//         meta!($T:ty);
// 
//         let $from_slice: &[$T] = _;
//         let $from_slice_mut: &mut [$T] = _;
// 
//         let $from_raw: *const [$T] = &raw const *$from_slice;
//         let $from_raw_mut: *mut [$T] = &raw mut *$from_slice_mut;
// 
//         let $to_ptr: *const u8 = copy $from_raw as *const u8 (PtrToPtr);
//         let $to_ptr_mut: *mut u8 = copy $from_raw_mut as *mut u8 (PtrToPtr);
// 
//         let $from_len: usize = Len(*$from_slice);
//         let $ty_size: usize = SizeOf($T);
//         let $to_len: usize = Mul(move $from_len, move $ty_size);
// 
//         let $to_raw: *const [u8] = *const [u8] from (copy $to_ptr, copy $to_len);
//         let $to_raw_mut: *mut [u8] = *mut [u8] from (copy $to_ptr_mut, copy $to_len);
// 
//         let $to_slice: &[u8] = &*$to_raw;
//         let $to_slice_mut: &mut [u8] = &mut *$to_raw_mut;
//     } => {
//         let _?0: &[?T0];
//         let _?1: &mut [?T0];
//         let _?2: *const [?T0];
//         let _?3: *mut [?T0];
//         let _?4: *const u8;
//         let _?5: *mut u8;
//         let _?6: usize;
//         let _?7: usize;
//         let _?8: usize;
//         let _?9: *const [u8];
//         let _?10: *mut [u8];
//         let _?11: &[u8];
//         let _?12: &mut [u8];
//         ?bb0: {
//             _?0 = _;
//             _?1 = _;
//             _?2 = &raw const (*_?0);
//             _?3 = &raw mut (*_?1);
//             _?4 = copy _?2 as *const u8 (PtrToPtr);
//             _?5 = copy _?3 as *mut u8 (PtrToPtr);
//             _?6 = Len((*_?0));
//             _?7 = SizeOf(?T0);
//             _?8 = Mul(move _?6, move _?7);
//             _?9 = *const [u8] from (copy _?4, copy _?8);
//             _?10 = *mut [u8] from (copy _?5, copy _?8);
//             _?11 = &(*_?9);
//             _?12 = &mut (*_?10);
//             end;
//         }
//     }
// }
// 
// test_case! {
//     fn cve_2020_35892() {
//         meta!($T:ty, $SlabT:ty);
// 
//         let $self: &mut $SlabT;
//         let $len: usize; // _2
//         let mut $x0: usize; // _17
//         let $x1: usize; // _14
//         let $x2: usize; // _15
//         let $x3: #[lang = "Option"]<usize>; // _3
//         let $x: usize; // _4
//         let mut $base: *mut $T; // _6
//         let $offset: isize; // _7
//         let $elem_ptr: *mut $T; // _5
//         let $x_cmp: usize; // _16
//         let $cmp: bool; // _13
// 
//         $len = copy (*$self).len;
//         $x0 = const 0_usize;
//         loop {
//             $x_cmp = copy $x0;
//             $cmp = Lt(move $x_cmp, copy $len);
//             switchInt(move $cmp) {
//                 false => break,
//                 _ => {
//                     $x1 = copy $x0;
//                     $x2 = core::iter::Step::forward_unchecked(copy $x1, const 1_usize);
//                     $x0 = move $x2;
//                     $x3 = #[lang = "Some"](copy $x1);
//                     $x = copy ($x3 as Some).0;
//                     $base = copy (*$self).mem;
//                     $offset = copy $x as isize (IntToInt);
//                     $elem_ptr = Offset(copy $base, copy $offset);
//                     _ = core::ptr::drop_in_place(copy $elem_ptr);
//                 }
//             }
//         }
//     } => {
//         let _?0: &mut ?T1;
//         let _?1: usize;
//         let _?2: usize;
//         let _?3: usize;
//         let _?4: usize;
//         let _?5: #[lang = "Option"]<usize>;
//         let _?6: usize;
//         let _?7: *mut ?T0;
//         let _?8: isize;
//         let _?9: *mut ?T0;
//         let _?10: usize;
//         let _?11: bool;
//         ?bb0 : {
//             _?1 = copy ((*_?0).len);
//             _?2 = const 0_usize;
//             goto ?bb1;
//         }
//         ?bb1 : {
//             _?10 = copy _?2;
//             _?11 = Lt(move _?10, copy _?1);
//             switchInt(move _?11) -> [false: ?bb4, otherwise: ?bb5];
//         }
//         ?bb2: { end; }
//         ?bb3: { goto ?bb1; }
//         ?bb4: { goto ?bb2; }
//         ?bb5: {
//             _?3 = copy _?2;
//             _?4 = core::iter::Step::forward_unchecked(copy _?3, const 1_usize) -> ?bb6;
//         }
//         ?bb6: {
//             _?2 = move _?4;
//             _?5 = #[lang = "Some"](copy _?3);
//             _?6 = copy ((_?5 as Some).0);
//             _?7 = copy ((*_?0).mem);
//             _?8 = copy _?6 as isize (IntToInt);
//             _?9 = Offset(copy _?7, copy _?8);
//             core::ptr::drop_in_place(copy _?9) -> ?bb3;
//         }
//     }
// }
// 
// test_case! {
//     fn cve_2018_21000() {
//         meta!($T1:ty, $T2:ty, $T3:ty);
// 
//         type VecT1 = std::vec::Vec<$T1>;
//         type _VecT2 = std::vec::Vec<$T2>;
//         type VecT3 = std::vec::Vec<$T3>;
//         type PtrT1 = *mut $T1;
//         type PtrT3 = *mut $T3;
// 
//         let $from_vec: VecT1 = _;
//         let $size: usize = SizeOf($T2);
//         let $from_cap: usize = Vec::capacity(move $from_vec);
//         let $to_cap: usize = Mul(copy $from_cap, copy $size);
//         let $from_len: usize = Len($from_vec);
//         let $to_len: usize = Mul(copy $from_len, copy $size);
//         let $from_vec_ptr: PtrT1 = Vec::as_mut_ptr(move $from_vec);
//         let $to_vec_ptr: PtrT3 = copy $from_vec_ptr as PtrT3 (PtrToPtr);
//         let $_tmp: () = std::mem::forget(move $from_vec);
//         let $res: VecT3 = Vec::from_raw_parts(copy $to_vec_ptr, copy $to_cap, copy $to_len);
//     } => {
//         let _?0: std::vec::Vec<?T0>;
//         let _?1: usize;
//         let _?2: usize;
//         let _?3: usize;
//         let _?4: usize;
//         let _?5: usize;
//         let _?6: *mut ?T0;
//         let _?7: *mut ?T2;
//         let _?8: ();
//         let _?9: std::vec::Vec<?T2>;
//         ?bb0: {
//             _?0 = _;
//             _?1 = SizeOf(?T1);
//             _?2 = Vec::capacity(move _?0) -> ?bb1;
//         }
//         ?bb1: {
//             _?3 = Mul(copy _?2, copy _?1);
//             _?4 = Len(_?0);
//             _?5 = Mul(copy _?4, copy _?1);
//             _?6 = Vec::as_mut_ptr(move _?0) -> ?bb2;
//         }
//         ?bb2: {
//             _?7 = copy _?6 as *mut ?T2 (PtrToPtr);
//             _?8 = std::mem::forget(move _?0) -> ?bb3;
//         }
//         ?bb3: {
//             _?9 = Vec::from_raw_parts(copy _?7, copy _?3, copy _?5) -> ?bb4;
//         }
//         ?bb4: { end; }
//     }
// }
// 
// test_case! {
//     fn cve_2018_21000_inlined() {
//         meta!($T:ty);
// 
//         let $to_vec: alloc::vec::Vec<$T, alloc::alloc::Global>;
//         let $from_vec: alloc::vec::Vec<u8, alloc::alloc::Global> = _;
//         let $to_vec_cap: usize;
//         let mut $from_vec_cap: usize;
//         let mut $tsize: usize;
//         let $to_vec_len: usize;
//         let mut $from_vec_len: usize;
//         let mut $from_vec_ptr: core::ptr::non_null::NonNull<u8>;
//         let mut $to_raw_vec: alloc::raw_vec::RawVec<$T, alloc::alloc::Global>;
//         let mut $to_raw_vec_inner: alloc::raw_vec::RawVecInner<alloc::alloc::Global>;
//         let mut $to_vec_wrapped_len: alloc::raw_vec::Cap = _;
//         let mut $from_vec_unique_ptr: core::ptr::unique::Unique<u8>;
// 
//         $from_vec_ptr = copy $from_vec.buf.inner.ptr.pointer;
//         $from_vec_cap = copy $from_vec.buf.inner.cap.0;
//         $tsize = SizeOf($T);
//         $to_vec_cap = Div(move $from_vec_cap, copy $tsize);
//         $from_vec_len = copy $from_vec.len;
//         $to_vec_len = Div(move $from_vec_len, copy $tsize);
//         $to_vec_wrapped_len = #[ctor] alloc::raw_vec::Cap(copy $to_vec_len);
//         $from_vec_unique_ptr = core::ptr::unique::Unique::<u8> {
//             pointer: copy $from_vec_ptr,
//             _marker: const core::marker::PhantomData::<u8>,
//         };
//         $to_raw_vec_inner = alloc::raw_vec::RawVecInner::<alloc::alloc::Global> {
//             ptr: move $from_vec_unique_ptr,
//             cap: copy $to_vec_wrapped_len,
//             alloc: const alloc::alloc::Global,
//         };
//         $to_raw_vec = alloc::raw_vec::RawVec::<$T, alloc::alloc::Global> {
//             inner: move $to_raw_vec_inner,
//             _marker: const core::marker::PhantomData::<$T>,
//         };
//         $to_vec = alloc::vec::Vec::<$T, alloc::alloc::Global> {
//             buf: move $to_raw_vec,
//             len: copy $to_vec_cap,
//         };
//     } => {
//         let _?0: alloc::vec::Vec<?T0, alloc::alloc::Global>;
//         let _?1: alloc::vec::Vec<u8, alloc::alloc::Global>;
//         let _?2: usize;
//         let _?3: usize;
//         let _?4: usize;
//         let _?5: usize;
//         let _?6: usize;
//         let _?7: core::ptr::non_null::NonNull<u8>;
//         let _?8: alloc::raw_vec::RawVec<?T0, alloc::alloc::Global>;
//         let _?9: alloc::raw_vec::RawVecInner<alloc::alloc::Global>;
//         let _?10: alloc::raw_vec::Cap;
//         let _?11: core::ptr::unique::Unique<u8>;
//         ?bb0: {
//             _?1 = _;
//             _?10 = _;
//             _?7 = copy ((((_?1.buf).inner).ptr).pointer);
//             _?3 = copy ((((_?1.buf).inner).cap).0);
//             _?4 = SizeOf(?T0);
//             _?2 = Div(move _?3 , copy _?4);
//             _?6 = copy (_?1.len);
//             _?5 = Div(move _?6 , copy _?4);
//             _?10 = alloc::raw_vec::Cap(copy _?5);
//             _?11 = core::ptr::unique::Unique::<u8> {
//                 pointer: copy _?7,
//                 _marker: const core::marker::PhantomData::<u8>
//             };
//             _?9 = alloc::raw_vec::RawVecInner::<alloc::alloc::Global> {
//                 ptr: move _?11 ,
//                 cap: copy _?10,
//                 alloc: const alloc::alloc::Global
//             };
//             _?8 = alloc::raw_vec::RawVec::<?T0, alloc::alloc::Global> {
//                 inner: move _?9,
//                 _marker: const core::marker::PhantomData::<?T0>
//             };
//             _?0 = alloc::vec::Vec::<?T0, alloc::alloc::Global> {
//                 buf: move _?8,
//                 len: copy _?2
//              };
//             end;
//         }
//     }
// }
// 
// test_case! {
//     fn cve_2020_35881_const() {
//         meta!($T1:ty);
// 
//         type PtrT1 = *const $T1;
//         type PtrPtrT1 = *const *const $T1;
//         type DerefPtrT1 = &*const $T1;
//         type PtrT2 = *const ();
//         type PtrPtrT2 = *const *const ();
// 
//         let $ptr_to_data: PtrT1 = _;
//         let $data: DerefPtrT1 = &$ptr_to_data;
//         let $ptr_to_ptr_to_data: PtrPtrT1 = &raw const (*$data);
//         let $ptr_to_ptr_to_res: PtrPtrT2 = move $ptr_to_ptr_to_data as *const *const () (Transmute);
//         let $ptr_to_res: PtrT2 = copy* $ptr_to_ptr_to_res;
//         // neglected the type-size-equivalence check
//     } => {
//         let _?0: *const ?T0;
//         let _?1: &*const ?T0;
//         let _?2: *const *const ?T0;
//         let _?3: *const *const ();
//         let _?4: *const ();
//         ?bb0: {
//             _?0 = _;
//             _?1 = &_?0;
//             _?2 = &raw const (*_?1);
//             _?3 = move _?2 as *const *const () (Transmute);
//             _?4 = copy (*_?3);
//             end;
//         }
//     }
// }
// 
// test_case! {
//     fn cve_2020_35881_mut() {
//         meta!($T1:ty);
// 
//         type PtrT1 = *mut $T1;
//         type PtrPtrT1 = *mut *mut $T1;
//         type DerefPtrT1 = &mut *mut $T1;
//         type PtrT2 = *mut ();
//         type PtrPtrT2 = *mut *mut ();
// 
//         let $ptr_to_data: PtrT1 = _;
//         let $data: DerefPtrT1 = &mut $ptr_to_data;
//         let $ptr_to_ptr_to_data: PtrPtrT1 = &raw mut (*$data);
//         let $ptr_to_ptr_to_res: PtrPtrT2 = move $ptr_to_ptr_to_data as *mut *mut () (Transmute);
//         let $ptr_to_res: PtrT2 = copy *$ptr_to_ptr_to_res;
//     } => {
//         let _?0: *mut ?T0;
//         let _?1: &mut *mut ?T0; // the blank space here cannot pass the test
//         let _?2: *mut *mut ?T0;
//         let _?3: *mut *mut ();
//         let _?4: *mut ();
//         ?bb0: {
//             _?0 = _;
//             _?1 = &mut _?0;
//             _?2 = &raw mut (*_?1);
//             _?3 = move _?2 as *mut *mut () (Transmute);
//             _?4 = copy (*_?3);
//             end;
//         }
//     }
// }
// 
// test_case! {
//     fn cve_2021_29941_2() {
//         meta!($T:ty);
// 
//         // type ExactSizeIterT = impl std::iter::ExactSizeIterator<Item = $T>;
//         // let's use a std::ops::Range<$T> instead temporarily
//         type RangeT = std::ops::Range<$T>;
//         type VecT = std::vec::Vec<$T>;
//         type RefMutVecT = &mut std::vec::Vec<$T>;
//         type PtrMutT = *mut $T;
//         type RefMutSliceT = &mut [$T];
//         type EnumerateRangeT = std::iter::Enumerate<RangeT>;
//         type RefMutEnumerateRangeT = &mut std::iter::Enumerate<RangeT>;
//         type OptionUsizeT = std::option::Option<(usize, $T)>;
// 
//         let $iter: RangeT = _;
//         // let len: usize = <RangeT as std::iter::ExactSizeIterator>::len(move iter);
//         let $len: usize = RangeT::len(move $iter);
//         let mut $vec: VecT = std::vec::Vec::with_capacity(copy $len);
//         let mut $ref_to_vec: RefMutVecT = &mut $vec;
//         let mut $ptr_to_vec: PtrMutT = Vec::as_mut_ptr(move $ref_to_vec);
//         let mut $slice: RefMutSliceT = std::slice::from_raw_parts_mut(copy $ptr_to_vec, copy $len);
//         // let mut enumerate: EnumerateRangeT = <RangeT as std::iter::Iterator>::enumerate(move iter);
//         let mut $enumerate: EnumerateRangeT = RangeT::enumerate(move $iter);
//         let mut $enumerate: RefMutEnumerateRangeT = &mut $enumerate;
//         let $next: OptionUsizeT;
//         let $cmp: isize;
//         let $first: usize;
//         let $second_t: $T;
//         let $second_usize: usize;
//         let $_tmp: ();
//         loop {
//             // next = <EnumerateRangeT as std::iter::Iterator>::next(move enumerate);
//             $next = EnumerateRangeT::next(move $enumerate);
//             // in `cmp = discriminant(copy next);`
//             // which discriminant should be used?
//             $cmp = balabala::discriminant(copy $next);
//             switchInt(move $cmp) {
//                 // true or 1 here?
//                 true => {
//                     $first = copy ($next as Some).0;
//                     $second_t = copy ($next as Some).1;
//                     $second_usize = copy $second_t as usize (IntToInt);
//                     (*$slice)[$second_usize] = copy $first as $T (IntToInt);
//                 }
//                 _ => break,
//             }
//         }
//         // variable shadowing?
//         // There cannot be two mutable references to `vec` in the same scope
//         $ref_to_vec = &mut $vec;
//         $_tmp = Vec::set_len(move $ref_to_vec, copy $len);
//     } => {
//         let _?0: std::ops::Range<?T0>;
//         let _?1: usize;
//         let _?2: std::vec::Vec<?T0>;
//         let _?3: &mut std::vec::Vec<?T0>;
//         let _?4: *mut ?T0;
//         let _?5: &mut [?T0];
//         let _?6: std::iter::Enumerate<std::ops::Range<?T0> >;
//         let _?7: &mut std::iter::Enumerate<std::ops::Range<?T0> >;
//         let _?8: std::option::Option<(usize ,?T0 ,)>;
//         let _?9: isize;
//         let _?10: usize;
//         let _?11: ?T0;
//         let _?12: usize;
//         let _?13: ();
//         ?bb0: {
//             _?0 = _;
//             _?1 = RangeT::len (move _?0) -> ?bb1;
//         }
//         ?bb1: {
//             _?2 = std::vec::Vec::with_capacity (copy _?1) -> ?bb2;
//         }
//         ?bb2: {
//             _?3 = &mut _?2;
//             _?4 = Vec::as_mut_ptr (move _?3) -> ?bb3;
//         }
//         ?bb3: {
//             _?5 = std::slice::from_raw_parts_mut(
//                 copy _?4,
//                 copy _?1
//             ) -> ?bb4;
//         }
//         ?bb4: {
//             _?6 = RangeT::enumerate(move _?0) -> ?bb5;
//         }
//         ?bb5: {
//             _?7 = &mut _?7;
//             goto ?bb6;
//         }
//         ?bb6: {
//             _?8 = EnumerateRangeT::next (move _?7) -> ?bb8;
//         }
//         ?bb7: {
//             _?3 = &mut _?2;
//             _?13 = Vec::set_len (move _?3 , copy _?1) -> ?bb13;
//         }
//         ?bb8: {
//             _?9 = balabala::discriminant(copy _?8) -> ?bb9;
//         }
//         ?bb9: {
//             switchInt(move _?9) -> [true: ?bb11 , otherwise: ?bb12];
//         }
//         ?bb10: {
//             goto ?bb6;
//         }
//         ?bb11: {
//             _?10 = copy ((_?8 as Some).0);
//             _?11 = copy ((_?8 as Some).1);
//             _?12 = copy _?11 as usize (IntToInt);
//             ((* _?5) [_?12]) = copy _?10 as?T0 (IntToInt);
//             goto ?bb10;
//         }
//         ?bb12: {
//             goto ?bb7;
//         }
//         ?bb13: {
//             end;
//         }
//     }
// }
