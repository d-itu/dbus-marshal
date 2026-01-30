use core::num::NonZeroU32;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Serial(u32);

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Serial {
    pub const fn from_raw(value: u32) -> Self {
        Self(value)
    }
    pub const fn new() -> Self {
        Self(0)
    }
    fn get(&self) -> NonZeroU32 {
        unsafe { NonZeroU32::new_unchecked(self.0) }
    }
    fn next(&mut self) -> NonZeroU32 {
        self.0 += 1;
        self.get()
    }

    pub fn method_call<'a, T: Marshal + MultiSignature>(
        &mut self,
        flags: Flags,
        proxy: Proxy<'_>,
        member: impl Into<&'a strings::String>,
        arguments: T,
    ) -> Box<[u8]> {
        let sig = T::DATA;
        let signature = sig.signature();
        let fields = Fields {
            signature: if signature.is_empty() {
                None
            } else {
                Some(signature)
            },
            member: Some(member.into()),
            ..proxy.fields()
        };
        marshal::marshal(&Message {
            header: Header {
                message_type: MessageType::MethodCall,
                flags,
                serial: self.next(),
                fields,
            },
            arguments,
        })
    }

    pub fn method_return<T: Marshal + MultiSignature>(
        &mut self,
        method_call: &Header,
        arguments: T,
    ) -> Box<[u8]> {
        let sig = T::DATA;
        let signature = sig.signature();
        let fields = Fields {
            signature: if signature.is_empty() {
                None
            } else {
                Some(signature)
            },
            reply_serial: Some(method_call.serial.get()),
            destination: method_call.fields.sender,
            ..Fields::empty()
        };
        marshal::marshal(&Message {
            header: Header {
                message_type: MessageType::MethodReturn,
                flags: Flags::empty(),
                serial: self.next(),
                fields,
            },
            arguments,
        })
    }

    pub fn error<'a, T: Marshal + MultiSignature>(
        &mut self,
        name: impl Into<&'a strings::String>,
        method_call: &Header,
        arguments: T,
    ) -> Box<[u8]> {
        let sig = T::DATA;
        let signature = sig.signature();
        let fields = Fields {
            signature: if signature.is_empty() {
                None
            } else {
                Some(signature)
            },
            error_name: Some(name.into()),
            reply_serial: Some(method_call.serial.get()),
            destination: method_call.fields.sender,
            ..Fields::empty()
        };
        marshal::marshal(&Message {
            header: Header {
                message_type: MessageType::Error,
                flags: Flags::empty(),
                serial: self.next(),
                fields,
            },
            arguments,
        })
    }

    pub fn signal<'a, 'b, 'c, T: Marshal + MultiSignature>(
        &mut self,
        path: impl Into<&'a strings::ObjectPath>,
        interface: impl Into<&'b strings::String>,
        member: impl Into<&'c strings::String>,
        arguments: T,
    ) -> Box<[u8]> {
        let sig = T::DATA;
        let signature = sig.signature();
        let fields = Fields {
            signature: if signature.is_empty() {
                None
            } else {
                Some(signature)
            },
            path: Some(path.into()),
            interface: Some(interface.into()),
            member: Some(member.into()),
            ..Fields::empty()
        };
        marshal::marshal(&Message {
            header: Header {
                message_type: MessageType::Signal,
                flags: Flags::empty(),
                serial: self.next(),
                fields,
            },
            arguments,
        })
    }
}
