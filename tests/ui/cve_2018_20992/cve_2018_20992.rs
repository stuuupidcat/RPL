fn ensure_buffer_len(mut buffer: Vec<i32>, new_len: usize) -> Vec<i32> {
    if buffer.len() < new_len {
        if buffer.capacity() < new_len {
            buffer = Vec::with_capacity(new_len);
        }
        unsafe { buffer.set_len(new_len); }
    } else {
        buffer.truncate(new_len);
    }
    buffer
}