//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/no_generic.rs` fixture.
struct Atom(usize);

unsafe impl Sync for Atom {}
unsafe impl Send for Atom {}

fn main() {}
