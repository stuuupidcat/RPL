//@rustc-env: RPL_PATS=tests/ui/features/session/shared_ty_hit.rpl
//@compile-flags: -Z inline-mir=false
//
// Contract: two MIR fns + `$Pair { $first: $T, $second: $U }` with only `$first` read.
// After FieldPat matching is fixed, unused `$second` must not create a second solution.

#![allow(dead_code)]

struct Pair<T, U> {
    first: T,
    second: U,
}

fn uses_pair_a<T: Copy, U>(p: &Pair<T, U>) -> T {
    p.first
    //~^ ERROR: session shared type variables matched
}

fn uses_pair_b<T: Copy, U>(p: &Pair<T, U>) -> T {
    p.first
    //~^ ERROR: session shared type variables matched
}

fn main() {}
