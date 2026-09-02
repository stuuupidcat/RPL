//@check-pass: no pattern yet
//@compile-flags: -Z inline-mir=false
// Experimental: unified $poison→$sink; stderr captures current over-approx (see CVE note).

//! Minimal CVE-2021-30456 / id-map `get_or_insert_with` — unified poison→sink experiment.
//! `$poison` = mark_occupied, `$sink` = call_f (may_panic).

#![allow(dead_code)]

use std::ptr;

#[inline(never)]
fn mark_occupied(slot: &mut bool) {
    *slot = true;
}

#[inline(never)]
fn call_f<T, F: FnOnce() -> T>(f: F) -> T {
    f()
}

struct IdMap<T> {
    occupied: Vec<bool>,
    values: Vec<T>,
}

impl<T> IdMap<T> {
    fn with_capacity(cap: usize) -> Self {
        Self {
            occupied: vec![false; cap],
            values: Vec::with_capacity(cap),
        }
    }

    fn get_or_insert_with<F>(&mut self, id: usize, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        if id >= self.occupied.len() {
            self.occupied.resize(id + 1, false);
        }
        if !self.occupied[id] {
            mark_occupied(&mut self.occupied[id]);
            if self.values.capacity() < id + 1 {
                self.values.reserve(id + 1);
            }
            unsafe {
                let space = self.values.as_mut_ptr().add(id);
                let val = call_f(f);
                ptr::write(space, val);
                &mut *space
            }
        } else {
            unsafe { &mut *self.values.as_mut_ptr().add(id) }
        }
    }

    /// TN: user code runs before occupancy is marked.
    fn get_or_insert_with_safe<F>(&mut self, id: usize, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        if id >= self.occupied.len() {
            self.occupied.resize(id + 1, false);
        }
        if !self.occupied[id] {
            if self.values.capacity() < id + 1 {
                self.values.reserve(id + 1);
            }
            unsafe {
                let space = self.values.as_mut_ptr().add(id);
                let val = call_f(f);
                ptr::write(space, val);
                mark_occupied(&mut self.occupied[id]);
                &mut *space
            }
        } else {
            unsafe { &mut *self.values.as_mut_ptr().add(id) }
        }
    }
}

impl<T> Drop for IdMap<T> {
    fn drop(&mut self) {
        for (id, live) in self.occupied.iter().enumerate() {
            if *live {
                unsafe {
                    ptr::drop_in_place(self.values.as_mut_ptr().add(id));
                }
            }
        }
    }
}

fn main() {
    let mut map = IdMap::<u8>::with_capacity(1);
    let _ = map.get_or_insert_with(0, || 42);
    let mut map2 = IdMap::<u8>::with_capacity(1);
    let _ = map2.get_or_insert_with_safe(0, || 7);
}
