//@compile-flags: -Z mir-opt-level=0
fn main() {
    let vec = vec![b'a', b'b', b'c'];
    let ptr = vec.as_ptr();

    let offset_u8 = 1_u8;
    let offset_usize = 1_usize;
    let offset_isize = 1_isize;

    unsafe {
        let _ = ptr.offset(offset_usize as isize);
        //~^ ptr_offset_with_cast
        //~|ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        let _ = ptr.offset(offset_isize as isize);
        //~^ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        let _ = ptr.offset(offset_u8 as isize);
        //~^ERROR: it is an undefined behavior to offset a pointer using an unchecked integer

        let _ = ptr.wrapping_offset(offset_usize as isize);
        //~^ ptr_offset_with_cast
        let _ = ptr.wrapping_offset(offset_isize as isize);
        let _ = ptr.wrapping_offset(offset_u8 as isize);

        let _ = S.offset(offset_usize as isize);
        let _ = S.wrapping_offset(offset_usize as isize);

        let _ = (&ptr).offset(offset_usize as isize);
        //~^ ptr_offset_with_cast
        //~|ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        let _ = (&ptr).wrapping_offset(offset_usize as isize);
        //~^ ptr_offset_with_cast
    }
}

#[derive(Clone, Copy)]
struct S;

impl S {
    fn offset(self, _: isize) -> Self {
        self
    }
    fn wrapping_offset(self, _: isize) -> Self {
        self
    }
}
