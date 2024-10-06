#![feature(rustc_private)]
#![feature(macro_metavar_expr_concat)]

extern crate rustc_arena;
extern crate rustc_middle;
extern crate rustc_span;

use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use rpl_mir::pat::{Patterns, PatternsBuilder};
use rustc_arena::DroplessArena;

macro_rules! test_case {
    (fn $name:ident() {$($input:tt)*} => {$($expected:tt)*} $(,)?) => {
        #[rpl_macros::mir_pattern]
        fn $name(patterns: &mut PatternsBuilder<'_>) {
            mir! {
                $($input)*
            }
        }
        #[test]
        fn ${concat(test_, $name)}() {
            let arena = DroplessArena::default();
            rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
                let mut patterns = PatternsBuilder::new(&arena);
                $name(&mut patterns);
                assert_eq(patterns.build(), quote::quote!($($expected)*));
            });
        }
    };
}

#[track_caller]
fn assert_eq(patterns: Patterns<'_>, expected: TokenStream) {
    assert_eq!(
        format!("{patterns:?}")
            .parse::<TokenStream>()
            .unwrap()
            .to_string()
            .replace(";", ";\n"),
        expected.to_string().replace(';', ";\n"),
    );
}

test_case! {
    fn cve_2020_25016() {
        meta!($T:ty);

        let from_slice: &[$T] = _;
        let from_slice_mut: &mut [$T] = _;

        let from_raw: *const [$T] = &raw const *from_slice;
        let from_raw_mut: *mut [$T] = &raw mut *from_slice_mut;

        let to_ptr: *const u8 = copy from_raw as *const u8 (PtrToPtr);
        let to_ptr_mut: *mut u8 = copy from_raw_mut as *mut u8 (PtrToPtr);

        let from_len: usize = Len(*from_slice);
        let ty_size: usize = SizeOf($T);
        let to_len: usize = Mul(move from_len, move ty_size);

        let to_raw: *const [u8] = *const [u8] from (copy to_ptr, copy to_len);
        let to_raw_mut: *mut [u8] = *mut [u8] from (copy to_ptr_mut, copy to_len);

        let to_slice: &[u8] = &*to_raw;
        let to_slice_mut: &mut [u8] = &mut *to_raw_mut;
    } => {
        meta!(?T0:ty);
        let _?0: &[?T0];
        let _?1: &mut [?T0];
        let _?2: *const [?T0];
        let _?3: *mut [?T0];
        let _?4: *const u8;
        let _?5: *mut u8;
        let _?6: usize;
        let _?7: usize;
        let _?8: usize;
        let _?9: *const [u8];
        let _?10: *mut [u8];
        let _?11: &[u8];
        let _?12: &mut [u8];
        ?bb0: {
            _?0 = _;
            _?1 = _;
            _?2 = &raw const (*_?0);
            _?3 = &raw mut (*_?1);
            _?4 = copy _?2 as *const u8 (PtrToPtr);
            _?5 = copy _?3 as *mut u8 (PtrToPtr);
            _?6 = Len((*_?0));
            _?7 = SizeOf(?T0);
            _?8 = Mul(move _?6, move _?7);
            _?9 = RawPtr([u8], Not) from [copy _?4, copy _?8];
            _?10 = RawPtr([u8], Mut) from [copy _?5, copy _?8];
            _?11 = &(*_?9);
            _?12 = &mut (*_?10);
        }
    }
}

test_case! {
    fn cve_2020_35892() {
        meta!($T:ty, $SlabT:ty);

        let self: &mut $SlabT;
        let len: usize; // _2
        let mut x0: usize; // _17
        let x1: usize; // _14
        let x2: usize; // _15
        let x3: core::option::Option<usize>; // _3
        let x: usize; // _4
        let mut base: *mut $T; // _6
        let offset: isize; // _7
        let elem_ptr: *mut $T; // _5
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
                    x2 = core::iter::Step::forward_unchecked(copy x1, const 1_usize);
                    x0 = move x2;
                    x3 = core::option::Option::Some(copy x1);
                    x = copy (x3 as Some).0;
                    base = copy (*self).mem;
                    offset = copy x as isize (IntToInt);
                    elem_ptr = Offset(copy base, copy offset);
                    _ = core::ptr::drop_in_place(copy elem_ptr);
                }
            }
        }
    } => {
        meta!(?T0:ty, ?T1:ty);
        let _?0: &mut ?T1;
        let _?1: usize;
        let _?2: usize;
        let _?3: usize;
        let _?4: usize;
        let _?5: core::option::Option<usize>;
        let _?6: usize;
        let _?7: *mut ?T0;
        let _?8: isize;
        let _?9: *mut ?T0;
        let _?10: usize;
        let _?11: bool;
        ?bb0 : {
            _?1 = copy ((*_?0).len);
            _?2 = const 0_usize;
            goto ?bb1;
        }
        ?bb1 : {
            _?10 = copy _?2;
            _?11 = Lt(move _?10, copy _?1);
            switchInt(move _?11) -> [false -> ?bb4, otherwise -> ?bb5];
        }
        ?bb2: { }
        ?bb3: {
            goto ?bb1;
        }
        ?bb4: {
            goto ?bb2;
        }
        ?bb5: {
            _?3 = copy _?2;
            _?4 = core::iter::Step::forward_unchecked(copy _?3, const 1_usize) -> ?bb6;
        }
        ?bb6: {
            _?2 = move _?4;
            _?5 = core::option::Option::Some(copy _?3) -> ?bb7;
        }
        ?bb7: {
            _?6 = copy ((_?5 as Some).0);
            _?7 = copy ((*_?0).mem);
            _?8 = copy _?6 as isize (IntToInt);
            _?9 = Offset(copy _?7, copy _?8);
            core::ptr::drop_in_place(copy _?9) -> ?bb3;
        }
    }
}
