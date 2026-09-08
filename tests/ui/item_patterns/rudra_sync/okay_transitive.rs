//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/okay_transitive.rs` fixture.
trait Foo: Sync {}

struct Atom0<P>(P);
unsafe impl<P: Eq + Foo> Sync for Atom0<P> {}

struct Atom1<P>(P);
unsafe impl<P: Eq> Send for Atom1<P> where P: Foo {}

fn main() {}
