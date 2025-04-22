//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[inline] check-pass
//@[regular] compile-flags: -Z inline-mir=false

macro_rules! cases {
    ($init:expr) => {
        // #[rpl::dump_mir(dump_cfg, dump_ddg)]
        fn from_raw_parts_mut_spare_capacity() {
            let mut buf: Vec<u8> = $init;
            let b = buf.len();

            let buf = unsafe {
                std::slice::from_raw_parts_mut(
                    //~[regular]^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
                    //~[regular]| HELP: See https://doc.rust-lang.org/std/slice/fn.from_raw_parts_mut.html#safety
                    //~[regular]| ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
                    //~[regular]| HELP: See https://doc.rust-lang.org/std/slice/fn.from_raw_parts_mut.html#safety
                    //~[regular]| ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
                    //~[regular]| HELP: See https://doc.rust-lang.org/std/slice/fn.from_raw_parts_mut.html#safety
                    //~[regular]| ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
                    //~[regular]| HELP: See https://doc.rust-lang.org/std/slice/fn.from_raw_parts_mut.html#safety
                    //~[regular]| NOTE: `#[deny(rpl::slice_from_raw_parts_uninitialized)]` on by default
                    buf.as_mut_ptr().offset(b as isize),
                    buf.capacity() - b,
                )
            };
        }

        // #[rpl::dump_mir(dump_cfg, dump_ddg)]
        fn from_raw_parts_mut() {
            let mut buf: Vec<u8> = $init;
            let b = buf.len();

            let buf = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr(), b) };
        }

        fn deref_coerce() {
            let mut buf: Vec<u8> = $init;

            let slice: &mut [u8] = &mut buf;
        }

        fn index_slice_range() {
            let mut buf: Vec<u8> = $init;

            let slice = &mut buf[..];
        }

        fn index_slice_range_from_zero() {
            let mut buf: Vec<u8> = $init;

            let slice = &mut buf[0..];
        }

        fn index_slice_range_from_len() {
            let mut buf: Vec<u8> = $init;
            let b = buf.len();

            let slice = &mut buf[b..];
        }

        fn as_mut_slice() {
            let mut buf: Vec<u8> = $init;

            let slice = buf.as_mut_slice();
        }
    };
}

mod new {
    cases!(Vec::new());
}

mod initialized {
    cases!(vec![1, 2, 3]);
}

mod with_capacity_0 {
    cases!(Vec::with_capacity(0));
}

mod with_capacity_1 {
    cases!(Vec::with_capacity(1));
}
