//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[regular] compile-flags: -Z inline-mir=false

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn main() {
    let mut buf: Vec<u8> = Vec::new();
    let b = buf.len();
    let buf = unsafe {
        std::slice::from_raw_parts_mut(buf.as_mut_ptr().offset(b as isize), buf.capacity() - b)
        //~^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    };
}
