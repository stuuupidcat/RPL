use core::slice;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

pub trait BitStore: Copy {
    const INDX: u8;
    const MASK: u8;
    const BITS: u8;
    type Access;
}

pub trait BitOrder {}

#[derive(Clone, Copy)]
#[doc(hidden)]
pub(crate) union Pointer<T>
where
    T: BitStore,
{
    /// A shareable pointer to some contended mutable data.
    a: *const <T as BitStore>::Access,
    /// A read pointer to some data.
    r: *const T,
    /// A write pointer to some data.
    w: *mut T,
    /// The pointer address as a bare integer.
    u: usize,
}

impl<T> From<usize> for Pointer<T>
where
    T: BitStore,
{
    fn from(u: usize) -> Self {
        Self { u }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct BitPtr<T>
where
    T: BitStore,
{
    _ty: PhantomData<T>,
    ptr: NonNull<u8>,
    len: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct BitTail<T>
where
    T: BitStore,
{
    /// Semantic index *after* an element. Constrained to `0 ..= T::BITS`.
    end: u8,
    /// Marker for the tailed type.
    _ty: PhantomData<T>,
}

impl<T> BitTail<T>
where
    T: BitStore,
{
    /// Mark that `end` is a tail index for a type.
    ///
    /// # Parameters
    ///
    /// - `end` must be in the range `0 ..= T::BITS`.
    pub(crate) unsafe fn new_unchecked(end: u8) -> Self {
        debug_assert!(
            end <= T::BITS,
            "Bit tail {} cannot surpass type width {}",
            end,
            T::BITS,
        );
        Self {
            end,
            _ty: PhantomData,
        }
    }
    pub(crate) fn span(self, len: usize) -> (usize, Self) {
        let val = *self;
        debug_assert!(
            val <= T::BITS,
            "Tail out of range: {} overflows type width {}",
            val,
            T::BITS,
        );

        if len == 0 {
            return (0, self);
        }

        let head = val & T::MASK;

        let bits_in_head = (T::BITS - head) as usize;

        if len <= bits_in_head {
            let val = head + len as u8;
            return (0, unsafe { Self::new_unchecked(val) });
        }

        let bits_after_head = len - bits_in_head;

        let elts = bits_after_head >> T::INDX;
        let tail = bits_after_head as u8 & T::MASK;

        let is_zero = (tail == 0) as u8;
        let edges = 2 - is_zero as usize;
        let val = ((is_zero << T::INDX) | tail) as u8;
        return (elts + edges, unsafe { Self::new_unchecked(val) });

        /* The above expression is the branchless equivalent of this structure:

        if tail == 0 {
            (elts + 1, T::BITS.tail())
        }
        else {
            (elts + 2, tail.tail())
        }
        */
    }
}

impl<T: BitStore> Deref for BitTail<T> {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.end
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitIdx<T>
where
    T: BitStore,
{
    /// Semantic index within an element. Constrained to `0 .. T::BITS`.
    idx: u8,
    /// Marker for the indexed type.
    _ty: PhantomData<T>,
}

impl<T: BitStore> Deref for BitIdx<T> {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.idx
    }
}

impl<T> BitIdx<T>
where
    T: BitStore,
{
    #[inline]
    pub(crate) fn span(self, len: usize) -> (usize, BitTail<T>) {
        unsafe { BitTail::new_unchecked(*self) }.span(len)
    }
}

impl<T> BitPtr<T>
where
    T: BitStore,
{
    pub const PTR_DATA_MASK: usize = !Self::PTR_HEAD_MASK;
    pub const PTR_HEAD_BITS: usize = T::INDX as usize - Self::LEN_HEAD_BITS;
    pub const PTR_HEAD_MASK: usize = T::MASK as usize >> Self::LEN_HEAD_BITS;
    pub const LEN_HEAD_BITS: usize = 3;
    pub const LEN_HEAD_MASK: usize = 0b0111;
    pub const MAX_ELTS: usize = (Self::MAX_BITS >> 3) + 1;
    pub const MAX_BITS: usize = !0 >> Self::LEN_HEAD_BITS;

    pub(crate) fn pointer(&self) -> Pointer<T> {
        (self.ptr.as_ptr() as usize & Self::PTR_DATA_MASK).into()
    }

    #[inline]
    pub fn as_slice<'a>(&self) -> &'a [T] {
        unsafe { slice::from_raw_parts(self.pointer().r, self.elements()) }
    }

    /// Accesses the element slice behind the pointer as a Rust mutable slice.
    ///
    /// # Parameters
    ///
    /// - `&self`
    ///
    /// # Returns
    ///
    /// Standard Rust slice handle over the data governed by this pointer.
    ///
    /// # Lifetimes
    ///
    /// - `'a`: Lifetime for which the data behind the pointer is live.
    #[inline]
    pub fn as_mut_slice<'a>(&self) -> &'a mut [T] {
        unsafe { slice::from_raw_parts_mut(self.pointer().w, self.elements()) }
    }

    pub fn elements(&self) -> usize {
        self.head().span(self.len()).0
    }

    #[inline]
    pub fn head(&self) -> BitIdx<T> {
        let ptr = self.ptr.as_ptr() as usize;
        let ptr_head = (ptr & Self::PTR_HEAD_MASK) << Self::LEN_HEAD_BITS;
        let len_head = self.len & Self::LEN_HEAD_MASK;
        let idx = (ptr_head | len_head) as u8;
        BitIdx {
            idx,
            _ty: PhantomData,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len >> Self::LEN_HEAD_BITS
    }
}

pub struct BitBox<O, T>
where
    O: BitOrder,
    T: BitStore,
{
    _order: PhantomData<O>,
    pointer: BitPtr<T>,
}

impl<O, T> Drop for BitBox<O, T>
where
    O: BitOrder,
    T: BitStore,
{
    fn drop(&mut self) {
        let ptr = self.as_mut_slice().as_mut_ptr();
        let len = self.as_slice().len();
        //  Run the `Box<[T]>` destructor.
        drop(unsafe { Vec::from_raw_parts(ptr, 0, len) }.into_boxed_slice());
    }
}

impl<O, T> BitBox<O, T>
where
    O: BitOrder,
    T: BitStore,
{
    pub fn as_slice(&self) -> &[T] {
        self.bitptr().as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.bitptr().as_mut_slice()
    }

    /// Gives read access to the `BitPtr<T>` structure powering the box.
    ///
    /// # Parameters
    ///
    /// - `&self`
    ///
    /// # Returns
    ///
    /// A copy of the interior `BitPtr<T>`.
    pub(crate) fn bitptr(&self) -> BitPtr<T> {
        self.pointer
    }
}
