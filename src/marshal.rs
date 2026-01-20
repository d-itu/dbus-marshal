use core::{ptr, result};

use crate::{
    aligned,
    signature::{self, Signature},
    strings,
};

#[derive(Debug)]
pub enum Error {}

pub type Result<T> = result::Result<T, Error>;

pub unsafe trait Write: Copy {
    fn position(&self) -> usize;

    #[must_use]
    fn align_to(&mut self, n: usize) -> Result<()>;

    #[must_use]
    fn skip(&mut self, n: usize) -> Result<()>;

    #[must_use]
    fn skip_aligned(&mut self, n: usize) -> Result<Self> {
        let result = *self;
        self.align_to(n)?;
        self.skip(n)?;
        Ok(result)
    }

    #[must_use]
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()>;

    #[must_use]
    fn write_byte(&mut self, byte: u8) -> Result<()>;

    #[must_use]
    fn write_value<T: Marshal>(&mut self, v: T) -> Result<()> {
        v.marshal(self)?;
        Ok(())
    }
}

unsafe impl Write for usize {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        *self += bytes.len();
        Ok(())
    }
    fn write_byte(&mut self, _: u8) -> Result<()> {
        *self += 1;
        Ok(())
    }
    fn align_to(&mut self, n: usize) -> Result<()> {
        *self = aligned(*self, n);
        Ok(())
    }
    fn skip(&mut self, n: usize) -> Result<()> {
        *self += n;
        Ok(())
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
    fn len(&self) -> usize {
        self.end.addr() - self.begin.addr()
    }
}

unsafe impl Write for Span {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.end, bytes.len());
            self.end = self.end.add(bytes.len());
        }
        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<()> {
        unsafe {
            *self.end = byte;
            self.end = self.end.add(1);
        };
        Ok(())
    }

    fn skip(&mut self, n: usize) -> Result<()> {
        unsafe { self.end = self.end.add(n) };
        Ok(())
    }

    fn align_to(&mut self, n: usize) -> Result<()> {
        self.end = unsafe { self.begin.add(aligned(self.len(), n)) };
        Ok(())
    }

    fn position(&self) -> usize {
        self.len()
    }
}

pub trait Marshal: Copy {
    type Signature: Signature + ?Sized;

    #[must_use]
    fn marshal<W: Write>(self, w: &mut W) -> Result<()>;
}

macro_rules! impl_marshal {
    ($($t: ty),* $(,)?) => {
        $(impl Marshal for $t {
            type Signature = $t;

            fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
                w.align_to(core::mem::align_of::<$t>())?;
                w.write_bytes(&self.to_ne_bytes())?;
                Ok(())
            }
        })*
    };
}

impl<T: Marshal> Marshal for &T {
    type Signature = T::Signature;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        w.write_value(self)
    }
}

impl_marshal!(u8, i16, u16, i32, u32, i64, u64, f64);

impl Marshal for bool {
    type Signature = bool;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        w.align_to(4)?;
        match self {
            true => 1u32,
            false => 0u32,
        }
        .marshal(w)?;
        Ok(())
    }
}

fn write_string<W: Write>(w: &mut W, string: &[u8]) -> Result<()> {
    w.write_value(string.len() as u32)?;
    w.write_bytes(string)?;
    w.write_byte(0)
}

impl Marshal for &strings::String {
    type Signature = strings::String;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        write_string(w, self)
    }
}

impl Marshal for &strings::Signature {
    type Signature = strings::Signature;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        w.write_byte(self.len() as _)?;
        w.write_bytes(self)?;
        w.write_byte(0)
    }
}

impl Marshal for &strings::ObjectPath {
    type Signature = strings::ObjectPath;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        write_string(w, self)
    }
}

#[derive(Clone, Copy)]
pub struct Variant<T: Marshal>(T);

impl<T: Marshal> Marshal for Variant<T> {
    type Signature = strings::ObjectPath;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        let sig = crate::signature_bytes!(Self::Signature);
        let sig = strings::Signature::from_bytes(sig);
        w.write_value(sig)?;
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

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        let mut len_writer = w.skip_aligned(4)?;
        for v in &self.0 {
            w.write_value(v)?;
        }
        let len = w.position() - len_writer.position();
        len_writer.write_value(len as u32)
    }
}

#[derive(Clone, Copy)]
pub struct DictEntry<K: Marshal, V: Marshal>(pub K, pub V);
impl<K: Marshal, V: Marshal> Marshal for DictEntry<K, V> {
    type Signature = signature::DictEntry<K::Signature, V::Signature>;

    fn marshal<W: Write>(self, w: &mut W) -> Result<()> {
        w.align_to(8)?;
        w.write_value(self.0)?;
        w.write_value(self.1)
    }
}

pub fn calc_size<Value: Marshal>(value: Value) {
    let mut count = 0;
    value.marshal(&mut count).unwrap();
}

#[test]
fn test_marshal() {
    0.marshal(&mut 0).unwrap();
    DictEntry(0, &&1).marshal(&mut 0).unwrap();
    let arr = Array([0]);
    arr.marshal(&mut 0).unwrap();
}
