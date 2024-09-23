fn foo() {
    let pixel_count = 1920 * 1080;
    let mut ret: Vec<(u8, u8, u8)> = Vec::with_capacity(pixel_count);
    unsafe {
        ret.set_len(pixel_count);
    }
}
