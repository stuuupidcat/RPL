//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[inline] ignore-on-host
//@[regular] compile-flags: -Z inline-mir=false

#[derive(Default)]
pub struct Wrapper {
    buf: Vec<u8>,
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn main() {
    let mut wrapped: Wrapper = Wrapper::default();
    let b = wrapped.buf.len();
    let buf = unsafe {
        std::slice::from_raw_parts_mut(wrapped.buf.as_mut_ptr().offset(b as isize), wrapped.buf.capacity() - b)
        //~^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    };
}
