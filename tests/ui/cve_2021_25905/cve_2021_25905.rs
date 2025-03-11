//@ ignore-on-host
//@ revisions: inline regular
//@[inline] compile-flags: -Z inline-mir=true
//@[regular] compile-flags: -Z inline-mir=false

use std::io::{BufRead, Error as IoError, ErrorKind as IoErrorKind, Read, Result as IoResult};
use std::ops::Bound;
use std::ops::RangeBounds;

/// A buffered reader that greedily retains all memory read into a buffer.
///
/// Like [`std::io::BufReader`], it fetches bytes from the source in bulk to
/// reduce the number of actual reads. Moreover, it provides methods for
/// reading a byte or slice of bytes at an arbitrary position, reading as many
/// bytes as required to reach that position of the data stream, if they are
/// not in memory already. The position indices are always relative to the
/// position of the data source when it was passed to this construct via
/// [`new`] or [`with_capacity`].
///
/// [`std::io::BufReader`]: https://doc.rust-lang.org/std/io/struct.BufReader.html
/// [`new`]: ./struct.GreedyAccessReader.html#method.new
/// [`with_capacity`]: ./struct.GreedyAccessReader.html#method.with_capacity
pub struct GreedyAccessReader<R> {
    inner: R,
    buf: Vec<u8>,
    consumed: usize,
}

impl<R> GreedyAccessReader<R>
where
    R: Read,
{
    /// Creates a new greedy buffered reader with the given byte source.
    pub fn new(src: R) -> Self {
        GreedyAccessReader {
            inner: src,
            buf: Vec::new(),
            consumed: 0,
        }
    }

    /// Creates a new greedy buffered reader with the given byte source and
    /// the specified buffer capacity.
    ///
    /// The buffer will be able to read approximately `capacity` bytes without
    /// reallocating.
    pub fn with_capacity(src: R, capacity: usize) -> Self {
        GreedyAccessReader {
            inner: src,
            buf: Vec::with_capacity(capacity),
            consumed: 0,
        }
    }

    /// Retrieves the internal reader, discarding the buffer in the process.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Retrieves the internal buffer in its current state, discarding the
    /// reader in the process.
    pub fn into_buffer(self) -> Vec<u8> {
        self.buf
    }

    /// Retrieves the internal reader and buffer in their current state.
    pub fn into_parts(self) -> (R, Vec<u8>) {
        (self.inner, self.buf)
    }

    /// Fetches a single byte from the buffered data source.
    pub fn get(&mut self, index: usize) -> IoResult<u8> {
        if let Some(v) = self.buf.get(index) {
            Ok(*v)
        } else {
            self.prefetch_up_to(index + 1)?;

            self.buf
                .get(index)
                .cloned()
                .ok_or_else(|| IoError::new(IoErrorKind::Other, "Index out of bounds"))
        }
    }

    /// Obtains a slice of bytes.
    ///
    /// The range's end must be bound (e.g. `5..` is not supported).
    ///
    /// # Error
    ///
    /// Returns an I/O error if the range is out of the boundaries
    ///
    /// # Panics
    ///
    /// Panics if the range is not end bounded.
    pub fn slice<T>(&mut self, range: T) -> IoResult<&[u8]>
    where
        T: Clone,
        T: RangeBounds<usize>,
    {
        let end = range.end_bound();
        let e = match end {
            Bound::Unbounded => {
                unimplemented!("Unbounded end is currently not supported");
            }
            Bound::Excluded(&e) => e,
            Bound::Included(&e) => e + 1,
        };

        let b = match range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Excluded(&b) | Bound::Included(&b) => b,
        };

        self.prefetch_up_to(e)?;

        if b > e || e > self.buf.len() {
            Err(IoError::new(IoErrorKind::Other, "Index out of bounds"))
        } else {
            Ok(&self.buf[b..e])
        }
    }

    /// Clears all memory of past reads, shrinking or freeing the buffer in the
    /// process. The reader will behave as if freshly constructed, save for
    /// already prefetched data, so that no bytes are lost. The following byte
    /// being read becomes the byte at index `#0`.
    pub fn clear(&mut self) {
        if self.consumed < self.buf.len() {
            self.buf = self.buf[self.consumed..].to_vec();
        } else {
            self.buf = Vec::new();
        }
        self.consumed = 0;
    }

    /// Shrinks the internal buffer to minimal capacity.
    pub fn shrink_to_fit(&mut self) {
        self.buf.shrink_to_fit()
    }

    fn reserve_up_to(&mut self, index: usize) {
        let mut new_size = 16;
        while new_size < index || new_size < self.buf.capacity() {
            new_size *= 2;
        }
        let additional = new_size - self.buf.capacity();
        if additional > 0 {
            self.buf.reserve(additional);
        }
    }

    fn data_to_read(&self) -> &[u8] {
        &self.buf[self.consumed..]
        //~[inline]^ ERROR: it is an undefined behavior to offset a pointer using an unchecked integer
        // Seems to be a false positive, as the offset is checked in the `read` method
    }

    fn prefetch_up_to(&mut self, i: usize) -> IoResult<()> {
        self.reserve_up_to(i);
        let mut l = 0;
        while self.buf.len() <= i {
            let b = self.fill_buf()?;
            if b.len() == l {
                // no extra data since last call, retreat
                break;
            } else {
                // record length, continue fetching
                l = b.len();
            }
        }
        Ok(())
    }
}

impl<R> Read for GreedyAccessReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // we'll be reading from the buffer
        let mut to_read = self.data_to_read();
        if to_read.is_empty() {
            self.fill_buf()?;
            to_read = self.data_to_read();
        }

        let len = usize::min(to_read.len(), buf.len());
        &mut buf[..len].copy_from_slice(&self.buf[self.consumed..self.consumed + len]);
        self.consume(len);
        Ok(len)
    }
}

impl<R> BufRead for GreedyAccessReader<R>
where
    R: Read,
{
    // #[rpl::dump_mir(dump_cfg, dump_ddg)]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        if self.buf.capacity() == self.consumed {
            self.reserve_up_to(self.buf.capacity() + 16);
        }

        let b = self.buf.len();
        let buf = unsafe {
            // safe because it's within the buffer's limits
            // and we won't be reading uninitialized memory
            std::slice::from_raw_parts_mut(
                self.buf.as_mut_ptr().offset(b as isize),
                self.buf.capacity() - b,
            )
        };

        match self.inner.read(buf) {
            Ok(o) => {
                unsafe {
                    // reset the size to include the written portion,
                    // safe because the extra data is initialized
                    self.buf.set_len(b + o);
                }

                Ok(&self.buf[self.consumed..])
            }
            Err(e) => Err(e),
        }
    }

    fn consume(&mut self, amt: usize) {
        self.consumed += amt;
    }
}

fn main() {}
