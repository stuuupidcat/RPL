//@ ignore-on-host

use std::{ffi::CStr, ops::Deref};

#[allow(non_camel_case_types)]
type c_char = i8;

pub trait DisposeRef {
    /// What a reference to this type represents as a C pointer.
    type RefTo;
    /// Destroy the contents at the pointer's location.
    ///
    /// This should run some variant of `libc::free(ptr)`
    unsafe fn dispose(ptr: *mut Self::RefTo) {
        // free(ptr as *mut c_void);
    }
}

impl DisposeRef for str {
    type RefTo = c_char;
}

pub struct CBox<D: ?Sized>
where
    D: DisposeRef,
{
    pub ptr: *mut D::RefTo,
}

impl<D: ?Sized> CBox<D>
where
    D: DisposeRef,
{
    #[inline(always)]
    /// Wrap the pointer in a `CBox`.
    pub fn new(ptr: *mut D::RefTo) -> Self {
        CBox { ptr }
    }
    #[inline(always)]
    /// Returns the internal pointer.
    pub unsafe fn as_ptr(&self) -> *mut D::RefTo {
        self.ptr
    }
    #[inline(always)]
    /// Returns the internal pointer.
    pub unsafe fn unwrap(self) -> *mut D::RefTo {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }
}

impl<'a> Deref for CBox<str> {
    type Target = str;
    fn deref(&self) -> &str {
        unsafe {
            let text = CStr::from_ptr(self.ptr);
            std::str::from_utf8_unchecked(text.to_bytes())
        }
    }
}
