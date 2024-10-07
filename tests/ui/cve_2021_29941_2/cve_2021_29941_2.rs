pub fn swap_index(bla: impl ExactSizeIterator<Item = u32>) -> Vec<u32> {
    let len = bla.len();
    let mut vec = Vec::with_capacity(len);
    let arr: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(vec.as_mut_ptr(), bla.len()) };
    for (i, a) in bla.enumerate() {
        arr[a as usize] = i as u32;
    }

    unsafe {
        vec.set_len(len);
    }
    vec
}