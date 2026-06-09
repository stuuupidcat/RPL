//@revisions: inline normal
//@[normal] compile-flags: -Zinline-mir=false
//@compile-flags: -A rpl::uninit_assumed_init
//@[inline] check-pass: `mem_replace_with_uninit` is undetectable after inlining (`mem::replace` inlines away); see FIXMEs below — category B
#![expect(clippy::uninit_assumed_init)] // Non-related
use std::mem;

fn might_panic<X>(x: X) -> X {
    // in practice this would be a possibly-panicky operation
    if false {
        panic!();
    }
    x
}

#[cfg_attr(test, test)]
pub(crate) fn base_case() {
    let mut v = vec![0i32; 4];
    // the following is UB if `might_panic` panics
    unsafe {
        #[expect(invalid_value)]
        let taken_v = mem::replace(&mut v, mem::MaybeUninit::uninit().assume_init());
        //~[normal]^ mem_replace_with_uninit
        // FIXME(nightly-2026-06): inline can't detect this — see the other arms and
        // docs/nightly-migration-remaining-failures.md (category B).

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }
}

#[cfg_attr(test, test)]
pub(crate) fn cross_function() {
    unsafe fn uninit<T>() -> T {
        let x = mem::MaybeUninit::<T>::uninit();
        unsafe { x.assume_init() }
    }
    let mut v = vec![0i32; 4];
    // the following is UB if `might_panic` panics
    unsafe {
        let taken_v = mem::replace(&mut v, uninit());
        // FIXME(nightly-2026-06): ~[inline]^ mem_replace_with_uninit — `mem::replace`
        // inlines away so there is no call to match. See remaining-failures.md (B).

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }
}

#[cfg_attr(test, test)]
pub(crate) fn cross_statement() {
    let mut v = vec![0i32; 4];
    // the following is UB if `might_panic` panics
    unsafe {
        let u = mem::MaybeUninit::uninit();
        let u = u.assume_init();
        let taken_v = mem::replace(&mut v, u);
        // FIXME(nightly-2026-06): ~[inline]^ mem_replace_with_uninit — `mem::replace`
        // inlines away so there is no call to match. See remaining-failures.md (B).

        let new_v = might_panic(taken_v);
        std::mem::forget(mem::replace(&mut v, new_v));
    }
}

pub(crate) fn main() {
    base_case();
    cross_function();
    cross_statement();
}
