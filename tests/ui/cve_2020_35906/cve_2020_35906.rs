//@ ignore-on-host

use core::task::{RawWaker, RawWakerVTable, Waker};
use std::sync::Arc;

pub fn waker<W>(wake: Arc<W>) -> core::task::Waker
where
    // bug fix:
    // W: ArcWake + 'static
    W: ArcWake,
{
    let ptr = Arc::into_raw(wake) as *const ();

    unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) }
}
pub trait ArcWake: Send + Sync {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }

    fn wake_by_ref(arc_self: &Arc<Self>);
}

pub fn waker_vtable<W: ArcWake>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

// FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
// code here. We should guard against this by aborting.

#[allow(clippy::redundant_clone)] // The clone here isn't actually redundant.
unsafe fn increase_refcount<T: ArcWake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = std::mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    // Now increase refcount, but don't drop new refcount either
    let _arc_clone: std::mem::ManuallyDrop<_> = arc.clone();
}

// used by `waker_ref`
unsafe fn clone_arc_raw<T: ArcWake>(data: *const ()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(arc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_arc_raw<T: ArcWake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = std::mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    ArcWake::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data as *const T))
}
