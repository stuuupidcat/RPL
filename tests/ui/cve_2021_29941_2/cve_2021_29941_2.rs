//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[regular] compile-flags: -Z inline-mir=false

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn swap_index_for_enumerate_(bla: impl ExactSizeIterator<Item = u32>) -> Vec<u32> {
    let len = bla.len();
    let mut vec = Vec::with_capacity(len);
    let arr: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr(), bla.len()) };
    //~^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    for (i, a) in bla.enumerate() {
        arr[a as usize] = i as u32;
    }

    unsafe {
        vec.set_len(len);
        //~^ ERROR: it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
    }
    vec
}

pub fn swap_index_range_loop_next(bla: std::ops::Range<u32>) -> Vec<u32> {
    let len = bla.len();
    let mut vec = Vec::with_capacity(len);
    let arr: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr(), bla.len()) };
    //~^ ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    /* for (i, a) in bla.enumerate() {
        arr[a as usize] = i as u32;
    } */
    let mut iter = bla.enumerate();
    loop {
        match iter.next() {
            Some((i, a)) => {
                arr[a as usize] = i as u32;
            }
            None => break,
        }
    }

    unsafe {
        vec.set_len(len);
        //~[inline]^ ERROR: Use `Vec::set_len` to extend the length of a `Vec`, potentially including uninitialized elements
        //~| ERROR: it violates the precondition of `Vec::set_len` to extend a `Vec`'s length without initializing its content in advance
        //~| ERROR: it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
    }
    vec
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn swap_index_for_enumerate(bla: impl ExactSizeIterator<Item = u32>) -> Vec<u32> {
    let len = bla.len();
    let mut vec = Vec::with_capacity(len);
    let arr: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr(), len) };
    //~[inline]^ERROR: it violates the precondition of `std::slice::from_raw_parts_mut` to create a slice from uninitialized data
    for (i, a) in bla.enumerate() {
        arr[a as usize] = i as u32;
    }

    unsafe {
        vec.set_len(len);
        //~^ERROR: it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
        //~[inline]|ERROR: Use `Vec::set_len` to extend the length of a `Vec`, potentially including uninitialized elements
        //~|ERROR: it violates the precondition of `Vec::set_len` to extend a `Vec`'s length without initializing its content in advance
    }
    vec
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn swap_index(bla: impl ExactSizeIterator<Item = u32>) -> Vec<u32> {
    let mut vec = Vec::new();
    let len = bla.len();
    // vec.reserve(len);
    // let arr: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr(), bla.len()) };
    // for (i, a) in bla.enumerate() {
    //     arr[a as usize] = i as u32;
    // }

    unsafe {
        vec.set_len(len);
        //~^ERROR: it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
        //~[inline]|ERROR: Use `Vec::set_len` to extend the length of a `Vec`, potentially including uninitialized elements
    }
    vec
}

fn main() {}
