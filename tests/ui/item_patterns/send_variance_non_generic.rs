//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl
//@check-pass

struct Wrapper {
    value: std::rc::Rc<()>,
}

unsafe impl Send for Wrapper {}

fn main() {}
