//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

struct Atom<P>(P);
unsafe impl<P: Ord> Send for Atom<P> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
