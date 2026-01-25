use core::{
    convert::Infallible,
    fmt::{self, Formatter},
    mem,
    num::NonZeroU32,
};

use crate::{
    marshal::{self, Marshal},
    signature::{Signature, SignatureProxy},
    strings,
    types::{self, Variant},
    unmarshal::{self, Error, Unmarshal},
};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Endian {
    Little = b'l',
    Big = b'B',
}

impl Endian {
    const fn from_u8(x: u8) -> unmarshal::Result<Self> {
        Ok(match x {
            b'l' => Self::Little,
            b'B' => Self::Big,
            _ => Err(Error::InvalidHeader)?,
        })
    }
}

#[cfg(target_endian = "little")]
const NATIVE_ENDIAN: Endian = Endian::Little;

#[cfg(target_endian = "big")]
const NATIVE_ENDIAN: Endian = Endian::Big;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    MethodCall = 1,
    MethodReturn = 2,
    Error = 3,
    Signal = 4,
}

impl MessageType {
    const fn from_u8(x: u8) -> unmarshal::Result<Self> {
        if x < 1 || x > 4 {
            Err(Error::InvalidHeader)?
        }
        Ok(unsafe { mem::transmute(x) })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Flags(pub u8);

impl Flags {
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn with_no_reply_expected(self) -> Self {
        Self(self.0 | 1)
    }
    pub const fn no_reply_expected(self) -> bool {
        self.0 & 1 != 0
    }
    pub const fn with_no_auto_start(self) -> Self {
        Self(self.0 | 2)
    }
    pub const fn no_auto_start(self) -> bool {
        self.0 & 2 != 0
    }
    pub const fn with_allow_interactive_authorization(self) -> Self {
        Self(self.0 | 4)
    }
    pub const fn allow_interactive_authorization(self) -> bool {
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
        #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Fields<'a> {
            $(pub $field: Option<$type>,)*
        }

        impl<'a> Fields<'a> {
            pub const fn empty() -> Self {
                Self {
                    $($field: None,)*
                }
            }
            $(pub const fn $field(self, value: impl [const] Into<$type>) -> Self {
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

        impl<'a> Unmarshal<'a> for Entry<'a> {
            fn unmarshal(r: &mut unmarshal::Reader<'a>) -> unmarshal::Result<Self> {
                let id: u8 = r.read()?;
                match id {
                    $($id => {
                        let value: Variant<$type> = r.read()?;

                        let field = value.0.into();
                        Ok(Entry { id, field })
                    })*
                    _ => Err(Error::InvalidHeader)?,
                }
            }
        }

        impl<'a> Unmarshal<'a> for Fields<'a> {
            fn unmarshal(r: &mut unmarshal::Reader<'a>) -> unmarshal::Result<Self> {
                let mut result = Self::empty();
                let iter: unmarshal::ArrayIter<Entry> = r.read()?;
                for x in iter {
                    let Entry { id, field } = x?;
                    match id {
                        $($id => {
                            result = result.$field(field);
                        })*
                        _ => {}
                    }
                }
                Ok(result)
            }
        }
    };
}

macro_rules! define_field {
    ($($name:ident: $type:ty),* $(,)?) => {
        union Field<'a> {
            $($name: $type,)*
        }

        $(
            impl<'a> From<$type> for Field<'a> {
                fn from(value: $type) -> Self {
                    Self { $name: value }
                }
            }
            impl<'a> Into<$type> for Field<'a> {
                fn into(self) -> $type {
                    unsafe { self.$name }
                }
            }
        )*
    };
}

define_field!(
    object: &'a strings::ObjectPath,
    string: &'a strings::String,
    signature: &'a strings::Signature,
    u32: u32,
);

struct Entry<'a> {
    id: u8,
    field: Field<'a>,
}

impl SignatureProxy for Entry<'_> {
    type Proxy = types::Entry<u8, types::Variant<Infallible>>;
}

unsafe impl Signature for Entry<'_> {
    const ALIGNMENT: usize = 8;
}

define_fields! {
    1 path: &'a strings::ObjectPath,
    2 interface: &'a strings::String,
    3 member: &'a strings::String,
    4 error_name: &'a strings::String,
    5 reply_serial: u32,
    6 destination: &'a strings::String,
    7 sender: &'a strings::String,
    8 signature: &'a strings::Signature,
    9 unix_fds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header<'a> {
    pub message_type: MessageType,
    pub flags: Flags,
    pub serial: NonZeroU32,
    pub fields: Fields<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message<'a, T> {
    pub header: Header<'a>,
    pub body: T,
}

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

impl<'a> Unmarshal<'a> for Message<'a, &'a [u8]> {
    fn unmarshal(r: &mut unmarshal::Reader<'a>) -> unmarshal::Result<Self> {
        let endian = r.read_byte().and_then(Endian::from_u8)?;
        if endian != NATIVE_ENDIAN {
            Err(Error::UnsupportedEndian)?
        }
        let message_type = r.read_byte().and_then(MessageType::from_u8)?;
        let flags = r.read_byte().map(Flags)?;
        let _version = r.read_byte()?;
        let body_len: u32 = r.read()?;
        let serial = r.read()?;
        let serial = NonZeroU32::new(serial).ok_or(Error::InvalidHeader)?;
        let fields = r.read()?;
        let header = Header {
            message_type,
            flags,
            serial,
            fields,
        };
        r.align_to(8)?;
        let body_len = body_len as usize;
        let body = r.remaining().get(..body_len).ok_or(Error::NotEnoughData)?;
        r.seek(body_len)?;
        Ok(Self { header, body })
    }
}

impl<'a> Message<'a, &'a [u8]> {
    pub fn from_bytes(data: &'a [u8]) -> unmarshal::Result<Self> {
        Self::unmarshal(&mut unmarshal::Reader::new(data))
    }
}

pub struct MessageIterator<'a> {
    reader: unmarshal::Reader<'a>,
}

impl<'a> MessageIterator<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            reader: unmarshal::Reader::new(data),
        }
    }
    pub fn next(&mut self) -> Option<unmarshal::Result<Message<'a, &'a [u8]>>> {
        if self.reader.remaining().is_empty() {
            None?;
        }
        match self.reader.read() {
            Ok(x) => {
                self.reader = unmarshal::Reader::new(self.reader.remaining());
                Some(Ok(x))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl<'a> Iterator for MessageIterator<'a> {
    type Item = unmarshal::Result<Message<'a, &'a [u8]>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

#[cfg(test)]
const fn test_header() -> Header<'static> {
    Header {
        message_type: MessageType::Signal,
        flags: Flags(1),
        serial: NonZeroU32::new(0xffffffff).unwrap(),
        fields: Fields::empty()
            .sender("org.freedesktop.DBus")
            .destination(":1.1758")
            .path("/org/freedesktop/DBus")
            .interface("org.freedesktop.DBus")
            .member("NameAcquired")
            .signature("s"),
    }
}

#[test]
fn test_marshal() {
    let header = test_header();
    dbg!(&header);
    let msg = Message {
        header,
        body: strings::String::from_str(":1.1758"),
    };
    let res = marshal::marshal(&msg);
    dbg!(crate::show_bytes(&res));
}

#[test]
fn test_unmarshal() {
    let header = test_header();
    let msg = Message {
        header,
        body: strings::String::from_str(":1.1758"),
    };
    let res = marshal::marshal(&msg);
    let mut iter = MessageIterator::new(&res);
    let msg = iter.next().unwrap().unwrap();
    assert_eq!(msg.header, header);
}
