//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl
//@check-pass

struct Wrapper<P> {
    value: P,
}

unsafe impl Send for Wrapper<u8> {}

fn main() {}
