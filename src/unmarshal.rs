use core::{marker::PhantomData, result, slice};

use crate::{aligned, strings};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    SignatureInvalidChar,
    InvalidEntrySize,
    NestingMismatched,
    NotEnoughData,
    RedundantData,
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
    pub fn seek(&mut self, n: usize) -> Result<()> {
        if self.count + n > self.len {
            Err(Error::NotEnoughData)?;
        }
        Ok(())
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
    pub fn rest_bytes(&self) -> &'a [u8] {
        unsafe { slice::from_raw_parts(self.begin.add(self.count), self.len - self.count) }
    }
    pub fn next_unchecked<T: Unmarshal<'a>>(&mut self) -> Result<T> {
        T::next_unchecked(self)
    }
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let res = self.rest_bytes().get(..len).ok_or(Error::NotEnoughData)?;
        self.seek_unchecked(len);
        Ok(res)
    }
    fn next_string_like(&mut self) -> Result<&'a [u8]> {
        let len = self.next_unchecked::<u32>()? as usize;
        let res = self.rest_bytes().get(..len).ok_or(Error::NotEnoughData)?;
        self.seek_unchecked(len + 1); // sentinel 0
        Ok(res)
    }
}

pub trait Unmarshal<'a>: Sized {
    /// read without checking signature
    fn next_unchecked(r: &mut Reader<'a>) -> Result<Self>;
}

macro_rules! impl_unmarshal {
    ($($t: ty),* $(,)?) => {
        $(impl Unmarshal<'_> for $t {
            fn next_unchecked(r: &mut Reader) -> Result<Self> {
                r.align_to(core::mem::align_of::<Self>())?;
                let bytes = r
                    .rest_bytes()
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
    fn next_unchecked(r: &mut Reader) -> Result<Self> {
        u32::next_unchecked(r).map(|x| x != 0)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::String {
    fn next_unchecked(r: &mut Reader<'a>) -> Result<Self> {
        r.next_string_like().map(strings::String::from_bytes)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::ObjectPath {
    fn next_unchecked(r: &mut Reader<'a>) -> Result<Self> {
        r.next_string_like().map(strings::ObjectPath::from_bytes)
    }
}

impl<'a> Unmarshal<'a> for &'a strings::Signature {
    fn next_unchecked(r: &mut Reader<'a>) -> Result<Self> {
        let len = r.next_unchecked::<u8>()? as usize;
        let res = r
            .rest_bytes()
            .get(..len)
            .ok_or(Error::NotEnoughData)
            .map(strings::Signature::from_bytes)?;
        r.seek_unchecked(len + 1);
        Ok(res)
    }
}

mod iter;
pub use iter::*;
