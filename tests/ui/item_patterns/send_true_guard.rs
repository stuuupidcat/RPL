//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/true_guard.rpl

struct Wrapper<T> {
    value: T,
}

unsafe impl<T> Send for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
