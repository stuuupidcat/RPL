#![feature(macro_metavar_expr_concat)]
#![feature(rustc_private)]

extern crate rustc_arena;
extern crate rustc_index;
extern crate rustc_middle;
extern crate rustc_span;

use pretty_assertions::assert_eq;
use proc_macro2::TokenStream;
use quote::quote;
use rpl_mir::pat::PatternsBuilder;
use rpl_mir_graph::pat::PatDataDepGraph;
use rustc_arena::DroplessArena;
use std::fmt::Write;

macro_rules! test_case {
    (fn $name:ident() {$($input:tt)*} => { $($deps:tt)* }) => {
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
                let pattern = patterns.build();
                let graph = PatDataDepGraph::from_patterns(&pattern);
                let string = &mut String::new();
                for (bb, block) in graph.blocks() {
                    write!(string, "{bb:?}: {{").unwrap();
                    write!(string, "IN -> {:?},", block.rdep_start().collect::<Vec<_>>()).unwrap();
                    for stmt in 0..block.num_statements() {
                        write!(string, "{stmt} <- {:?},", block.deps(stmt).collect::<Vec<_>>()).unwrap();
                        if stmt < pattern.basic_blocks[bb].statements.len() {
                            write!(string, " | {:?};", pattern.basic_blocks[bb].statements[stmt]).unwrap();
                        } else if let Some(terminator) = &pattern.basic_blocks[bb].terminator {
                            write!(string, " | {:?};", terminator).unwrap();
                        }
                    }
                    write!(string, "OUT <- {:?}", block.dep_end().collect::<Vec<_>>()).unwrap();
                    write!(string, "}}").unwrap();
                }
                assert_eq!(
                    string
                        .parse::<TokenStream>()
                        .unwrap()
                        .to_string()
                        .replace(";", ";\n")
                        .replace("{", "{\n")
                        .replace("}", "}\n"),
                    quote!($($deps)*)
                        .to_string()
                        .replace(";", ";\n")
                        .replace("{", "{\n")
                        .replace("}", "}\n"),
                );
            });
        }
    };
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
        let from_mut_len: usize = Len(*from_slice_mut);

        let ty_size: usize = SizeOf($T);

        let to_len: usize = Mul(move from_len, move ty_size);
        let to_mut_len: usize = Mul(move from_mut_len, move ty_size);

        let to_raw: *const [u8] = *const [u8] from (copy to_ptr, copy to_len);
        let to_raw_mut: *mut [u8] = *mut [u8] from (copy to_ptr_mut, copy to_mut_len);

        let to_slice: &[u8] = &*to_raw;
        let to_slice_mut: &mut [u8] = &mut *to_raw_mut;
    } => {
        ?bb0: {
            IN -> [],
            0 <- [],       | _?0 = _;
            1 <- [],       | _?1 = _;
            2 <- [0],      | _?2 = &raw const (*_?0);
            3 <- [1],      | _?3 = &raw mut (*_?1);
            4 <- [2],      | _?4 = copy _?2 as *const u8 (PtrToPtr);
            5 <- [3],      | _?5 = copy _?3 as *mut u8 (PtrToPtr);
            6 <- [0],      | _?6 = Len((*_?0));
            7 <- [1],      | _?7 = Len((*_?1));
            8 <- [],       | _?8 = SizeOf(?T0);
            9 <- [6, 8],   | _?9 = Mul(move _?6, move _?8);
            10 <- [7, 8],  | _?10 = Mul(move _?7, move _?8);
            11 <- [4, 9],  | _?11 = *const [u8] from (copy _?4, copy _?9);
            12 <- [5, 10], | _?12 = *mut [u8] from (copy _?5, copy _?10);
            13 <- [11],    | _?13 = &(*_?11);
            14 <- [12],    | _?14 = &mut (*_?12);
            15 <- [],      | end;
            OUT <- [13, 14]
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
        let x3: #[lang = "Option"]<usize>; // _3
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
                    x3 = #[lang = "Some"](copy x1);
                    x = copy (x3 as Some).0;
                    base = copy (*self).mem;
                    offset = copy x as isize (IntToInt);
                    elem_ptr = Offset(copy base, copy offset);
                    _ = core::ptr::drop_in_place(copy elem_ptr);
                }
            }
        }
    } => {
        ?bb0: {
            IN -> [0],
            0 <- [], | _?1 = copy ((*_?0).len);
            1 <- [], | _?2 = const 0_usize;
            2 <- [], | goto ?bb1;
            OUT <- [0, 1]
        }
        ?bb1: {
            IN -> [0, 1],
            0 <- [],  | _?10 = copy _?2;
            1 <- [0], | _?11 = Lt(move _?10, copy _?1);
            2 <- [1], | switchInt(move _?11) -> [false -> ?bb4, otherwise -> ?bb5];
            OUT <- []
        }
        ?bb2: { IN -> [], 0 <- [], | end;       OUT <- [] }
        ?bb3: { IN -> [], 0 <- [], | goto ?bb1; OUT <- [] }
        ?bb4: { IN -> [], 0 <- [], | goto ?bb2; OUT <- [] }
        ?bb5: {
            IN -> [0],
            0 <- [],  | _?3 = copy _?2;
            1 <- [0], | _?4 = core::iter::Step::forward_unchecked(copy _?3, const 1_usize) -> ?bb6;
            OUT <- [1]
        }
        ?bb6: {
            IN -> [0, 1, 3],
            0 <- [],     | _?2 = move _?4;
            1 <- [],     | _?5 = #[lang = "Some"](copy _?3);
            2 <- [1],    | _?6 = copy ((_?5 as Some).0);
            3 <- [],     | _?7 = copy ((*_?0).mem);
            4 <- [2],    | _?8 = copy _?6 as isize (IntToInt);
            5 <- [3, 4], | _?9 = Offset(copy _?7, copy _?8);
            6 <- [5],    | core::ptr::drop_in_place(copy _?9) -> ?bb3;
            OUT <- [0]
        }
    }
}
