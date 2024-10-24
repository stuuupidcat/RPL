pub trait ComponentSlice<T> {
    /// The components interpreted as an array, e.g. one `RGB` expands to 3 elements.
    ///
    /// It's implemented for individual pixels as well as slices of pixels.
    fn as_slice(&self) -> &[T];
    /// The components interpreted as a mutable array, e.g. one `RGB` expands to 3 elements.
    ///
    /// It's implemented for individual pixels as well as slices of pixels.
    fn as_mut_slice(&mut self) -> &mut [T];
}

pub trait ComponentBytes<T: Copy + Send + Sync + 'static>
where
    Self: ComponentSlice<T>,
{
    /// The components interpreted as raw bytes, in machine's native endian. In `RGB` bytes of the red component are first.
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        let slice = self.as_slice();
        //~^ NOTE: trying to cast from this value of `&[T]` type
        unsafe {
            core::slice::from_raw_parts(
                //~^ ERROR: it is unsound to cast any slice `&[T]` to a byte slice `&[u8]`
                slice.as_ptr() as *const _,
                slice.len() * core::mem::size_of::<T>(),
            )
        }
    }
    /// The components interpreted as raw bytes, in machine's native endian. In `RGB` bytes of the red component are first.
    #[inline]
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let slice = self.as_mut_slice();
        //~^ NOTE: trying to cast from this value of `&mut [T]` type
        unsafe {
            core::slice::from_raw_parts_mut(
                //~^ ERROR: it is unsound to cast any slice `&mut [T]` to a byte slice `&mut [u8]`
                slice.as_mut_ptr() as *mut _,
                slice.len() * core::mem::size_of::<T>(),
            )
        }
    }
}
