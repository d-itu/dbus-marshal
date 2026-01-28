use core::{marker::PhantomData, result, slice};

use crate::{
    aligned,
    signature::{Node, Signature},
    strings,
    types::*,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    SignatureInvalidChar,
    UnexpectedType,
    InvalidEntrySize,
    NestingMismatched,
    NotEnoughData,
    InvalidHeader,
    UnsupportedEndian,
    NestingDepthExceeded,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Clone, Copy)]
pub struct Reader<'a> {
    begin: *const u8,
    len: usize,
    count: usize,
    marker: PhantomData<&'a [u8]>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            begin: data.as_ptr(),
            len: data.len(),
            count: 0,
            marker: PhantomData,
        }
    }
    fn seek_unchecked(&mut self, n: usize) {
        self.count += n;
    }
    pub fn seek(&mut self, n: usize) -> Result<Self> {
        if self.count + n > self.len {
            Err(Error::NotEnoughData)?;
        }
        let res = Self {
            len: self.count + n,
            ..*self
        };
        self.seek_unchecked(n);
        Ok(res)
    }
    fn aligned(&self, align: usize) -> Result<usize> {
        let aligned = aligned(self.count, align);
        if aligned > self.len {
            Err(Error::NotEnoughData)?;
        }
        Ok(aligned)
    }
    pub fn align_to(&mut self, align: usize) -> Result<()> {
        self.count = self.aligned(align)?;
        Ok(())
    }
    pub fn remaining(&self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.begin.add(self.count), self.len - self.count) }
    }
    pub fn read<T: Unmarshal<'a>>(&mut self) -> Result<T> {
        T::unmarshal(self)
    }
    pub fn read_byte(&mut self) -> Result<u8> {
        let res = *self.remaining().get(0).ok_or(Error::NotEnoughData)?;
        self.seek_unchecked(1);
        Ok(res)
    }
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let res = self.remaining().get(..len).ok_or(Error::NotEnoughData)?;
        self.seek_unchecked(len);
        Ok(res)
    }
    fn next_string_like(&mut self) -> Result<&'a [u8]> {
        let len = self.read::<u32>()? as usize;
        let res = self.remaining().get(..len).ok_or(Error::NotEnoughData)?;
        self.seek_unchecked(len + 1); // sentinel 0
        Ok(res)
    }
}

pub trait Unmarshal<'a>: Sized {
    /// read without checking signature
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self>;
}

macro_rules! impl_unmarshal {
    ($($t: ty),* $(,)?) => {
        $(impl Unmarshal<'_> for $t {
            fn unmarshal(r: &mut Reader) -> Result<Self> {
                r.align_to(core::mem::align_of::<Self>())?;
                let bytes = r
                    .remaining()
                    .get(..core::mem::size_of::<Self>())
                    .ok_or(Error::NotEnoughData)?;
                let res = Self::from_ne_bytes(bytes.as_array().copied().unwrap());
                r.seek_unchecked(core::mem::size_of::<Self>());
                Ok(res)
            }
        })*
    };
}

impl_unmarshal!(u8, i16, u16, i32, u32, i64, u64, f64);

impl Unmarshal<'_> for bool {
    fn unmarshal(r: &mut Reader) -> Result<Self> {
        u32::unmarshal(r).map(|x| x != 0)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::String {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        r.next_string_like().map(strings::String::from_bytes)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::ObjectPath {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        r.next_string_like().map(strings::ObjectPath::from_bytes)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::Signature {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        let len = r.read::<u8>()? as usize;
        let res = r
            .remaining()
            .get(..len)
            .ok_or(Error::NotEnoughData)
            .map(strings::Signature::from_bytes)?;
        r.seek_unchecked(len + 1);
        Ok(res)
    }
}

impl<'a, T: Unmarshal<'a> + Signature> Unmarshal<'a> for Variant<T> {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        let sig: &strings::Signature = r.read()?;
        if sig != T::DATA.signature() {
            Err(Error::UnexpectedType)?
        }
        let inner = r.read()?;
        Ok(Self(inner))
    }
}

impl<'a, K: Unmarshal<'a>, V: Unmarshal<'a>> Unmarshal<'a> for Entry<K, V> {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        r.align_to(8)?;
        Ok(Self(K::unmarshal(r)?, V::unmarshal(r)?))
    }
}

impl Unmarshal<'_> for Empty {
    fn unmarshal(_: &mut Reader<'_>) -> Result<Self> {
        Ok(Empty)
    }
}

impl<'a, Xs: Unmarshal<'a>, X: Unmarshal<'a>> Unmarshal<'a> for Append<Xs, X> {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        Ok(Self(Xs::unmarshal(r)?, X::unmarshal(r)?))
    }
}

impl<'a, T: Unmarshal<'a> + StructConstructor> Unmarshal<'a> for Struct<T> {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        r.align_to(8)?;
        Ok(Self(T::unmarshal(r)?))
    }
}

pub struct ArrayIter<'a, T> {
    reader: Reader<'a>,
    marker: PhantomData<T>,
}

impl<'a, T: Signature + Unmarshal<'a>> ArrayIter<'a, T> {
    fn next(&mut self) -> iter::IterResult<T> {
        if self.reader.remaining().is_empty() {
            Err(iter::IterErr::EndOfIteration)?
        }
        self.reader.align_to(T::ALIGNMENT)?;
        Ok(self.reader.read()?)
    }
}

impl<'a, T: Signature + Unmarshal<'a>> Iterator for ArrayIter<'a, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        flatten(self.next())
    }
}

impl<'a, T: Unmarshal<'a> + Signature> Unmarshal<'a> for ArrayIter<'a, T> {
    fn unmarshal(r: &mut Reader<'a>) -> Result<Self> {
        let len: u32 = r.read()?;
        r.align_to(T::ALIGNMENT)?;
        Ok(Self {
            reader: r.seek(len as _)?,
            marker: PhantomData,
        })
    }
}

mod iter;
pub use iter::*;
