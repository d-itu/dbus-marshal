#![macro_use]

use core::marker::PhantomData;

use crate::strings;

mod private {
    pub trait Node: Copy {}
}
use private::Node;
impl Node for u8 {}
impl<const N: usize> Node for [u8; N] {}
impl Node for () {}
impl<X: Node, Y: Node> Node for Pair<X, Y> {}
impl<X: Node, Y: Node, Z: Node> Node for Triple<X, Y, Z> {}
impl<X: Node, Y: Node, Z: Node, W: Node> Node for Quadruple<X, Y, Z, W> {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pair<X, Y>(X, Y);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Triple<X, Y, Z>(X, Y, Z);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Quadruple<X, Y, Z, W>(X, Y, Z, W);

pub unsafe trait MultiSignature {
    type Data: Node;
    const DATA: Self::Data;
}

pub unsafe trait Signature: MultiSignature {
    const ALIGN: usize;
}

unsafe impl MultiSignature for u8 {
    type Data = u8;
    const DATA: Self::Data = b'y';
}
unsafe impl Signature for u8 {
    const ALIGN: usize = 1;
}

unsafe impl MultiSignature for bool {
    type Data = u8;
    const DATA: Self::Data = b'b';
}
unsafe impl Signature for bool {
    const ALIGN: usize = 4;
}

unsafe impl MultiSignature for i16 {
    type Data = u8;
    const DATA: Self::Data = b'n';
}
unsafe impl Signature for i16 {
    const ALIGN: usize = 2;
}

unsafe impl MultiSignature for u16 {
    type Data = u8;
    const DATA: Self::Data = b'q';
}
unsafe impl Signature for u16 {
    const ALIGN: usize = 2;
}

unsafe impl MultiSignature for i32 {
    type Data = u8;
    const DATA: Self::Data = b'i';
}
unsafe impl Signature for i32 {
    const ALIGN: usize = 4;
}

unsafe impl MultiSignature for u32 {
    type Data = u8;
    const DATA: Self::Data = b'u';
}
unsafe impl Signature for u32 {
    const ALIGN: usize = 4;
}

unsafe impl MultiSignature for i64 {
    type Data = u8;
    const DATA: Self::Data = b'x';
}
unsafe impl Signature for i64 {
    const ALIGN: usize = 8;
}

unsafe impl MultiSignature for u64 {
    type Data = u8;
    const DATA: Self::Data = b't';
}
unsafe impl Signature for u64 {
    const ALIGN: usize = 8;
}

unsafe impl MultiSignature for f64 {
    type Data = u8;
    const DATA: Self::Data = b'd';
}
unsafe impl Signature for f64 {
    const ALIGN: usize = 8;
}

unsafe impl MultiSignature for strings::String {
    type Data = u8;
    const DATA: Self::Data = b's';
}
unsafe impl Signature for strings::String {
    const ALIGN: usize = 4;
}

unsafe impl MultiSignature for strings::Signature {
    type Data = u8;
    const DATA: Self::Data = b'g';
}
unsafe impl Signature for strings::Signature {
    const ALIGN: usize = 1;
}

unsafe impl MultiSignature for strings::ObjectPath {
    type Data = u8;
    const DATA: Self::Data = b'o';
}
unsafe impl Signature for strings::ObjectPath {
    const ALIGN: usize = 4;
}

pub struct Variant;
unsafe impl MultiSignature for Variant {
    type Data = u8;
    const DATA: Self::Data = b'v';
}
unsafe impl Signature for Variant {
    const ALIGN: usize = 1;
}

pub struct Array<T: ?Sized>(T);
unsafe impl<T: Signature + ?Sized> MultiSignature for Array<T> {
    type Data = Pair<u8, T::Data>;
    const DATA: Self::Data = Pair(b'a', T::DATA);
}
unsafe impl<T: Signature + ?Sized> Signature for Array<T> {
    const ALIGN: usize = 4;
}

pub struct DictEntry<K: ?Sized, V: ?Sized>(PhantomData<K>, V);
unsafe impl<K: Signature + ?Sized, V: Signature + ?Sized> MultiSignature for DictEntry<K, V> {
    type Data = Quadruple<u8, K::Data, V::Data, u8>;
    const DATA: Self::Data = Quadruple(b'{', K::DATA, V::DATA, b'}');
}
unsafe impl<K: Signature + ?Sized, V: Signature + ?Sized> Signature for DictEntry<K, V> {
    const ALIGN: usize = 8;
}

pub struct Empty;
unsafe impl MultiSignature for Empty {
    type Data = ();
    const DATA: Self::Data = ();
}

pub struct Append<Xs: ?Sized, X: ?Sized>(PhantomData<Xs>, X);
unsafe impl<Xs: MultiSignature + ?Sized, X: MultiSignature + ?Sized> MultiSignature
    for Append<Xs, X>
{
    type Data = Pair<Xs::Data, X::Data>;
    const DATA: Self::Data = Pair(Xs::DATA, X::DATA);
}

pub struct Struct<T: MultiSignature + ?Sized>(T);
unsafe impl<T: MultiSignature + ?Sized> MultiSignature for Struct<T> {
    type Data = Triple<u8, T::Data, u8>;

    const DATA: Self::Data = Triple(b'(', T::DATA, b')');
}
unsafe impl<T: MultiSignature + ?Sized> Signature for Struct<T> {
    const ALIGN: usize = 8;
}

static_assertions::assert_impl_all!(Append<Empty, u8>: MultiSignature);
static_assertions::assert_not_impl_all!(Append<Empty, ()>: MultiSignature);
static_assertions::assert_not_impl_all!(Append<Empty, u8>: Signature);
static_assertions::assert_impl_all!(Struct<Append<Empty, u8>>: Signature);

#[macro_export]
macro_rules! multi_signature {
    ($x:ty, $($xs:ty),* $(,)?) => {
        crate::signature::Append<$x, crate::multi_signature!($($xs)*)>
    };
    ($x:ty $(,)?) => {
        crate::signature::Append<$x, crate::signature::Empty>
    };
    () => {
        crate::signature::Empty
    };
}

#[macro_export]
macro_rules! signature_bytes_static {
    ($x:ty) => {{
        static_assertions::assert_eq_align!(Data, u8);
        type Data = <$x as crate::signature::MultiSignature>::Data;
        const DATA: Data = <$x as crate::signature::MultiSignature>::DATA;
        let result: &[u8; core::mem::size_of::<Data>()] = unsafe { core::mem::transmute(&DATA) };
        result
    }};
}

#[macro_export]
macro_rules! signature_bytes {
    ($x:ty) => {{
        let data = <$x as crate::signature::MultiSignature>::DATA;
        let ptr = &data as *const _ as *const u8;
        let len = core::mem::size_of_val(&data);
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }};
}

#[test]
const fn test_signature() {
    static_assertions::const_assert!(match signature_bytes_static!(Struct<T>) {
        b"(yu)" => true,
        _ => false,
    });
    type T = multi_signature!(u8, u32);
}
