use core::task::{RawWaker, RawWakerVTable, Waker};
use std::cell::UnsafeCell;

fn noop_waker() -> Waker {
    unsafe fn clone(_data: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)
    }

    unsafe fn wake(_data: *const ()) {}

    unsafe fn wake_by_ref(_data: *const ()) {}

    unsafe fn drop(_data: *const ()) {}

    static NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)) }
}

pub fn noop_waker_ref() -> &'static Waker {
    thread_local! {
        static NOOP_WAKER_INSTANCE: UnsafeCell<Waker> =
            UnsafeCell::new(noop_waker());
    }
    NOOP_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}
