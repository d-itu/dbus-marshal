#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(
    const_convert,
    const_destruct,
    const_index,
    const_trait_impl,
    const_try,
    slice_from_ptr_range,
    str_as_str
)]

pub mod marshal;
pub mod unmarshal;

use core::{
    fmt::{self, Debug},
    mem::MaybeUninit,
};

pub use message::*;
pub use strings::*;
pub use types::*;

mod message;
mod signature;
mod strings;
mod types;

const fn aligned(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}

#[allow(dead_code)]
const fn align_padding(size: usize, align: usize) -> usize {
    aligned(size, align) - size
}

struct ArrayVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: u8,
}

impl<T, const N: usize> ArrayVec<T, N> {
    pub const fn new() -> Self {
        Self {
            data: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }
    pub const fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.len < N as u8 {
            unsafe {
                self.data.get_unchecked_mut(self.len as usize).write(value);
            }
            self.len += 1;
            return Ok(());
        }
        Err(value)
    }
    pub const fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        self.len -= 1;
        Some(unsafe {
            self.data
                .get_unchecked(self.len as usize)
                .assume_init_read()
        })
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub const fn last(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        Some(unsafe {
            self.data
                .get_unchecked(self.len as usize - 1)
                .assume_init_ref()
        })
    }
    pub const fn last_mut(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            return None;
        }
        Some(unsafe {
            self.data
                .get_unchecked_mut(self.len as usize - 1)
                .assume_init_mut()
        })
    }
}

#[allow(dead_code)]
fn show_bytes(xs: &[u8]) -> impl Debug {
    fmt::from_fn(move |f| {
        Ok(for &x in xs {
            if x.is_ascii_graphic() {
                write!(f, "{}", x as char)?;
            } else {
                write!(f, "\\{x}")?;
            }
        })
    })
}
