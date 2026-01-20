use core::ptr;

use crate::{
    aligned,
    signature::{self, Signature},
    strings,
};

pub unsafe trait Write: Copy {
    fn position(&self) -> usize;

    fn align_to(&mut self, n: usize);

    fn skip(&mut self, n: usize);

    #[must_use]
    fn skip_aligned(&mut self, n: usize) -> Self {
        let result = *self;
        self.align_to(n);
        self.skip(n);
        result
    }

    fn write_bytes(&mut self, bytes: &[u8]);

    fn write_byte(&mut self, byte: u8);

    fn write_value<T: Marshal>(&mut self, v: T) {
        v.marshal(self)
    }
}

unsafe impl Write for usize {
    fn write_bytes(&mut self, bytes: &[u8]) {
        *self += bytes.len();
    }
    fn write_byte(&mut self, _: u8) {
        *self += 1;
    }
    fn align_to(&mut self, n: usize) {
        *self = aligned(*self, n);
    }
    fn skip(&mut self, n: usize) {
        *self += n;
    }
    fn position(&self) -> usize {
        *self
    }
}

#[derive(Clone, Copy)]
struct Span {
    begin: *mut u8,
    end: *mut u8,
}

impl Span {
    fn new(ptr: *mut u8) -> Self {
        Self {
            begin: ptr,
            end: ptr,
        }
    }
    fn len(&self) -> usize {
        self.end.addr() - self.begin.addr()
    }
}

unsafe impl Write for Span {
    fn write_bytes(&mut self, bytes: &[u8]) {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.end, bytes.len());
            self.end = self.end.add(bytes.len());
        }
    }

    fn write_byte(&mut self, byte: u8) {
        unsafe {
            *self.end = byte;
            self.end = self.end.add(1);
        };
    }

    fn skip(&mut self, n: usize) {
        unsafe { self.end = self.end.add(n) };
    }

    fn align_to(&mut self, n: usize) {
        self.end = unsafe { self.begin.add(aligned(self.len(), n)) };
    }

    fn position(&self) -> usize {
        self.len()
    }
}

pub trait Marshal: Copy {
    type Signature: Signature + ?Sized;

    fn marshal<W: Write>(self, w: &mut W);
}

macro_rules! impl_marshal {
    ($($t: ty),* $(,)?) => {
        $(impl Marshal for $t {
            type Signature = $t;

            fn marshal<W: Write>(self, w: &mut W) {
                w.align_to(core::mem::align_of::<$t>());
                w.write_bytes(&self.to_ne_bytes());
            }
        })*
    };
}

impl<T: Marshal> Marshal for &T {
    type Signature = T::Signature;

    fn marshal<W: Write>(self, w: &mut W) {
        w.write_value(*self)
    }
}

impl_marshal!(u8, i16, u16, i32, u32, i64, u64, f64);

impl Marshal for bool {
    type Signature = bool;

    fn marshal<W: Write>(self, w: &mut W) {
        w.align_to(4);
        match self {
            true => 1u32,
            false => 0u32,
        }
        .marshal(w)
    }
}

fn write_string<W: Write>(w: &mut W, string: &[u8]) {
    w.write_value(string.len() as u32);
    w.write_bytes(string);
    w.write_byte(0)
}

impl Marshal for &strings::String {
    type Signature = strings::String;

    fn marshal<W: Write>(self, w: &mut W) {
        write_string(w, self)
    }
}

impl Marshal for &strings::Signature {
    type Signature = strings::Signature;

    fn marshal<W: Write>(self, w: &mut W) {
        w.write_byte(self.len() as _);
        w.write_bytes(self);
        w.write_byte(0)
    }
}

impl Marshal for &strings::ObjectPath {
    type Signature = strings::ObjectPath;

    fn marshal<W: Write>(self, w: &mut W) {
        write_string(w, self)
    }
}

#[derive(Clone, Copy)]
pub struct Variant<T: Marshal>(T);

impl<T: Marshal> Marshal for Variant<T> {
    type Signature = strings::ObjectPath;

    fn marshal<W: Write>(self, w: &mut W) {
        let sig = crate::signature_bytes!(Self::Signature);
        let sig = strings::Signature::from_bytes(sig);
        w.write_value(sig);
        w.write_value(self.0)
    }
}

#[derive(Clone, Copy)]
pub struct Array<T, I>(pub I)
where
    I: Copy,
    T: Marshal,
    for<'a> &'a I: IntoIterator<Item = &'a T>;

impl<T, I> Marshal for &Array<T, I>
where
    I: Copy,
    T: Marshal,
    for<'a> &'a I: IntoIterator<Item = &'a T>,
{
    type Signature = signature::Array<T::Signature>;

    fn marshal<W: Write>(self, w: &mut W) {
        let mut len_writer = w.skip_aligned(4);
        for v in &self.0 {
            w.write_value(v);
        }
        let len = w.position() - len_writer.position();
        len_writer.write_value(len as u32)
    }
}

#[derive(Clone, Copy)]
pub struct DictEntry<K: Marshal, V: Marshal>(pub K, pub V);
impl<K: Marshal, V: Marshal> Marshal for DictEntry<K, V> {
    type Signature = signature::DictEntry<K::Signature, V::Signature>;

    fn marshal<W: Write>(self, w: &mut W) {
        w.align_to(8);
        w.write_value(self.0);
        w.write_value(self.1)
    }
}

pub fn calc_size<Value: Marshal>(value: Value) -> usize {
    let mut count = 0;
    value.marshal(&mut count);
    count
}

/// safety: caller must ensure that `ptr` is valid for writing `calc_size(value)` bytes.
pub unsafe fn write_unchecked<Value: Marshal>(value: Value, ptr: *mut u8) {
    let mut writer = Span::new(ptr);
    value.marshal(&mut writer);
}

pub fn marshal<Value: Marshal>(value: Value) -> Box<[u8]> {
    let mut res = Box::new_uninit_slice(calc_size(value));
    unsafe {
        write_unchecked(value, res.as_mut_ptr() as _);
        res.assume_init()
    }
}

#[test]
fn test_marshal() {
    0.marshal(&mut 0);
    DictEntry(0, &&1).marshal(&mut 0);
    let arr = Array([0]);
    arr.marshal(&mut 0);
}
