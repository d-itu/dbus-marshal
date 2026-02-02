use core::ptr;

use crate::marshal::Marshal;

pub unsafe trait Write {
    fn position(&self) -> usize;

    fn seek(&mut self, n: usize);

    fn align_to(&mut self, n: usize) {
        let padding = crate::aligned(self.position(), n) - self.position();
        self.seek(padding);
    }

    fn skip_aligned(&mut self, n: usize) -> usize {
        self.align_to(n);
        let res = self.position();
        self.seek(n);
        res
    }

    fn write_bytes(&mut self, bytes: &[u8]);

    fn write_byte(&mut self, byte: u8);

    fn write<T: Marshal>(&mut self, v: T) {
        v.marshal(self);
    }

    fn insert<T: Marshal>(&mut self, v: T, pos: usize);
}

unsafe impl Write for usize {
    fn position(&self) -> usize {
        *self
    }

    fn seek(&mut self, n: usize) {
        *self += n;
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        *self += bytes.len();
    }

    fn write_byte(&mut self, _: u8) {
        *self += 1;
    }

    fn insert<T: Marshal>(&mut self, _: T, _: usize) {}
}

pub struct Span {
    begin: *mut u8,
    cursor: *mut u8,
}

impl Span {
    pub const fn new(ptr: *mut u8) -> Self {
        Self {
            begin: ptr,
            cursor: ptr,
        }
    }
    const fn len(&self) -> usize {
        unsafe { self.cursor.byte_offset_from_unsigned(self.begin) }
    }
}

struct Cursor(*mut u8);
unsafe impl Write for Cursor {
    fn position(&self) -> usize {
        unimplemented!()
    }

    fn seek(&mut self, _: usize) {
        unimplemented!()
    }

    fn align_to(&mut self, _: usize) {}

    fn write_bytes(&mut self, bytes: &[u8]) {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.0, bytes.len());
        }
    }

    fn write_byte(&mut self, _: u8) {
        unimplemented!()
    }

    fn insert<T: Marshal>(&mut self, _: T, _: usize) {
        unimplemented!()
    }
}

unsafe impl Write for Span {
    fn write_bytes(&mut self, bytes: &[u8]) {
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), self.cursor, bytes.len()) }
        self.seek(bytes.len())
    }

    fn write_byte(&mut self, byte: u8) {
        unsafe {
            *self.cursor = byte;
            self.cursor = self.cursor.add(1);
        };
    }

    fn seek(&mut self, n: usize) {
        unsafe { self.cursor = self.cursor.add(n) };
    }

    fn align_to(&mut self, n: usize) {
        let padding = crate::align_padding(self.len(), n);
        unsafe { ptr::write_bytes(self.cursor, 0, padding) };
        self.seek(padding);
    }

    fn position(&self) -> usize {
        self.len()
    }

    fn insert<T: Marshal>(&mut self, v: T, pos: usize) {
        Cursor(unsafe { self.begin.add(pos) }).write(v)
    }
}
