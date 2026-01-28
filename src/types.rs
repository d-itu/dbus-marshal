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
macro_rules! define_dict {
    ($(#[$meta:meta])* $pub:vis struct $name:ident($entry:ident, $key:ident, $value:ident $(,)?) $(<$a:lifetime>)? {
        $($field_pub:vis $field:ident: $type:ty),* $(,)?
    }) => {
        $(#[$meta])*
        $pub struct $name<$($a)?> {
            $($field_pub $field: Option<$type>,)*
        }
        impl<$($a)?> $crate::signature::SignatureProxy for $name<$($a)?> {
            type Proxy = [$crate::Entry<&'static str, $crate::Variant<()>>];
        }
        impl<$($a)?> $crate::marshal::Marshal for $name<$($a)?> where Self: Clone {
            fn marshal<W: $crate::marshal::Write + ?Sized>(self, w: &mut W) {
                let insert_pos = w.skip_aligned(4);
                let begin = w.position();
                $(if let Some(value) = self.$field {
                    w.align_to(8);
                    w.write(stringify!($field));
                    w.write(Variant(value));
                })*
                let len = w.position() - begin;
                w.insert(len as u32, insert_pos);
            }
        }
        crate::define_dict!(@unmarshal $name $entry $key $value $($a)? $($field $type)*);
        #[allow(non_camel_case_types)]
        enum $key {
            $($field),*
        }
        union $value<$($a)?> {
            $($field: $type,)*
        }
    };
    (@unmarshal $name:ident $entry:ident $key:ident $value:ident $lifetime:lifetime $($field:ident $type:ty)*) => {
        impl<'a> $crate::unmarshal::Unmarshal<'a> for $name<'a> {
            fn unmarshal(r: &mut $crate::unmarshal::Reader<'a>) -> $crate::unmarshal::Result<Self> {
                let mut res = Self { $($field: None),* };
                let it: $crate::unmarshal::ArrayIter<'a, $entry> = r.read()?;
                for entry in it {
                    let Entry(key, val) = entry?;
                    match key {
                        $(Key::$field => res.$field = Some(unsafe { val.$field }),)*
                    }
                }
                Ok(res)
            }
        }
        struct $entry<'a>($key, $value<'a>);
        impl $crate::signature::SignatureProxy for $entry<'_> {
            type Proxy = $crate::Entry<&'static str, $crate::Variant<()>>;
        }
        impl<$lifetime> $crate::unmarshal::Unmarshal<$lifetime> for $entry<$lifetime> {
            fn unmarshal(r: &mut $crate::unmarshal::Reader<$lifetime>) -> $crate::unmarshal::Result<Self> {
                let key: &$crate::String = r.read()?;
                match unsafe { core::str::from_utf8_unchecked(key) } {
                    $(stringify!($field) => {
                        let val: Variant<$type> = r.read()?;
                        Ok(Self($key::$field, $value {
                            $field: val.0
                        }))
                    })*
                    _ => Err($crate::unmarshal::Error::UnexpectedType)?
                }
            }
        }
    };
    (@unmarshal $name:ident $entry:ident $key:ident $value:ident $($field:ident $type:ty)*) => {
        impl $crate::unmarshal::Unmarshal<'_> for $name {
            fn unmarshal(_r: &mut $crate::unmarshal::Reader) -> $crate::unmarshal::Result<Self> {
                todo!();
            }
        }
        struct $entry($key, $value);
        impl $crate::signature::SignatureProxy for $entry {
            type Proxy = $crate::Entry<&'static str, $crate::Variant<()>>;
        }
        impl $crate::unmarshal::Unmarshal<'_> for $entry {
            fn unmarshal(r: &mut $crate::unmarshal::Reader<'_>) -> $crate::unmarshal::Result<Self> {
                let key: &$crate::String = r.read()?;
                match unsafe { core::str::from_utf8_unchecked(key) } {
                    $(stringify!($field) => {
                        let val: Variant<$type> = r.read()?;
                        Ok(Self($key::$field, $value {
                            $field: val.0
                        }))
                    })*
                    _ => Err($crate::unmarshal::Error::UnexpectedType)?
                }
            }
        }
    };
}

#[allow(dead_code)]
#[test]
fn test_dict() {
    {
        define_dict! {
            #[derive(Clone, Copy)]
            struct Person(Entry, Key, Value)<'b> {
                name: &'b crate::String,
                age: u8,
            }
        }
    }
    {
        define_dict! {
            #[allow(dead_code)]
            #[derive(Clone, Copy)]
            struct Foo(Entry, Key, Value) {
                age: u8,
            }
        }
    }
}
