use core::fmt::Debug;

use arrayvec::ArrayVec;

#[derive(Debug)]
pub enum Error<IoError: Debug> {
    AuthenticationFailed,
    NegotiationFailed,
    Io(IoError),
}

impl<IoError: Debug> From<IoError> for Error<IoError> {
    fn from(value: IoError) -> Self {
        Error::Io(value)
    }
}

pub trait Io {
    type Error: Debug;
    fn read(&mut self) -> impl Future<Output = Result<&[u8], Self::Error>>;
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

const fn digits(mut x: u32) -> u32 {
    let mut res = 0;
    while x != 0 {
        x /= 10;
        res += 1;
    }
    res
}

struct DigitIter {
    n: u32,
    base: u32,
}

impl DigitIter {
    const fn new(n: u32) -> Self {
        Self {
            n,
            base: 10u32.pow(digits(n) - 1),
        }
    }
    const fn next(&mut self) -> Option<u32> {
        if self.base == 0 {
            None?
        }
        let res = self.n / self.base;
        self.n -= res * self.base;
        self.base /= 10;
        Some(res)
    }
}

impl Iterator for DigitIter {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

const fn hex_to_ascii(hex: u8) -> u8 {
    if hex < 10 {
        b'0' + hex
    } else {
        b'a' + hex - 10
    }
}

const fn to_ascii(digit: u8) -> [u8; 2] {
    let ascii = digit as u8 + b'0';
    [ascii / 16, ascii % 16].map(hex_to_ascii)
}

static_assertions::const_assert_eq!(to_ascii(1), *b"31");

#[test]
fn test_digit_iter() {
    let mut iter = DigitIter::new(1000);
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), None);
}

pub async fn authenticate<T: Io>(io: &mut T, uid: u32) -> Result<(), Error<T::Error>> {
    let mut buf: ArrayVec<u8, 128> = ArrayVec::new();
    buf.try_extend_from_slice(b"\x00AUTH EXTERNAL ").ok();
    for digit in DigitIter::new(uid) {
        buf.try_extend_from_slice(&to_ascii(digit as _)).ok();
    }
    buf.try_extend_from_slice(b"\r\n").ok();
    io.write(buf.as_slice()).await?;
    if !io.read().await?.starts_with(b"OK") {
        Err(Error::AuthenticationFailed)?
    }

    io.write(b"NEGOTIATE_UNIX_FD\r\nBEGIN\r\n").await?;
    if !io.read().await?.starts_with(b"AGREE_UNIX_FD\r\n") {
        Err(Error::AuthenticationFailed)?
    }

    Ok(())
}
