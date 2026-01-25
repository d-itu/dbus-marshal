#![macro_use]

use crate::signature::{self, MultiSignature, Signature};

#[derive(Clone, Copy)]
pub struct Variant<T: ?Sized>(pub T);

unsafe impl<T> MultiSignature for Variant<T> {
    type Data = u8;
    const DATA: Self::Data = b'v';
}
unsafe impl<T> Signature for Variant<T> {
    const ALIGNMENT: usize = 1;
}

#[derive(Clone, Copy)]
pub struct Entry<K, V>(pub K, pub V);

unsafe impl<K: Signature, V: Signature> MultiSignature for Entry<K, V> {
    type Data = signature::Quadruple<u8, K::Data, V::Data, u8>;
    const DATA: Self::Data = signature::Quadruple(b'{', K::DATA, V::DATA, b'}');
}
unsafe impl<K: Signature, V: Signature> Signature for Entry<K, V> {
    const ALIGNMENT: usize = 8;
}

unsafe impl<T: Signature> MultiSignature for [T] {
    type Data = signature::Pair<u8, T::Data>;
    const DATA: Self::Data = signature::Pair(b'a', T::DATA);
}
unsafe impl<T: Signature> Signature for [T] {
    const ALIGNMENT: usize = 4;
}

mod private {
    pub trait StructConstructor {}
}
pub(crate) use private::StructConstructor;

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Empty;
impl StructConstructor for Empty {}
unsafe impl MultiSignature for Empty {
    type Data = ();
    const DATA: Self::Data = ();
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Append<Xs, X>(pub Xs, pub X);
impl<X, Xs: StructConstructor> StructConstructor for Append<X, Xs> {}
unsafe impl<X: Signature, Xs: MultiSignature> MultiSignature for Append<X, Xs> {
    type Data = signature::Pair<X::Data, Xs::Data>;
    const DATA: Self::Data = signature::Pair(X::DATA, Xs::DATA);
}

#[derive(Clone, Copy)]
pub struct Struct<T: StructConstructor>(pub T);
unsafe impl<T: MultiSignature + StructConstructor> MultiSignature for Struct<T> {
    type Data = signature::Triple<u8, T::Data, u8>;
    const DATA: Self::Data = signature::Triple(b'(', T::DATA, b')');
}
unsafe impl<T: MultiSignature + StructConstructor> Signature for Struct<T> {
    const ALIGNMENT: usize = 8;
}

#[macro_export]
macro_rules! multiple_type {
    ($x:ty, $($xs:ty),* $(,)?) => {
        $crate::Append<$x, $crate::multiple_type!($($xs),*)>
    };
    ($x:ty $(,)?) => {
        $crate::Append<$x, $crate::Empty>
    };
    () => {
        $crate::Empty
    };
}

#[macro_export]
macro_rules! struct_type {
    ($($xs:ty),* $(,)? ) => {
        $crate::Struct<$crate::multiple_type!($($xs),*)>
    };
}

#[macro_export]
macro_rules! multiple_new {
    ($x:expr, $($xs:expr),* $(,)?) => {
        $crate::Append($x, $crate::multiple_new!($($xs),*))
    };
    ($x:expr $(,)?) => {
        $crate::Append($x, $crate::Empty)
    };
    () => {
        $crate::Empty
    };
}

#[macro_export]
macro_rules! struct_new {
    ($($xs:expr),* $(,)? ) => {
        $crate::Struct($crate::multiple_new!($($xs),*))
    };
}

#[macro_export]
macro_rules! multiple_match {
    ($x:pat, $($xs:pat),* $(,)?) => {
        $crate::Append($x, $crate::multiple_match!($($xs),*))
    };
    ($x:pat $(,)?) => {
        $crate::Append($x, $crate::Empty)
    };
    () => {
        $crate::Empty
    };
}

#[macro_export]
macro_rules! struct_match {
    ($($xs:pat),* $(,)? ) => {
        $crate::Struct($crate::multiple_match!($($xs),*))
    };
}

#[macro_export]
macro_rules! signature_static {
    ($x:ty) => {{
        type Data = <$x as $crate::signature::MultiSignature>::Data;
        static_assertions::assert_eq_align!(Data, u8);
        const DATA: Data = <$x as $crate::signature::MultiSignature>::DATA;
        let result: &[u8; core::mem::size_of::<Data>()] = unsafe { core::mem::transmute(&DATA) };
        $crate::strings::Signature::from_bytes(result)
    }};
}

#[macro_export]
macro_rules! signature {
    ($x:ty) => {{
        let data = <$x as $crate::signature::MultiSignature>::DATA;
        let ptr = &data as *const _ as *const u8;
        let len = core::mem::size_of_val(&data);
        $crate::strings::Signature::from_bytes(unsafe { core::slice::from_raw_parts(ptr, len) })
    }};
}

#[test]
fn test_signature() {
    static_assertions::const_assert!(match signature_static!(T).as_bytes() {
        b"(yun)" => true,
        _ => false,
    });
    type T = struct_type!(u8, u32, i16);
    const XS: struct_type!(i32, f32, u8) = struct_new!(1i32, 2.0, 2u8);
    let struct_match!(x, _, z) = XS;
    assert_eq!(x, 1);
    assert_eq!(z, 2);
}
