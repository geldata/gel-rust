use std::mem::MaybeUninit;

#[derive(Debug)]
pub struct BufWriter<'a> {
    buf: &'a mut [MaybeUninit<u8>],
    size: usize,
    error: bool,
}

impl<'a> BufWriter<'a> {
    /// Create a new `BufWriter` from a slice of bytes.
    #[inline(always)]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            // SAFETY: it's safe to go the other way as long as we never
            // uninitialize bytes
            buf: unsafe { std::mem::transmute(buf) },
            size: 0,
            error: false,
        }
    }

    /// Create a new `BufWriter` from a slice of uninitialized bytes.
    #[inline(always)]
    pub fn new_uninit(buf: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            buf,
            size: 0,
            error: false,
        }
    }

    #[inline]
    pub fn test(&mut self, size: usize) -> bool {
        if self.buf.len() < size {
            self.size += size;
            self.error = true;
            false
        } else {
            true
        }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn write_rewind(&mut self, offset: usize, buf: &[u8]) {
        if self.error {
            return;
        }
        self.buf[offset..offset + buf.len()].copy_from_slice(unsafe { std::mem::transmute(buf) });
    }

    #[inline]
    pub fn write(&mut self, buf: &[u8]) {
        let len = buf.len();
        self.size += len;
        if self.error {
            return;
        }
        if self.buf.len() < len {
            self.error = true;
            return;
        }
        self.buf[self.size - len..self.size].copy_from_slice(unsafe { std::mem::transmute(buf) });
    }

    #[inline]
    pub fn write_u8(&mut self, value: u8) {
        self.size += 1;
        if self.error {
            return;
        }
        if self.buf.is_empty() {
            self.error = true;
            return;
        }
        self.buf[self.size - 1].write(value);
    }

    pub const fn finish(self) -> Result<usize, usize> {
        if self.error {
            Err(self.size)
        } else {
            Ok(self.size)
        }
    }

    /// Finish the writer and return a slice of the written bytes.
    pub const fn finish_buf(self) -> Result<&'a mut [u8], usize> {
        if self.error {
            Err(self.size)
        } else {
            // SAFETY: we know that the buffer is valid because we've written to
            // every byte
            Ok(unsafe { std::slice::from_raw_parts_mut(self.buf.as_mut_ptr().cast(), self.size) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_buf_writer() {
        let mut buf = [0u8; 10];
        let mut writer = BufWriter::new(&mut buf);
        writer.write(b"hello");
        assert_eq!(writer.size(), 5);
    }

    #[test]
    fn test_buf_writer_too_large() {
        let mut buf = [0u8; 10];
        let mut writer = BufWriter::new(&mut buf);
        writer.write(b"hello world");
        assert_eq!(writer.size(), 11);
        assert!(writer.error);
    }
}
