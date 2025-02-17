fn unchecked<T>(ptr: *const T, index: usize, length: usize) -> *const T {
    unsafe {
        let mut p = ptr;
        p = p.add(index);
        //~^ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        p
    }
}

fn unchecked_slice<T>(slice: &[T], index: usize) -> *const T {
    let mut p = slice.as_ptr();
    let length = slice.len();
    unsafe {
        p = p.add(index);
        //~^ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        &*p
    }
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn checked_lt<T>(slice: &[T], index: usize) -> &T {
    let mut p: *const T = slice.as_ptr();
    let length: usize = slice.len();
    assert!(index < length);
    unsafe {
        p = p.add(index);
        &*p
    }
}

fn checked_le<T>(ptr: *const T, index: usize, length: usize) -> *const T {
    unsafe {
        let mut p = ptr;
        // Though `index + 1` is moved in MIR, the negative pattern is still detected, so no false positive here
        assert!(index + 1 <= length);
        p = p.add(index);
        p
    }
}

fn checked_le_1<T>(ptr: *const T, index: usize, right: usize) -> *const T {
    unsafe {
        let mut p = ptr;
        assert!(index <= right);
        p = p.add(index);
        p
    }
}

fn safe_unchecked<T>(slice: &[T; 2]) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(1) }
    //~^ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
    //FIXME: this is a false positive
}

fn safe_unchecked_2_const<T, const N: usize>(slice: &[T; N], index: usize) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(index % N) }
}

fn safe_unchecked_2_const_literal_2<T>(slice: &[T; 2], index: usize) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(index % 2) }
}

fn safe_unchecked_2_const_literal_0<T>(slice: &[T; 0], index: usize) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(index % 0) }
    //~^ERROR: this operation will panic at runtime
}

fn safe_unchecked_2_mismatched<T>(slice: &[T], index: usize) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(index % 2) }
    //FIXME: this is a false negative
}

fn safe_unchecked_2_const_literal_2_3_mismatched<T>(slice: &[T; 2], index: usize) -> &T {
    let ptr = slice.as_ptr();
    unsafe { &*ptr.add(index % 3) }
    //FIXME: this is a false negative
}

fn safe_unchecked_2<T>(slice: &[T], index: usize) -> &T {
    let ptr = slice.as_ptr();
    let length = slice.len();
    unsafe { &*ptr.add(index % length) }
}

fn safe_unchecked_without_offset<T>(slice: &[T; 2]) -> &T {
    &slice[1]
}

unsafe fn unsafe_unchecked<T>(p: *const T) -> *const T {
    // Do anything you want with `p`, as it's in an `unsafe` function
    unsafe { p.add(1) }
}
