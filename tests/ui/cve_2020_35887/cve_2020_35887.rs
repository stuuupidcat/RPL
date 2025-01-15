//@ ignore-on-host

use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use std::ops::{Index, IndexMut, Range};

pub struct Array<T> {
    size: usize,
    ptr: *mut T,
}

impl<T> Array<T> {
    /// Convert to slice
    pub fn to_slice<'a>(&'a self) -> &'a [T] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const T, self.size) }
    }

    /// Convert to mutable slice
    pub fn to_slice_mut<'a>(&'a mut self) -> &'a mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.size) }
    }

    /// The length of the array (number of elements T)
    pub fn len(&self) -> usize {
        self.size
    }
}

impl<T> Array<T>
where
    T: Default + Copy,
{
    /// Easy initialization if all you want is your T's default instantiation
    pub fn new(size: usize) -> Self {
        let objsize = std::mem::size_of::<T>();
        let layout = Layout::from_size_align(size * objsize, 8).unwrap();
        let ptr = unsafe { alloc(layout) as *mut T };
        let default: T = Default::default();
        for i in 0..size {
            unsafe {
                (*(ptr.wrapping_offset(i as isize))) = default;
            }
        }
        Self { size, ptr }
    }
}

impl<T> Array<T>
where
    T: Clone,
{
    /// More generic initialization instantiating all elements as copies of some template
    pub fn new_from_template(size: usize, template: &T) -> Self {
        let objsize = std::mem::size_of::<T>();
        let layout = Layout::from_size_align(size * objsize, 8).unwrap();
        let ptr = unsafe { alloc(layout) as *mut T };
        for i in 0..size {
            unsafe {
                (*(ptr.wrapping_offset(i as isize))) = template.clone();
                //~^ ERROR: Possibly dropping an uninitialized value
                //FIXME: check if this is a false positive
            }
        }
        Self { size, ptr }
    }
}

impl<T> Index<usize> for Array<T> {
    type Output = T;

    // #[rpl::dump_mir(dump_cfg, dump_ddg)]
    fn index<'a>(&'a self, idx: usize) -> &'a Self::Output {
        unsafe { self.ptr.wrapping_offset(idx as isize).as_ref() }.unwrap()
    }
}

impl<T> IndexMut<usize> for Array<T> {
    fn index_mut<'a>(&'a mut self, idx: usize) -> &'a mut Self::Output {
        unsafe { self.ptr.wrapping_offset(idx as isize).as_mut() }.unwrap()
    }
}

impl<T> Index<Range<usize>> for Array<T> {
    type Output = [T];

    fn index<'a>(&'a self, idx: Range<usize>) -> &'a Self::Output {
        &self.to_slice()[idx]
    }
}

impl<T> IndexMut<Range<usize>> for Array<T> {
    fn index_mut<'a>(&'a mut self, idx: Range<usize>) -> &'a mut Self::Output {
        &mut self.to_slice_mut()[idx]
    }
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        let objsize = std::mem::size_of::<T>();
        let layout = Layout::from_size_align(self.size * objsize, 8).unwrap();
        unsafe {
            dealloc(self.ptr as *mut u8, layout);
        }
    }
}