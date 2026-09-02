//@check-pass: no pattern yet
//@compile-flags: -Z inline-mir=false
// Experimental: unified $poison→$sink; see CVE note (also fires unexpectedly — not a clean miss).

//! Minimal CVE-2021-30457 / id-map `remove_set` — unified poison→sink experiment.
//! `$poison` = drop_one, `$sink` = clear_occupied.
//! clear_occupied typically does *not* may_panic → expect miss (documents unified-shape limit).

#![allow(dead_code)]

use std::ptr;

#[inline(never)]
fn drop_one<T>(p: *mut T) {
    unsafe {
        ptr::drop_in_place(p);
    }
}

#[inline(never)]
fn clear_occupied(slot: &mut bool) {
    *slot = false;
}

struct IdMap<T> {
    occupied: Vec<bool>,
    values: Vec<T>,
}

impl<T> IdMap<T> {
    fn new() -> Self {
        Self {
            occupied: Vec::new(),
            values: Vec::new(),
        }
    }

    fn insert(&mut self, id: usize, val: T) {
        if id >= self.occupied.len() {
            self.occupied.resize(id + 1, false);
        }
        if self.values.capacity() < id + 1 {
            self.values.reserve(id + 1);
        }
        unsafe {
            ptr::write(self.values.as_mut_ptr().add(id), val);
        }
        self.occupied[id] = true;
    }

    fn remove_set(&mut self, to_remove: &[bool]) {
        let n = self.occupied.len().min(to_remove.len());
        for id in 0..n {
            if self.occupied[id] && to_remove[id] {
                unsafe {
                    drop_one(self.values.as_mut_ptr().add(id));
                }
                clear_occupied(&mut self.occupied[id]);
            }
        }
    }
}

impl<T> Drop for IdMap<T> {
    fn drop(&mut self) {
        for (id, live) in self.occupied.iter().enumerate() {
            if *live {
                unsafe {
                    drop_one(self.values.as_mut_ptr().add(id));
                }
            }
        }
    }
}

fn main() {
    let mut map = IdMap::new();
    map.insert(0, String::from("x"));
    map.remove_set(&[true]);
}
