//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

struct Wrapper<T> {
    value: T,
}

unsafe impl<T> Send for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
