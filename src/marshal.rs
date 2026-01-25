use core::mem::MaybeUninit;

use crate::{marshal::writer::*, signature::Signature, strings, types::*};

pub const trait Marshal: Copy {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W);
}

macro_rules! impl_marshal {
    ($($t: ty),* $(,)?) => {
        $(impl const Marshal for $t {
            fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
                w.align_to(core::mem::align_of::<$t>());
                w.write_bytes(&self.to_ne_bytes());
            }
        })*
    };
}

impl_marshal!(u8, i16, u16, i32, u32, i64, u64, f64);

macro_rules! impl_non_zero {
    ($($t: ty),* $(,)?) => {
        $(impl const Marshal for core::num::NonZero<$t> {
            fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
                w.write(self.get());
            }
        })*
    };
}

impl_non_zero!(u8, i16, u16, i32, u32, i64, u64);

impl<T: [const] Marshal> const Marshal for &T {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.write(*self)
    }
}

impl const Marshal for bool {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.align_to(4);
        match self {
            true => 1u32,
            false => 0u32,
        }
        .marshal(w)
    }
}

const fn write_string_like<W: [const] Write + ?Sized>(w: &mut W, string: &[u8]) {
    w.write(string.len() as u32);
    w.write_bytes(string);
    w.write_byte(0)
}

impl const Marshal for &strings::String {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        write_string_like(w, self.as_bytes())
    }
}

impl const Marshal for &strings::Signature {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.write_byte(self.as_bytes().len() as _);
        w.write_bytes(self.as_bytes());
        w.write_byte(0)
    }
}

impl const Marshal for &strings::ObjectPath {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        write_string_like(w, self.as_bytes())
    }
}

impl<T: [const] Marshal + Signature> const Marshal for Variant<T> {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.write(crate::signature!(T));
        w.write(self.0)
    }
}

impl<K: [const] Marshal, V: [const] Marshal> const Marshal for Entry<K, V> {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.align_to(8);
        w.write(self.0);
        w.write(self.1);
    }
}

impl const Marshal for Empty {
    fn marshal<W: [const] Write + ?Sized>(self, _: &mut W) {}
}
impl<Xs: [const] Marshal, X: [const] Marshal> const Marshal for Append<Xs, X> {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        let Self(xs, x) = self;
        w.write(xs);
        w.write(x);
    }
}
impl<T: [const] Marshal + StructConstructor> const Marshal for Struct<T> {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        w.align_to(8);
        w.write(self.0);
    }
}

const fn marshal_array_elements<T: [const] Marshal, W: [const] Write + ?Sized>(
    arr: &[T],
    w: &mut W,
) {
    if let [x, xs @ ..] = arr {
        w.write(x);
        marshal_array_elements(xs, w)
    }
}

impl<T: Signature + [const] Marshal> const Marshal for &[T] {
    fn marshal<W: [const] Write + ?Sized>(self, w: &mut W) {
        let insert_pos = w.skip_aligned(4);
        w.align_to(T::ALIGNMENT);
        let begin = w.position();
        marshal_array_elements(self, w);
        let len = w.position() - begin;
        w.insert(len as u32, insert_pos);
    }
}

pub const fn calc_size<Value: [const] Marshal>(value: Value) -> usize {
    let mut count = 0;
    value.marshal(&mut count);
    count
}

/// safety: caller must ensure that `ptr` is valid for writing `calc_size(value)` bytes.
pub const unsafe fn write_unchecked<Value: [const] Marshal>(value: Value, ptr: *mut u8) {
    let mut writer = Span::new(ptr);
    value.marshal(&mut writer);
}

pub const fn write<Value: [const] Marshal>(
    value: Value,
    buf: &mut [MaybeUninit<u8>],
) -> Result<(&mut [u8], &mut [MaybeUninit<u8>]), ()> {
    let size = calc_size(value);
    let (write, remaining) = buf.split_at_mut_checked(size).ok_or(())?;
    unsafe {
        write_unchecked(value, write.as_ptr() as _);
        let write = write.assume_init_mut();
        Ok((write, remaining))
    }
}

#[macro_export]
macro_rules! marshal_const {
    ($vis:vis const $iden:ident = $expr:expr) => {
        $vis const $iden: [u8; $crate::marshal::calc_size($expr)] = {
            let mut buf = [0; $crate::marshal::calc_size($expr)];
            unsafe { $crate::marshal::write_unchecked($expr, buf.as_mut_ptr() as _) };
            buf
        };
    };
}

#[cfg(any(feature = "std", test))]
#[must_use]
pub fn marshal<Value: Marshal>(value: Value) -> Box<[u8]> {
    #[cfg(any(test, debug_assertions))]
    let mut buf = Box::new_zeroed_slice(calc_size(value));

    #[cfg(not(any(test, debug_assertions)))]
    let mut res = Box::new_uninit_slice(calc_size(value));

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
    marshal_const!(const X = 1u16);
    static_assertions::const_assert!(match X {
        [1, 0] => true,
        _ => false,
    });

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
