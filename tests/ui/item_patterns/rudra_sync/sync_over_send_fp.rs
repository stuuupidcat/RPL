//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/sync_over_send_fp.rs` known-FP
// fixture. With no safe shared-reference API, neither resource relation holds.
struct Channel<P, Q>(P, Q);

unsafe impl<P: Sync, Q: Send> Sync for Channel<P, Q> {}

fn main() {}
