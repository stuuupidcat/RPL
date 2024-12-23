pub fn ensure_buffer_len(mut buffer: Vec<i32>, new_len: usize) -> Vec<i32> {
    if buffer.len() < new_len {
        if buffer.capacity() < new_len {
            buffer = Vec::with_capacity(new_len);
        }
        unsafe {
            buffer.set_len(new_len);
            //~^ ERROR: Use `Vec::set_len` to extend the length of a `Vec`, including uninitialized elements
        }
    } else {
        buffer.truncate(new_len);
    }
    buffer
}

/* pub fn safe_but_strange_ver(mut buffer: Vec<i32>, new_len: usize) -> Vec<i32> {
    if buffer.len() >= new_len {
    //Use `Vec::set_len` to truncate the length of a `Vec`
        unsafe {
            buffer.set_len(new_len);
        }
    }
    buffer
} */