use core::{
    fmt::{self, Formatter},
    num::NonZeroU32,
};

use zerocopy::{FromBytes, Immutable, KnownLayout, TryFromBytes, Unaligned};

use crate::{
    marshal::{self, Marshal},
    strings,
    types::Variant,
    unmarshal,
};

#[derive(Debug, Clone, Copy, TryFromBytes, Unaligned, PartialEq, Immutable)]
#[repr(u8)]
pub enum Endian {
    Little = b'l',
    Big = b'B',
}

#[derive(Clone, Copy, TryFromBytes, Unaligned, PartialEq, Immutable)]
#[repr(u8)]
pub enum Version {
    V1 = 1,
}

#[cfg(target_endian = "little")]
const NATIVE_ENDIAN: Endian = Endian::Little;

#[cfg(target_endian = "big")]
const NATIVE_ENDIAN: Endian = Endian::Big;

#[derive(Debug, Clone, Copy, TryFromBytes, Unaligned, Immutable)]
#[repr(u8)]
pub enum MessageType {
    MethodCall = 1,
    MethodReturn = 2,
    Error = 3,
    Signal = 4,
}

#[derive(Clone, Copy, FromBytes, Unaligned, Immutable)]
#[repr(transparent)]
pub struct Flags(u8);

impl Flags {
    pub fn with_no_reply_expected(self) -> Self {
        Self(self.0 | 1)
    }
    pub fn no_reply_expected(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn with_no_auto_start(self) -> Self {
        Self(self.0 | 2)
    }
    pub fn no_auto_start(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn with_allow_interactive_authorization(self) -> Self {
        Self(self.0 | 4)
    }
    pub fn allow_interactive_authorization(self) -> bool {
        self.0 & 4 != 0
    }
}

impl core::fmt::Debug for Flags {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flags")
            .field("no_reply_expected", &self.no_reply_expected())
            .field("no_auto_start", &self.no_auto_start())
            .field(
                "allow_interactive_authorization",
                &self.allow_interactive_authorization(),
            )
            .finish()
    }
}

macro_rules! define_fields {
    ($($id:literal $field:ident: $type:ty),* $(,)?) => {
        #[derive(Default, Debug, Clone, Copy)]
        pub struct Fields<'a> {
            $(pub $field: Option<$type>,)*
        }

        impl<'a> Fields<'a> {
            $(pub fn $field(self, value: impl Into<$type>) -> Self {
                Self {
                    $field: Some(value.into()),
                    ..self
                }
            })*
        }

        impl const Marshal for &Fields<'_> {
            fn marshal<W: [const] marshal::Write + ?Sized>(self, w: &mut W) {
                $(if let Some(value) = self.$field {
                    w.align_to(8);
                    w.write($id as u8);
                    w.write(Variant(value));
                })*
            }
        }
    };
}

define_fields! {
    7 sender: &'a strings::String,
    6 destination: &'a strings::String,
    1 path: &'a strings::ObjectPath,
    2 interface: &'a strings::String,
    3 member: &'a strings::String,
    8 signature: &'a strings::Signature,
    4 error_name: &'a strings::String,
    5 reply_serial: u32,
    9 unix_fds: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Header<'a> {
    pub message_type: MessageType,
    pub flags: Flags,
    pub serial: NonZeroU32,
    pub fields: Fields<'a>,
}

pub struct Message<'a, T> {
    header: Header<'a>,
    body: T,
}

#[repr(C)]
#[derive(TryFromBytes, KnownLayout, Immutable)]
struct Fixed {
    endian: Endian,
    message_type: MessageType,
    flags: Flags,
    version: Version,
    body_len: u32,
    serial: NonZeroU32,
}

impl<'a> Fields<'a> {
    fn from_data(data: &'a [u8]) -> unmarshal::Result<(Self, &'a [u8])> {
        todo!()
    }
}

// impl<'a> Message<'a, Option<unmarshal::Iterator<'a>>> {
//     pub fn from_data(data: &'a [u8]) -> unmarshal::Result<(Self, &'a [u8])> {
//         let begin = data.len();
//         let (
//             Fixed {
//                 endian,
//                 message_type,
//                 flags,
//                 body_len,
//                 serial,
//                 ..
//             },
//             data,
//         ) = Fixed::try_read_from_prefix(data).map_err(|e| match e {
//             ConvertError::Size(_) => Error::NotEnoughData,
//             ConvertError::Validity(_) => Error::InvalidHeader,
//         })?;
//         if endian != NATIVE_ENDIAN {
//             Err(Error::UnsupportEndian)?
//         }
//         let (fields, data) = Fields::from_data(data)?;
//         let position = begin - data.len();
//         let padding = crate::aligned(position, 8) - position;
//         let data = data.get(padding..).ok_or(Error::NotEnoughData)?;
//         let (data, rest) = data
//             .split_at_checked(body_len as _)
//             .ok_or(Error::NotEnoughData)?;
//
//         Ok((
//             Message {
//                 header: Header {
//                     message_type,
//                     flags,
//                     serial,
//                     fields,
//                 },
//                 body: fields
//                     .signature
//                     .map(|signature| unmarshal::Iterator::new(IteratorData { data, signature })),
//             },
//             rest,
//         ))
//     }
// }

impl<T: [const] Marshal> const Marshal for &Message<'_, T> {
    fn marshal<W: [const] marshal::Write + ?Sized>(self, w: &mut W) {
        let Message { header, body } = self;
        w.write_byte(NATIVE_ENDIAN as _);
        w.write_byte(header.message_type as _);
        w.write_byte(header.flags.0);
        w.write_byte(1);
        let body_len_insertion = w.position();
        w.seek(4);
        w.write(header.serial);

        let header_len_insertion = w.position();
        w.seek(4);
        w.align_to(8);
        w.write(&header.fields);
        let header_len = w.position() - 16;
        w.insert(header_len as u32, header_len_insertion);
        w.align_to(8);

        let body_begin = w.position();
        body.marshal(w);
        let body_len = w.position() - body_begin;
        w.insert(body_len as u32, body_len_insertion);
    }
}

#[test]
fn test_marshal() {
    let header = Header {
        message_type: MessageType::Signal,
        flags: Flags(1),
        serial: NonZeroU32::new(0xffffffff).unwrap(),
        fields: Fields {
            ..Default::default()
        }
        .sender("org.freedesktop.DBus")
        .destination(":1.1758")
        .path("/org/freedesktop/DBus")
        .interface("org.freedesktop.DBus")
        .member("NameAcquired")
        .signature("s"),
    };
    dbg!(&header);
    let msg = Message {
        header,
        body: strings::String::from_str(":1.1758"),
    };
    let res = marshal::marshal(&msg);
    dbg!(ShowBytes(&res));
}

#[allow(dead_code)]
struct ShowBytes<'a>(&'a [u8]);

impl core::fmt::Debug for ShowBytes<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for &x in self.0 {
            if x.is_ascii_graphic() {
                write!(f, "{}", x as char)?;
            } else {
                write!(f, "\\{x}")?;
            }
        }
        Ok(())
    }
}
