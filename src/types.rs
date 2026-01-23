#![macro_use]

use crate::signature::{self, MultiSignature, Signature};

#[derive(Clone, Copy)]
pub struct Variant<T: ?Sized>(pub T);

unsafe impl<T> MultiSignature for Variant<T> {
    type Data = u8;
    const DATA: Self::Data = b'v';
}
unsafe impl<T> Signature for Variant<T> {
    const ALIGN: usize = 1;
}

#[derive(Clone, Copy)]
pub struct Entry<K, V>(pub K, pub V);

unsafe impl<K: Signature, V: Signature> MultiSignature for Entry<K, V> {
    type Data = signature::Quadruple<u8, K::Data, V::Data, u8>;
    const DATA: Self::Data = signature::Quadruple(b'{', K::DATA, V::DATA, b'}');
}
unsafe impl<K: Signature, V: Signature> Signature for Entry<K, V> {
    const ALIGN: usize = 8;
}

unsafe impl<T: Signature> MultiSignature for [T] {
    type Data = signature::Pair<u8, T::Data>;
    const DATA: Self::Data = signature::Pair(b'a', T::DATA);
}
unsafe impl<T: Signature> Signature for [T] {
    const ALIGN: usize = 4;
}

// #[derive(Clone, Copy)]
// pub struct Array<I>(I);
//
// unsafe impl<T, I> MultiSignature for Array<I>
// where
//     T: Signature,
//     for<'a> &'a I: IntoIterator<Item = &'a T>,
// {
//     type Data = signature::Pair<u8, T::Data>;
//     const DATA: Self::Data = signature::Pair(b'a', T::DATA);
// }
// unsafe impl<T, I> Signature for Array<I>
// where
//     T: Signature,
//     for<'a> &'a I: IntoIterator<Item = &'a T>,
// {
//     const ALIGN: usize = 4;
// }

mod private {
    pub trait StructConstructor {}
}
pub(crate) use private::StructConstructor;

#[derive(Clone, Copy)]
pub struct Empty;
impl StructConstructor for Empty {}
unsafe impl MultiSignature for Empty {
    type Data = ();
    const DATA: Self::Data = ();
}

#[derive(Clone, Copy)]
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
    const ALIGN: usize = 8;
}

#[macro_export]
macro_rules! struct_constructor {
    ($x:ty, $($xs:ty),* $(,)?) => {
        crate::types::Append<$x, crate::struct_constructor!($($xs),*)>
    };
    ($x:ty $(,)?) => {
        crate::types::Append<$x, crate::types::Empty>
    };
    () => {
        crate::types::Empty
    };
}

#[macro_export]
macro_rules! struct_type {
    ($($xs:ty),* $(,)? ) => {
        crate::types::Struct<crate::struct_constructor!($($xs),*)>
    };
}

#[macro_export]
macro_rules! struct_constructor_new {
    ($x:expr, $($xs:expr),* $(,)?) => {
        crate::types::Append($x, crate::struct_constructor_new!($($xs),*))
    };
    ($x:expr $(,)?) => {
        crate::types::Append($x, crate::types::Empty)
    };
    () => {
        crate::types::Empty
    };
}

#[macro_export]
macro_rules! struct_new {
    ($($xs:expr),* $(,)? ) => {
        crate::types::Struct(crate::struct_constructor_new!($($xs),*))
    };
}

#[macro_export]
macro_rules! signature_bytes_static {
    ($x:ty) => {{
        type Data = <$x as crate::signature::MultiSignature>::Data;
        static_assertions::assert_eq_align!(Data, u8);
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
    static_assertions::const_assert!(match signature_bytes_static!(T) {
        b"(yun)" => true,
        _ => false,
    });
    type T = struct_type!(u8, u32, i16);
}
