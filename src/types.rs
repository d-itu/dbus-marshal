#![macro_use]

use core::convert::Infallible;

use crate::signature::{self, MultiSignature, Signature};

#[derive(Clone, Copy)]
pub struct Variant<T: ?Sized = Infallible>(pub T);

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

mod private {
    pub trait StructConstructor {}
}
pub(crate) use private::StructConstructor;

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Empty;
unsafe impl MultiSignature for Empty {
    type Data = ();
    const DATA: Self::Data = ();
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Append<Xs, X>(pub Xs, pub X);
impl<X, Xs> StructConstructor for Append<X, Xs> {}
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

#[allow(dead_code)]
#[test]
fn test_dict() {
    {
        #[derive(Clone, Copy, crate::Dict)]
        #[crate::dict]
        struct Person<'b> {
            #[name = "hello"]
            name: &'b crate::String,
            age: u8,
        }
    }
    {
        #[derive(Clone, Copy, crate::Dict)]
        #[crate::dict]
        struct Foo {
            age: u8,
        }
    }
}
