//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/custom_marker_guard.rpl

struct Wrapper<T> {
    value: T,
}

mod fake {
    pub unsafe trait Marker {}
}

mod api {
    pub use crate::fake::Marker;
}

unsafe impl<T> fake::Marker for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
