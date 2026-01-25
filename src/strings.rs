use core::ops::Deref;

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signature([u8]);

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct String([u8]);

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectPath([u8]);

macro_rules! impl_string {
    ($($t:ty),* $(,)?) => {
        $(impl $t {
            pub const fn as_bytes(&self) -> &[u8] {
                &self.0
            }
            pub const fn from_bytes(bytes: &[u8]) -> &Self {
                unsafe { core::mem::transmute(bytes) }
            }
            pub const fn from_str(bytes: &str) -> &Self {
                <$t>::from_bytes(bytes.as_bytes())
            }
        }
        impl const Deref for $t {
            type Target = [u8];

            fn deref(&self) -> &Self::Target {
                self.as_bytes()
            }
        }
        impl core::fmt::Debug for $t {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let s = unsafe { core::str::from_utf8_unchecked(self.as_bytes()) };
                write!(f, "{s:?}")
            }
        }
        impl<'a> const From<&'a str> for &'a $t {
            fn from(s: &'a str) -> Self {
                <$t>::from_str(s)
            }
        }
        impl<'a> const From<&'a [u8]> for &'a $t {
            fn from(s: &'a [u8]) -> Self {
                <$t>::from_bytes(s)
            }
        }
        impl const AsRef<[u8]> for $t {
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        })*
    };
}

impl_string!(Signature, String, ObjectPath);
