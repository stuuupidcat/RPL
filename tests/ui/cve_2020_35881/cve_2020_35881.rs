//@ ignore-on-host

use std::mem;

pub unsafe fn get_data<T: ?Sized>(val: *const T) -> *const () {
    *mem::transmute::<*const *const T, *const *const ()>(&val)
}

pub unsafe fn get_data_mut<T: ?Sized>(mut val: *mut T) -> *mut () {
    *mem::transmute::<*mut *mut T, *mut *mut ()>(&mut val)
}
