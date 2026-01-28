use core::{mem, slice};

use crate::strings;

mod private {
    pub trait Sealed {}
}
use private::Sealed;

pub trait Node: Sealed {
    fn signature(&self) -> &strings::Signature;
}

impl Sealed for u8 {}
impl Node for u8 {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(unsafe { slice::from_raw_parts(self, 1) })
    }
}
impl<const N: usize> Sealed for [u8; N] {}
impl<const N: usize> Node for [u8; N] {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(self)
    }
}
impl Sealed for () {}
impl Node for () {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(&[])
    }
}
impl<X: Node, Y: Node> Sealed for Pair<X, Y> {}
impl<X: Node, Y: Node> Node for Pair<X, Y> {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(unsafe {
            slice::from_raw_parts(self as *const Self as _, mem::size_of::<Self>())
        })
    }
}
impl<X: Node, Y: Node, Z: Node> Sealed for Triple<X, Y, Z> {}
impl<X: Node, Y: Node, Z: Node> Node for Triple<X, Y, Z> {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(unsafe {
            slice::from_raw_parts(self as *const Self as _, mem::size_of::<Self>())
        })
    }
}
impl<X: Node, Y: Node, Z: Node, W: Node> Sealed for Quadruple<X, Y, Z, W> {}
impl<X: Node, Y: Node, Z: Node, W: Node> Node for Quadruple<X, Y, Z, W> {
    fn signature(&self) -> &strings::Signature {
        strings::Signature::from_bytes(unsafe {
            slice::from_raw_parts(self as *const Self as _, mem::size_of::<Self>())
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pair<X, Y>(pub X, pub Y);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Triple<X, Y, Z>(pub X, pub Y, pub Z);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Quadruple<X, Y, Z, W>(pub X, pub Y, pub Z, pub W);

pub unsafe trait MultiSignature {
    type Data: Node;
    const DATA: Self::Data;
}

pub unsafe trait Signature: MultiSignature {
    const ALIGNMENT: usize;
}

pub trait SignatureProxy {
    type Proxy: Signature + ?Sized;
}

impl<T: Signature + ?Sized> SignatureProxy for &T {
    type Proxy = T;
}

unsafe impl<T: SignatureProxy + ?Sized> MultiSignature for T {
    type Data = <T::Proxy as MultiSignature>::Data;
    const DATA: Self::Data = T::Proxy::DATA;
}

unsafe impl<T: SignatureProxy + ?Sized> Signature for T {
    const ALIGNMENT: usize = T::Proxy::ALIGNMENT;
}

macro_rules! impl_signature {
    ($($t:ty = $s:literal),* $(,)?) => {
        $(unsafe impl Signature for $t {
            const ALIGNMENT: usize = core::mem::align_of::<Self>();
        })*
        $(unsafe impl MultiSignature for $t {
            type Data = u8;
            const DATA: Self::Data = $s;
        })*
    };
}

impl_signature! {
    u8 = b'y',
    i16 = b'n',
    u16 = b'q',
    i32 = b'i',
    u32 = b'u',
    i64 = b'x',
    u64 = b't',
    f64 = b'd',
}

unsafe impl MultiSignature for bool {
    type Data = u8;

    const DATA: Self::Data = b'b';
}
unsafe impl Signature for bool {
    const ALIGNMENT: usize = 4;
}

unsafe impl MultiSignature for str {
    type Data = u8;
    const DATA: Self::Data = b's';
}
unsafe impl Signature for str {
    const ALIGNMENT: usize = 4;
}

unsafe impl MultiSignature for strings::String {
    type Data = u8;
    const DATA: Self::Data = b's';
}
unsafe impl Signature for strings::String {
    const ALIGNMENT: usize = 4;
}

unsafe impl MultiSignature for strings::Signature {
    type Data = u8;
    const DATA: Self::Data = b'g';
}
unsafe impl Signature for strings::Signature {
    const ALIGNMENT: usize = 1;
}

unsafe impl MultiSignature for strings::ObjectPath {
    type Data = u8;
    const DATA: Self::Data = b'o';
}
unsafe impl Signature for strings::ObjectPath {
    const ALIGNMENT: usize = 4;
}

unsafe impl<T: Signature> MultiSignature for [T] {
    type Data = Pair<u8, T::Data>;
    const DATA: Self::Data = Pair(b'a', T::DATA);
}
unsafe impl<T: Signature> Signature for [T] {
    const ALIGNMENT: usize = 4;
}

#[test]
fn test_signature() {
    type T = crate::struct_type!(u8, u32, i16);
    const XS: crate::struct_type!(i32, f32, u8) = crate::struct_new!(1i32, 2.0, 2u8);
    let crate::struct_match!(x, _, z) = XS;
    assert_eq!(x, 1);
    assert_eq!(z, 2);

    assert_eq!(T::DATA.signature(), strings::Signature::from_str("(yun)"));
}
