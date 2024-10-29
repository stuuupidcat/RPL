extern crate libc;

use std::ops::Index;
use std::ptr;

pub struct Slab<T> {
    capacity: usize,
    len: usize,
    mem: *mut T,
}

impl<T> Drop for Slab<T> {
    fn drop(&mut self) {
        for x in 0..self.len {
            //~^ ERROR: MIR pattern matched
            unsafe {
                let elem_ptr = self.mem.offset(x as isize);
                ptr::drop_in_place(elem_ptr);
                std::hint::black_box(elem_ptr);
            }
        }
        unsafe { libc::free(self.mem as *mut _ as *mut libc::c_void) };
    }
}

impl<T> Index<usize> for Slab<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &(*(self.mem.offset(index as isize))) }
    }
}

impl<T> Slab<T> {
    #[inline]
    pub fn remove(&mut self, offset: usize) -> T {
        assert!(offset < self.len, "Offset out of bounds");

        let elem: T;
        let last_elem: T;
        let elem_ptr: *mut T;
        let last_elem_ptr: *mut T;

        unsafe {
            elem_ptr = self.mem.offset(offset as isize);
            last_elem_ptr = self.mem.offset(self.len as isize);

            elem = ptr::read(elem_ptr);
            last_elem = ptr::read(last_elem_ptr);

            ptr::write(elem_ptr, last_elem);
        }

        self.len -= 1;
        return elem;
    }
}
