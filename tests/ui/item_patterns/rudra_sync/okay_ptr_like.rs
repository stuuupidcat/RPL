//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl

// Rudra accepts `P: Sync` as a pointer-like exception. These wrappers own `P`,
// so RPL requires the stronger `P: Send` bound.

struct Atom1<P>(P);
unsafe impl<P: Sync> Send for Atom1<P> {}
//~^ ERROR: A pattern instance found in this span

struct Atom2<P>(P);
unsafe impl<P> Send for Atom2<P> where P: Sync {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
