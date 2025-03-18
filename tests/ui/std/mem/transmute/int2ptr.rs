//@ignore-on-host
use std::mem::transmute;

// https://doc.rust-lang.org/std/mem/fn.transmute.html#transmutation-between-pointers-and-integers
// Transmuting integers to pointers is a largely unspecified operation.
// It is likely not equivalent to an as cast.
// Doing non-zero-sized memory accesses with a pointer constructed this way
// is currently considered undefined behavior.
fn transmute_int_to_ptr() {
    let x: usize = 0;
    let ptr: *const usize = unsafe { transmute(x) };
    unsafe {
        println!("{}", *ptr);
    }
}

fn main() {}
