use core::{marker::PhantomData, mem, result, slice};

use crate::{
    ArrayVec, strings,
    unmarshal::{Error, Reader},
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum IterErr {
    EndOfIteration,
    Error(Error),
}

impl From<Error> for IterErr {
    fn from(value: Error) -> Self {
        IterErr::Error(value)
    }
}

pub type Result<T> = result::Result<T, Error>;
type IterResult<T> = result::Result<T, IterErr>;

#[derive(Debug)]
pub enum Token<'a> {
    U8(u8),
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    String(&'a strings::String),
    Object(&'a strings::ObjectPath),
    Signature(&'a strings::Signature),
    Array {
        signature: &'a [u8],
        /// begin of element, after length and first align padding
        data: &'a [u8],
    },
    VariantOpen,
    VariantClose,
    StructOpen,
    StructClose,
    EntryOpen,
    EntryClose,
}

macro_rules! define_token_kind {
    ($($name:ident = $value:literal),* $(,)?) => {
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy, PartialEq)]
        #[repr(u8)]
        enum TokenKind {
            $($name = $value,)*
        }

        impl TokenKind {
            #[allow(dead_code)]
            const fn validate(byte: u8) -> Result<Self> {
                const MASK: u128 = $(1 << $value)|*;
                if MASK & (1 << byte) != 0 {
                    Ok(unsafe { mem::transmute(byte) })
                } else {
                    Err(Error::SignatureInvalidChar)
                }
            }
        }
    };
}

define_token_kind! {
    U8 = b'y',
    Bool = b'b',
    I16 = b'n',
    U16 = b'q',
    I32 = b'i',
    U32 = b'u',
    I64 = b'x',
    U64 = b't',
    F64 = b'd',
    String = b's',
    Object = b'o',
    Signature = b'g',
    Variant = b'v',
    Array = b'a',
    StructOpen = b'(',
    StructClose = b')',
    EntryOpen = b'{',
    EntryClose = b'}',
}

impl TokenKind {
    #[allow(dead_code)]
    fn alignment(self) -> usize {
        match self {
            Self::U8 | Self::Signature | Self::Variant => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::Bool | Self::String | Self::Object | Self::Array => 4,
            Self::I64 | Self::U64 | Self::F64 | Self::StructOpen | Self::EntryOpen => 8,
            Self::StructClose | Self::EntryClose => unreachable!(),
        }
    }
}

enum Nesting<'a> {
    Array(*const u8),
    Struct,
    Entry(u8),
    Variant(SignatureIter<'a>),
}

const MAX_NESTING: usize = 32;

struct SignatureIter<'a> {
    ptr: *const u8,
    end: *const u8,
    marker: PhantomData<&'a [u8]>,
}

#[derive(PartialEq, Debug)]
struct SignatureToken<'a> {
    kind: TokenKind,
    payload: &'a [u8],
}

impl<'a> SignatureIter<'a> {
    fn next_byte(&mut self, stack: &mut NestingStack) -> IterResult<&'a u8> {
        if self.ptr == self.end {
            match stack.last() {
                None | Some(Nesting::Variant(_)) => Err(IterErr::EndOfIteration)?,
                _ => Err(Error::NestingMismatched)?,
            }
        }
        let byte = unsafe { &*self.ptr };
        self.ptr = unsafe { self.ptr.add(1) };
        Ok(byte)
    }
    fn close_array(
        &mut self,
        ptr: *const u8,
        stack: &mut NestingStack,
        array_depth: &mut usize,
    ) -> IterResult<SignatureToken<'a>> {
        match stack.last_mut() {
            Some(&mut Nesting::Array(ptr)) => {
                stack.pop();
                *array_depth -= 1;
                return self.close_array(ptr, stack, array_depth);
            }
            Some(Nesting::Struct) => {
                if *array_depth != 0 {
                    return self.next(stack, array_depth);
                }
            }
            Some(Nesting::Entry(x)) => {
                if *x == 2 {
                    Err(Error::InvalidEntrySize)?
                }
                *x += 1;
                if *array_depth != 0 {
                    return self.next(stack, array_depth);
                }
            }
            None | Some(Nesting::Variant(_)) => {}
        }
        Ok(SignatureToken {
            kind: TokenKind::Array,
            payload: unsafe { slice::from_ptr_range(ptr.add(1)..self.ptr) },
        })
    }
    fn at_value(
        &mut self,
        byte: u8,
        stack: &mut NestingStack,
        array_depth: &mut usize,
    ) -> IterResult<SignatureToken<'a>> {
        if let Some(x) = stack.last_mut() {
            match x {
                &mut Nesting::Array(ptr) => {
                    stack.pop();
                    *array_depth -= 1;
                    return self.close_array(ptr, stack, array_depth);
                }
                Nesting::Entry(depth) => {
                    if *depth != 2 {
                        *depth += 1;
                    } else {
                        Err(Error::InvalidEntrySize)?
                    }
                }
                _ => {}
            }
        };
        if *array_depth == 0 {
            let kind = unsafe { mem::transmute(byte) };
            Ok(SignatureToken { kind, payload: &[] })
        } else {
            return self.next(stack, array_depth);
        }
    }
    fn next(
        &mut self,
        stack: &mut NestingStack,
        array_depth: &mut usize,
    ) -> IterResult<SignatureToken<'a>> {
        let byte = self.next_byte(stack)?;
        match byte {
            b'y' | b'b' | b'n' | b'q' | b'i' | b'u' | b'x' | b't' | b'd' | b's' | b'o' | b'g'
            | b'v' => self.at_value(*byte, stack, array_depth),
            b'a' => {
                *array_depth += 1;
                stack
                    .try_push(Nesting::Array(byte))
                    .map_err(|_| Error::NestingDepthExceeded)?;
                self.next(stack, array_depth)
            }
            b'{' => {
                stack
                    .try_push(Nesting::Entry(0))
                    .map_err(|_| Error::NestingDepthExceeded)?;
                if *array_depth != 0 {
                    return self.next(stack, array_depth);
                }
                Ok(SignatureToken {
                    kind: TokenKind::EntryOpen,
                    payload: &[],
                })
            }
            b'(' => {
                stack
                    .try_push(Nesting::Struct)
                    .map_err(|_| Error::NestingDepthExceeded)?;
                if *array_depth != 0 {
                    return self.next(stack, array_depth);
                }
                Ok(SignatureToken {
                    kind: TokenKind::StructOpen,
                    payload: &[],
                })
            }
            b'}' => match stack.pop() {
                Some(Nesting::Entry(2)) => self.at_value(*byte, stack, array_depth),
                Some(Nesting::Entry(_)) => Err(Error::InvalidEntrySize)?,
                _ => Err(Error::NestingMismatched)?,
            },
            b')' => match stack.pop() {
                Some(Nesting::Struct) => self.at_value(*byte, stack, array_depth),
                _ => Err(Error::NestingMismatched)?,
            },
            _ => Err(Error::SignatureInvalidChar)?,
        }
    }
    fn new(data: &'a [u8]) -> Self {
        Self {
            ptr: data.as_ptr(),
            end: unsafe { data.as_ptr().add(data.len()) },
            marker: PhantomData,
        }
    }
}

type NestingStack<'a> = ArrayVec<Nesting<'a>, MAX_NESTING>;

pub struct Iter<'a> {
    reader: Reader<'a>,
    signature: SignatureIter<'a>,
    nesting_stack: NestingStack<'a>,
    array_depth: usize,
}

impl<'a> Iter<'a> {
    pub fn new(signature: &'a [u8], data: &'a [u8]) -> Self {
        Self {
            reader: Reader::new(data),
            signature: SignatureIter::new(signature),
            nesting_stack: ArrayVec::new(),
            array_depth: 0,
        }
    }
    fn iter(&mut self) -> IterResult<Token<'a>> {
        let SignatureToken { kind, payload } = {
            if let Ok(x) = self
                .signature
                .next(&mut self.nesting_stack, &mut self.array_depth)
            {
                x
            } else {
                self.signature = match self.nesting_stack.pop().ok_or(IterErr::EndOfIteration)? {
                    Nesting::Variant(x) => x,
                    _ => unreachable!(),
                };
                return Ok(Token::VariantClose);
            }
        };
        debug_assert_eq!(self.array_depth, 0);
        Ok(match kind {
            TokenKind::U8 => Token::U8(self.reader.read::<u8>()?),
            TokenKind::Bool => Token::Bool(self.reader.read::<bool>()?),
            TokenKind::I16 => Token::I16(self.reader.read::<i16>()?),
            TokenKind::U16 => Token::U16(self.reader.read::<u16>()?),
            TokenKind::I32 => Token::I32(self.reader.read::<i32>()?),
            TokenKind::U32 => Token::U32(self.reader.read::<u32>()?),
            TokenKind::I64 => Token::I64(self.reader.read::<i64>()?),
            TokenKind::U64 => Token::U64(self.reader.read::<u64>()?),
            TokenKind::F64 => Token::F64(self.reader.read::<f64>()?),
            TokenKind::String => Token::String(self.reader.read::<&strings::String>()?),
            TokenKind::Object => Token::Object(self.reader.read::<&strings::ObjectPath>()?),
            TokenKind::Signature => Token::Signature(self.reader.read::<&strings::Signature>()?),
            TokenKind::Array => {
                let len = self.reader.read::<u32>()? as usize;
                if let TokenKind::U64
                | TokenKind::I64
                | TokenKind::F64
                | TokenKind::StructOpen
                | TokenKind::EntryOpen = unsafe { mem::transmute(*payload.get_unchecked(0)) }
                {
                    self.reader.seek(4)?; // align to 8
                };
                Token::Array {
                    signature: payload,
                    data: self.reader.read_bytes(len)?,
                }
            }
            TokenKind::StructOpen => {
                self.reader.align_to(8)?;
                Token::StructOpen
            },
            TokenKind::EntryOpen => {
                self.reader.align_to(8)?;
                Token::EntryOpen
            },
            TokenKind::StructClose => Token::StructClose,
            TokenKind::EntryClose => Token::EntryClose,
            TokenKind::Variant => {
                let sig = self.reader.read::<&strings::Signature>()?;
                let mut sig = SignatureIter::new(sig);
                mem::swap(&mut sig, &mut self.signature);
                self.nesting_stack
                    .try_push(Nesting::Variant(sig))
                    .map_err(|_| Error::NestingDepthExceeded)?;
                Token::VariantOpen
            }
        })
    }
    pub fn next(&mut self) -> Option<Result<Token<'a>>> {
        match self.iter() {
            Ok(x) => Some(Ok(x)),
            Err(IterErr::Error(e)) => Some(Err(e)),
            Err(IterErr::EndOfIteration) => None,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Result<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next()
    }
}

// struct ArrayIter<'a> {
//     signature: &'a [u8],
//     data: &'a [u8],
// }
//
// impl<'a> Iterator for ArrayIter<'a> {
//     type Item = Option<Iter<'a>>;
//     fn next(&mut self) -> Option<Self::Item> {
//         let
//     }
// }

#[test]
fn test_iter() {
    let data = [
        34, 0, 0, 0, 0, 0, 0, 0, 1, 1, 121, 0, 1, 0, 0, 0, 2, 1, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 3, 4, 40, 121, 121, 41, 0, 0, 2, 4,
    ];
    let mut it = Iter::new(b"a{yv}", &data);
    while let Some(x) = it.next() {
        match x.unwrap() {
            Token::Array {
                signature,
                mut data,
            } => {
                let mut padding = 0;
                let sig: TokenKind = unsafe { mem::transmute(*signature.get_unchecked(0)) };
                let align = sig.alignment();
                loop {
                    data = &data[padding..];
                    let mut it = Iter::new(signature, data);
                    dbg!(data);
                    while let Some(x) = it.next() {
                        dbg!(x.unwrap());
                    }
                    data = it.reader.rest_bytes();
                    padding = crate::align_padding(it.reader.count, align);
                    if data.is_empty() {
                        break;
                    }
                }
            }
            x => {
                dbg!(x);
            }
        }
    }
    // panic!()
}

#[test]
fn test_signatre_parse() {
    let mut iter = SignatureIter::new(b"aii");
    let mut stack = ArrayVec::new();
    let mut depth = 0;
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::Array,
            payload: b"i"
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::I32,
            payload: &[],
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Err(IterErr::EndOfIteration)
    );
    assert!(stack.is_empty());
    assert_eq!(depth, 0);

    let mut iter = SignatureIter::new(b"((ai))");
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::StructOpen,
            payload: &[],
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::StructOpen,
            payload: &[],
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::Array,
            payload: b"i"
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::StructClose,
            payload: &[],
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::StructClose,
            payload: &[],
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Err(IterErr::EndOfIteration)
    );

    let mut iter = SignatureIter::new(b"a(aii)");
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::Array,
            payload: b"(aii)"
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Err(IterErr::EndOfIteration)
    );
    assert!(stack.is_empty());
    assert_eq!(depth, 0);

    let mut iter = SignatureIter::new(b"aai");
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Ok(SignatureToken {
            kind: TokenKind::Array,
            payload: b"ai"
        })
    );
    assert_eq!(
        iter.next(&mut stack, &mut depth),
        Err(IterErr::EndOfIteration)
    );
    assert!(stack.is_empty());
    assert_eq!(depth, 0);

    let mut iter = SignatureIter::new(b"(a{ai(ai)}ai)");
    loop {
        match iter.next(&mut stack, &mut depth) {
            Ok(x) => {
                dbg!(x);
            }
            Err(e) => {
                dbg!(&e);
                if e != IterErr::EndOfIteration {
                    panic!()
                }
                break;
            }
        }
    }
    assert!(stack.is_empty());
    assert_eq!(depth, 0);
}
