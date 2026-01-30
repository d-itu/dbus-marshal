#[cfg(feature = "alloc")]
use alloc::{borrow::ToOwned, boxed::Box};
use core::{
    convert::Infallible,
    fmt::{self, Formatter},
    mem,
    num::NonZeroU32,
};

use crate::{
    marshal::{self, Marshal},
    signature::{MultiSignature, Node as _, SignatureProxy},
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
    (@ref (ref $type:ty)) => {
        &'a $type
    };
    (@ref $type:ty) => {
        $type
    };
    (@owned (ref $type:ty)) => {
        Box<$type>
    };
    (@owned $type:ty) => {
        $type
    };
    (@to_owned $field:ident (ref $type:ty)) => {
        &**$field
    };
    (@to_owned $field:ident $type:ty) => {
        *$field
    };
    ($($id:literal $field:ident: $type:tt),* $(,)?) => {
        #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Fields<'a> {
            $(pub $field: Option<define_fields!(@ref $type)>,)*
        }

        #[cfg(feature = "alloc")]
        #[derive(Default, Debug, PartialEq, Eq)]
        pub struct OwnedFields {
            $(pub $field: Option<define_fields!(@owned $type)>,)*
        }

        #[cfg(feature = "alloc")]
        impl OwnedFields {
            pub fn as_ref(&self) -> Fields<'_> {
                Fields {
                    $($field: self.$field.as_ref().map(|x| define_fields!(@to_owned x $type)),)*
                }
            }
        }

        impl<'a> Fields<'a> {
            #[cfg(feature = "alloc")]
            pub fn to_owned(&self) -> OwnedFields {
                OwnedFields {
                    $($field: self.$field.map(|x| x.to_owned()),)*
                }
            }
            pub const fn empty() -> Self {
                Self {
                    $($field: None,)*
                }
            }
            $(pub const fn $field(self, value: impl [const] Into<define_fields!(@ref $type)>) -> Self {
                Self {
                    $field: Some(value.into()),
                    ..self
                }
            })*
        }

        impl Marshal for &Fields<'_> {
            fn marshal<W: marshal::Write + ?Sized>(self, w: &mut W) {
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
                        let value: Variant<define_fields!(@ref $type)> = r.read()?;

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

define_fields! {
    1 path: (ref strings::ObjectPath),
    2 interface: (ref strings::String),
    3 member: (ref strings::String),
    4 error_name: (ref strings::String),
    5 reply_serial: u32,
    6 destination: (ref strings::String),
    7 sender: (ref strings::String),
    8 signature: (ref strings::Signature),
    9 unix_fds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header<'a> {
    pub message_type: MessageType,
    pub flags: Flags,
    pub serial: NonZeroU32,
    pub fields: Fields<'a>,
}

#[cfg(feature = "alloc")]
impl Header<'_> {
    pub fn to_owned(&self) -> OwnedHeader {
        OwnedHeader {
            message_type: self.message_type,
            flags: self.flags,
            serial: self.serial,
            fields: self.fields.to_owned(),
        }
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, PartialEq, Eq)]
pub struct OwnedHeader {
    pub message_type: MessageType,
    pub flags: Flags,
    pub serial: NonZeroU32,
    pub fields: OwnedFields,
}

#[cfg(feature = "alloc")]
impl OwnedHeader {
    pub fn as_ref(&self) -> Header<'_> {
        Header {
            message_type: self.message_type,
            flags: self.flags,
            serial: self.serial,
            fields: self.fields.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message<'a, T> {
    pub header: Header<'a>,
    pub arguments: T,
}

impl<'a> Message<'a, &'a [u8]> {
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> OwnedMessage<Box<[u8]>> {
        OwnedMessage {
            header: self.header.to_owned(),
            arguments: self.arguments.to_owned().into(),
        }
    }
    pub fn parse<T: Unmarshal<'a> + MultiSignature>(&self) -> unmarshal::Result<T> {
        let signature = self
            .header
            .fields
            .signature
            .unwrap_or(&strings::Signature::from_bytes(b""));
        if signature != T::DATA.signature() {
            Err(Error::UnexpectedType)?
        }
        let mut reader = unmarshal::Reader::new(self.arguments);
        reader.read()
    }
}

#[cfg(feature = "alloc")]
#[derive(Debug, PartialEq, Eq)]
pub struct OwnedMessage<T> {
    pub header: OwnedHeader,
    pub arguments: T,
}

#[cfg(feature = "alloc")]
impl OwnedMessage<Box<[u8]>> {
    pub fn as_ref(&self) -> Message<'_, &[u8]> {
        Message {
            header: self.header.as_ref(),
            arguments: &self.arguments,
        }
    }
}

impl<T: Marshal> Marshal for &Message<'_, T> {
    fn marshal<W: marshal::Write + ?Sized>(self, w: &mut W) {
        let Message { header, arguments } = self;
        w.write_byte(NATIVE_ENDIAN as _);
        w.write_byte(header.message_type as _);
        w.write_byte(header.flags.0);
        w.write_byte(1);
        let args_len_insertion = w.position();
        w.seek(4);
        w.write(header.serial);

        let header_len_insertion = w.position();
        w.seek(4);
        w.align_to(8);
        w.write(&header.fields);
        let header_len = w.position() - 16;
        w.insert(header_len as u32, header_len_insertion);
        w.align_to(8);

        let args_begin = w.position();
        arguments.marshal(w);
        let args_len = w.position() - args_begin;
        w.insert(args_len as u32, args_len_insertion);
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
        let args_len: u32 = r.read()?;
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
        let args_len = args_len as usize;
        let args = r.remaining().get(..args_len).ok_or(Error::NotEnoughData)?;
        r.seek(args_len)?;
        Ok(Self {
            header,
            arguments: args,
        })
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
        arguments: strings::String::from_str(":1.1758"),
    };
    let res = marshal::marshal(&msg);
    dbg!(crate::show_bytes(&res));
}

#[test]
fn test_unmarshal() {
    let header = test_header();
    let msg = Message {
        header,
        arguments: strings::String::from_str(":1.1758"),
    };
    let size = marshal::calc_size(&msg);
    let mut buf = Box::new_zeroed_slice(size * 2);
    let (_, remaining) = marshal::write(&msg, &mut buf).unwrap();
    marshal::write(&msg, remaining).unwrap();

    let buf = unsafe { buf.assume_init() };
    let mut iter = MessageIterator::new(&buf);
    let msg = iter.next().unwrap().unwrap();
    assert_eq!(msg.header, header);
    let msg = iter.next().unwrap().unwrap();
    assert_eq!(msg.header, header);
    assert_eq!(iter.next(), None);
}

#[derive(Clone, Copy)]
pub struct Proxy<'a> {
    pub destination: &'a strings::String,
    pub path: &'a strings::ObjectPath,
    pub interface: &'a strings::String,
}

impl<'a> Proxy<'a> {
    fn fields(&self) -> Fields<'a> {
        Fields::empty()
            .destination(self.destination)
            .path(self.path)
            .interface(self.interface)
    }
}

#[cfg(feature = "alloc")]
pub use serial::Serial;
#[cfg(feature = "alloc")]
mod serial;
