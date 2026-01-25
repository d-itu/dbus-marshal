use crate::strings;

mod private {
    use core::marker::Destruct;
    pub trait Node: const Destruct {}
}

impl Node for u8 {}
impl<const N: usize> Node for [u8; N] {}
impl Node for () {}
use private::Node;
impl<X: Node, Y: Node> Node for Pair<X, Y> {}
impl<X: Node, Y: Node, Z: Node> Node for Triple<X, Y, Z> {}
impl<X: Node, Y: Node, Z: Node, W: Node> Node for Quadruple<X, Y, Z, W> {}

#[repr(packed)]
#[derive(Clone, Copy)]
pub struct Pair<X, Y>(pub X, pub Y);

#[repr(packed)]
#[derive(Clone, Copy)]
pub struct Triple<X, Y, Z>(pub X, pub Y, pub Z);

#[repr(packed)]
#[derive(Clone, Copy)]
pub struct Quadruple<X, Y, Z, W>(pub X, pub Y, pub Z, pub W);

pub trait SignatureProxy {
    type Proxy: MultiSignature + ?Sized;
}

pub unsafe trait MultiSignature {
    type Data: Node;
    const DATA: Self::Data;
}

unsafe impl<T: SignatureProxy + ?Sized> MultiSignature for T {
    type Data = <T::Proxy as MultiSignature>::Data;
    const DATA: Self::Data = T::Proxy::DATA;
}

impl<T: MultiSignature + ?Sized> SignatureProxy for &T {
    type Proxy = T;
}

pub unsafe trait Signature: MultiSignature {
    const ALIGNMENT: usize;
}

unsafe impl<T: Signature + ?Sized> Signature for &T {
    const ALIGNMENT: usize = T::ALIGNMENT;
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
