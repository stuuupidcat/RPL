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

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn noop_waker_ref() -> &'static Waker {
    //~^ ERROR: it is unsound to expose a `&'static std::task::Waker` from a thread-local where `std::task::Waker` is `Sync`
    thread_local! {
        static NOOP_WAKER_INSTANCE: UnsafeCell<Waker> =
            UnsafeCell::new(noop_waker());
    }
    NOOP_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}
