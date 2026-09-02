//@check-pass: no pattern yet
//@compile-flags: -Z inline-mir=false
// Experimental: unified $poison→$sink; stderr captures current over-approx (see CVE note).

//! Minimal CVE-2021-30455 / id-map `Clone::clone_from` — unified poison→sink experiment.
//! `$poison` = drop_one, `$sink` = clone_t (may_panic via local generic).

#![allow(dead_code)]

use std::ptr;

#[inline(never)]
fn drop_one<T>(p: *mut T) {
    unsafe {
        ptr::drop_in_place(p);
    }
}

#[inline(never)]
fn clone_t<T: Clone>(x: &T) -> T {
    x.clone()
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

    fn drop_values(&mut self) {
        for (id, live) in self.occupied.iter().enumerate() {
            if *live {
                unsafe {
                    drop_one(self.values.as_mut_ptr().add(id));
                }
            }
        }
    }
}

impl<T: Clone> Clone for IdMap<T> {
    fn clone(&self) -> Self {
        let mut out = IdMap::new();
        out.clone_from(self);
        out
    }

    fn clone_from(&mut self, other: &Self) {
        self.drop_values();
        self.occupied = other.occupied.clone();
        let cap = other.values.capacity();
        self.values.reserve(cap);
        unsafe {
            for (id, live) in self.occupied.iter().enumerate() {
                if *live {
                    let cloned = clone_t(&*other.values.as_ptr().add(id));
                    ptr::write(self.values.as_mut_ptr().add(id), cloned);
                }
            }
        }
    }
}

impl<T> Drop for IdMap<T> {
    fn drop(&mut self) {
        self.drop_values();
    }
}

fn main() {
    let mut a = IdMap::new();
    a.insert(0, String::from("a"));
    let mut b = IdMap::new();
    b.insert(0, String::from("b"));
    b.clone_from(&a);
}
