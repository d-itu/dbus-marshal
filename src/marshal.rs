use core::{
    mem::{self, MaybeUninit},
    num::NonZero,
};

use crate::{
    marshal::writer::*,
    signature::{Node as _, Signature, SignatureProxy},
    strings,
    types::*,
};

pub trait Marshal: Clone {
    fn marshal<W: Write + ?Sized>(self, w: &mut W);
}

macro_rules! impl_marshal {
    ($($t: ty),* $(,)?) => {
        $(impl Marshal for $t {
            fn marshal<W: Write + ?Sized>(self, w: &mut W) {
                w.align_to(mem::align_of::<$t>());
                w.write_bytes(&self.to_ne_bytes());
            }
        })*
    };
}

impl_marshal!(u8, i16, u16, i32, u32, i64, u64, f64);

macro_rules! impl_non_zero {
    ($($t: ty),* $(,)?) => {
        $(impl Marshal for NonZero<$t> {
            fn marshal<W: Write + ?Sized>(self, w: &mut W) {
                w.write(self.get());
            }
        })*
    };
}

impl_non_zero!(u8, i16, u16, i32, u32, i64, u64);

impl<T: Marshal> Marshal for &T {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.write(self.clone())
    }
}

impl Marshal for bool {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.align_to(4);
        match self {
            true => 1u32,
            false => 0u32,
        }
        .marshal(w)
    }
}

fn write_string_like<W: Write + ?Sized>(w: &mut W, string: &[u8]) {
    w.write(string.len() as u32);
    w.write_bytes(string);
    w.write_byte(0)
}

impl Marshal for &str {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        write_string_like(w, self.as_bytes())
    }
}

impl Marshal for &strings::String {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        write_string_like(w, self.as_bytes())
    }
}

impl Marshal for &strings::Signature {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.write_byte(self.as_bytes().len() as _);
        w.write_bytes(self.as_bytes());
        w.write_byte(0)
    }
}

impl Marshal for &strings::ObjectPath {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        write_string_like(w, self.as_bytes())
    }
}

impl<T: Marshal + Signature> Marshal for Variant<T> {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.write(T::DATA.signature());
        w.write(self.0)
    }
}

impl<K: Marshal, V: Marshal> Marshal for Entry<K, V> {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.align_to(8);
        w.write(self.0);
        w.write(self.1);
    }
}

impl Marshal for Empty {
    fn marshal<W: Write + ?Sized>(self, _: &mut W) {}
}
impl<Xs: Marshal, X: Marshal> Marshal for Append<Xs, X> {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        let Self(xs, x) = self;
        w.write(xs);
        w.write(x);
    }
}
impl<T: Marshal + StructConstructor> Marshal for Struct<T> {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        w.align_to(8);
        w.write(self.0);
    }
}

fn marshal_array_elements<T: Marshal, W: Write + ?Sized>(arr: &[T], w: &mut W) {
    if let [x, xs @ ..] = arr {
        w.write(x);
        marshal_array_elements(xs, w)
    }
}

impl<T: Signature + Marshal> Marshal for &[T] {
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        let insert_pos = w.skip_aligned(4);
        w.align_to(T::ALIGNMENT);
        let begin = w.position();
        marshal_array_elements(self, w);
        let len = w.position() - begin;
        w.insert(len as u32, insert_pos);
    }
}

#[derive(Clone, Copy)]
pub struct Array<I>(pub I);

impl<I, T> SignatureProxy for Array<I>
where
    I: Iterator<Item = T>,
    T: Signature,
{
    type Proxy = [T];
}

impl<I, T> Marshal for Array<I>
where
    I: Iterator<Item = T> + Clone,
    T: Marshal + Signature,
{
    fn marshal<W: Write + ?Sized>(self, w: &mut W) {
        let insert_pos = w.skip_aligned(4);
        w.align_to(T::ALIGNMENT);
        let begin = w.position();
        for x in self.0 {
            w.write(x);
        }
        let len = w.position() - begin;
        w.insert(len as u32, insert_pos);
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

pub fn write<Value: Marshal>(
    value: Value,
    buf: &mut [MaybeUninit<u8>],
) -> Result<(&mut [u8], &mut [MaybeUninit<u8>]), ()> {
    let size = calc_size(value.clone());
    let (write, remaining) = buf.split_at_mut_checked(size).ok_or(())?;
    unsafe {
        write_unchecked(value, write.as_mut_ptr().cast_init());
        let write = write.assume_init_mut();
        Ok((write, remaining))
    }
}

#[cfg(any(feature = "alloc", test))]
#[must_use]
pub fn marshal<Value: Marshal>(value: Value) -> alloc::boxed::Box<[u8]> {
    let mut buf = alloc::boxed::Box::new_uninit_slice(calc_size(value.clone()));

    unsafe {
        write_unchecked(value, buf.as_mut_ptr() as _);
        buf.assume_init()
    }
}

pub use writer::Write;

mod writer;

#[cfg(target_endian = "little")]
#[test]
fn test_marshal() {
    let x = marshal(&[2u64][..]);
    #[rustfmt::skip]
    assert_eq!(x.as_slice(), [
        8, 0, 0, 0,
        0, 0, 0, 0,
        2, 0, 0, 0, 0, 0, 0, 0
    ]);

    let x = marshal(&[Entry(2i32, 23u8), Entry(3i32, 24u8)][..]);
    #[rustfmt::skip]
    assert_eq!(x.as_slice(), [
        13, 0, 0, 0,
        0, 0, 0, 0,

        2, 0, 0, 0,
        23, 0, 0, 0,

        3, 0, 0, 0,
        24,
    ]);

    let x = marshal(
        &[
            crate::struct_new!(2i32, 23u8),
            crate::struct_new!(3i32, 24u8),
        ][..],
    );
    #[rustfmt::skip]
    assert_eq!(x.as_slice(), [
        13, 0, 0, 0,
        0, 0, 0, 0,

        2, 0, 0, 0,
        23, 0, 0, 0,

        3, 0, 0, 0,
        24,
    ]);
}
