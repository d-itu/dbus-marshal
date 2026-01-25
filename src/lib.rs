#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(
    const_array,
    const_cmp,
    const_convert,
    const_destruct,
    const_index,
    const_option_ops,
    const_trait_impl,
    const_try,
    slice_from_ptr_range,
    str_as_str
)]

use core::fmt::{self, Debug};

pub mod authentication;
pub mod marshal;
pub mod unmarshal;
pub mod signature;

pub use message::*;
pub use strings::*;
pub use types::*;

mod message;
mod strings;
mod types;

const fn aligned(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}

#[allow(dead_code)]
const fn align_padding(size: usize, align: usize) -> usize {
    aligned(size, align) - size
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
