//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

struct Wrapper<T, U> {
    safe: T,
    risky: U,
}

unsafe impl<T: Send, U> Send for Wrapper<T, U> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
