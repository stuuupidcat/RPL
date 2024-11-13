#![feature(rustc_private)]
#![feature(macro_metavar_expr_concat)]

extern crate rustc_arena;
extern crate rustc_driver;
extern crate rustc_middle;
extern crate rustc_span;

use std::fs::File;
use std::io::{Read, Write};

use pretty_assertions::assert_eq;
use rpl_graphviz::{pat_cfg_to_graphviz, pat_ddg_to_graphviz};
use rpl_mir::pat::PatternsBuilder;
use rustc_arena::DroplessArena;

fn read_from_file(file: &str) -> std::io::Result<String> {
    let mut file = File::open(file)?;
    let mut expected = String::new();
    file.read_to_string(&mut expected)?;
    Ok(expected)
}

fn write_to_file(file: &str, content: &[u8]) -> std::io::Result<()> {
    File::create(file)?.write_all(content)?;
    Ok(())
}

macro_rules! test_case {
    ( $(#[$meta:meta])* fn $name:ident() { $($input:tt)* }) => {
        #[rpl_macros::mir_pattern]
        #[allow(unused_variables)]
        fn $name(patterns: &mut PatternsBuilder<'_>) {
            mir! {
                $($input)*
            }
        }
        #[test]
        $(#[$meta])*
        fn ${concat(test_, $name)}() {
            let arena = DroplessArena::default();
            rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
                let mut patterns = PatternsBuilder::new(&arena);
                $name(&mut patterns);
                let patterns = patterns.build();
                let mut cfg = Vec::new();
                pat_cfg_to_graphviz(&patterns, &mut cfg, &Default::default()).unwrap();
                let cfg = String::from_utf8(cfg).unwrap();
                let mut ddg = Vec::new();
                pat_ddg_to_graphviz(&patterns, &mut ddg, &Default::default()).unwrap();
                let ddg = String::from_utf8(ddg).unwrap();

                let cfg_file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/graphs/", stringify!($name), "_pat_cfg.dot");
                let cfg_expected = read_from_file(cfg_file).unwrap_or_default();
                let ddg_file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/graphs/", stringify!($name), "_pat_ddg.dot");
                let ddg_expected = read_from_file(ddg_file).unwrap_or_default();

                if cfg_expected != cfg {
                    let file = concat!(env!("CARGO_TARGET_TMPDIR"), "/", stringify!($name), "_pat_cfg.dot");
                    write_to_file(file, cfg.as_bytes()).unwrap();
                    assert_eq!(cfg, cfg_expected, "CFG mismatch, see {cfg_file} and {file}");
                }

                if ddg_expected != ddg {
                    let file = concat!(env!("CARGO_TARGET_TMPDIR"), "/", stringify!($name), "_pat_ddg.dot");
                    write_to_file(file, ddg.as_bytes()).unwrap();
                    assert_eq!(ddg, ddg_expected, "DDG mismatch, see {ddg_file} and {file}");
                }
            })
        }
    }
}

test_case! {
    fn cve_2020_35892_3() {
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
                    x2 = core::iter::range::Step::forward_unchecked(copy x1, const 1_usize);
                    // x0 = move x2;
                    x3 = #[lang = "Some"](copy x1);
                    x = copy (x3 as Some).0;
                    base = copy (*self).mem;
                    offset = copy x as isize (IntToInt);
                    elem_ptr = Offset(copy base, copy offset);
                    _ = core::ptr::drop_in_place(copy elem_ptr);
                }
            }
        }
    }
}

test_case! {
    fn cve_2018_21000() {
        meta!($T:ty);

        let from_slice_mut: &mut [$T] = _;
        let from_raw_mut: *mut [$T] = &raw mut *from_slice_mut;
        let from_len_mut: usize = PtrMetadata(copy from_slice_mut);
        let ty_size_mut: usize = SizeOf($T);
        let to_ptr_mut: *mut u8 = copy from_raw_mut as *mut u8 (PtrToPtr);
        let to_len_mut: usize = Mul(move from_len_mut, move ty_size_mut);
        let to_raw_mut: *mut [u8] = *mut [u8] from (copy to_ptr_mut, copy to_len_mut);
        let to_slice_mut: &mut [u8] = &mut *to_raw_mut;
    }
}

test_case! {
    fn cve_2021_29941_2() {
        meta!($T:ty);
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
    }

}

test_case! {
    fn unsafe_cell_alias() {
        meta!($T:ty);

        type unsafe_cell_t = core::cell::UnsafeCell<$T>;

        let a: &unsafe_cell_t = _;
        let b: &unsafe_cell_t = _;
        let raw_a: *const unsafe_cell_t = &raw const *a;
        let raw_b: *const unsafe_cell_t = &raw const *b;
        let raw_mut_a: *mut $T = copy raw_a as *mut $T (PtrToPtr);
        let raw_mut_b: *mut $T = copy raw_b as *mut $T (PtrToPtr);
        let mut_a: &mut $T = &mut *raw_mut_a;
        let mut_b: &mut $T = &mut *raw_mut_b;
        (*mut_a) = _;
        (*mut_b) = _;
    }
}

test_case! {
    fn control_flow() {
        let a: &mut i32 = _;
        let f: bool = _;
        switchInt(copy f) {
            false => {}
            _ => {
                *a = Add(copy (*a), const 1_i32);
            }
        }
    }
}
