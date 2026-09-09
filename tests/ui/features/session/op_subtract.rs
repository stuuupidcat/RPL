//@rustc-env: RPL_PATS=tests/ui/features/session/op_subtract.rpl
//@check-pass
//@compile-flags: -Z inline-mir=false
//
// Expected: no diagnostic. Both util patterns match this function with the same `$T`;
// `wide - narrow` must subtract on SharedEnv + DefId, not NormalizedMatched equality.

#![allow(dead_code)]

fn both() -> i32 {
    let x = 1i32;
    let _z = 0i32;
    x
}

fn main() {}
