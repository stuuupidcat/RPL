//@ignore-on-host
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[repr(transparent)]
pub struct Atomic<T: ?Sized> {
    value: UnsafeCell<T>,
}

impl Atomic<u64> {
    pub fn fetch_add(&self, val: u64) -> u64 {
        let a = unsafe { &*(self.value.get() as *const AtomicU64) };
        a.fetch_add(val, Ordering::AcqRel)
    }
}
