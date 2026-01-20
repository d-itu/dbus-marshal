use core::ops::Deref;

#[repr(transparent)]
pub struct Signature([u8]);
#[repr(transparent)]
pub struct String([u8]);
#[repr(transparent)]
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
        }
        impl Deref for $t {
            type Target = [u8];

            fn deref(&self) -> &Self::Target {
                self.as_bytes()
            }
        }
        impl AsRef<[u8]> for $t {
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        })*
    };
}

impl_string!(Signature, String, ObjectPath);
