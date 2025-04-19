//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[regular] compile-flags: -Z inline-mir=false

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn from_raw_parts_mut_spare_capacity() {
    let mut buf: Vec<u8> = Vec::new();
    let b = buf.len();

    let buf = unsafe {
        std::slice::from_raw_parts_mut(buf.as_mut_ptr().offset(b as isize), buf.capacity() - b)
        //~^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    };
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn from_raw_parts_mut() {
    let mut buf: Vec<u8> = Vec::new();
    let b = buf.len();

    let buf = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr(), b) };
}

fn deref_coerce() {
    let mut buf: Vec<u8> = Vec::new();

    let slice: &mut [u8] = &mut buf;
}

fn index_slice_range() {
    let mut buf: Vec<u8> = Vec::new();

    let slice = &mut buf[..];
}

fn index_slice_range_from_zero() {
    let mut buf: Vec<u8> = Vec::new();

    let slice = &mut buf[0..];
}

fn index_slice_range_from_len() {
    let mut buf: Vec<u8> = Vec::new();
    let b = buf.len();

    let slice = &mut buf[b..];
}

fn as_mut_slice() {
    let mut buf: Vec<u8> = Vec::new();

    let slice = buf.as_mut_slice();
}
