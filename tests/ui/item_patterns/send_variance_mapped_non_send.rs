//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

struct Wrapper<P> {
    value: P,
}

unsafe impl<T> Send for Wrapper<Option<T>> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
