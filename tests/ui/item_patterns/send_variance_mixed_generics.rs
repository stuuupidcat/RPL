//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

use std::marker::PhantomData;

struct Wrapper<'a, T, const N: usize> {
    value: T,
    lifetime: PhantomData<&'a ()>,
    bytes: [u8; N],
}

unsafe impl<'a, T, const N: usize> Send for Wrapper<'a, T, N> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
