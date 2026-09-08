//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/okay_imm.rs` fixture.
struct Atom<P>(P);

unsafe impl<P: Ord + Sync> Sync for Atom<P> {}

fn main() {}
