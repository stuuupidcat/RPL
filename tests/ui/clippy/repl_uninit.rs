//@revisions: inline normal
//@[normal]compile-flags: -Z inline-mir=false
//@compile-flags: -A rpl::uninit_assumed_init
//@[inline] check-pass: `mem_replace_with_uninit` is undetectable after inlining (`mem::replace` inlines away); see FIXMEs below — category B
#![allow(deprecated, invalid_value)]

use std::mem;

fn might_panic<X>(x: X) -> X {
    // in practice this would be a possibly-panicky operation
    x
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn main() {
    let mut v = vec![0i32; 4];
    // the following is UB if `might_panic` panics
    unsafe {
        let taken_v = mem::replace(&mut v, mem::uninitialized());
        //~[normal]^ mem_replace_with_uninit

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }

    unsafe {
        let taken_v = mem::replace(&mut v, mem::MaybeUninit::uninit().assume_init());
        //~[normal]^ mem_replace_with_uninit
        // FIXME(nightly-2026-06): not detected in `inline` — `mem::replace` inlines to
        // a raw swap, so the matched call vanishes; and the inlined uninit's read-tail
        // is shared with `mem::uninitialized` (only a `write_bytes` fill differs), so a
        // reaching rule can't discriminate. See remaining-failures.md (category B).

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }

    unsafe {
        let taken_v = mem::replace(&mut v, mem::zeroed());
        //~[normal]^ mem_replace_with_uninit

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }

    // this is silly but OK, because usize is a primitive type
    let mut u: usize = 42;
    let uref = &mut u;
    let taken_u = unsafe { mem::replace(uref, mem::zeroed()) };
    *uref = taken_u + 1;

    // this is still not OK, because uninit
    let taken_u = unsafe { mem::replace(uref, mem::uninitialized()) };
    // FIXME: ~^ mem_replace_with_uninit

    *uref = taken_u + 1;
}
