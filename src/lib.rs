#![no_std]

pub mod marshal;
pub mod signature;
pub mod strings;

const fn aligned(size: usize, align: usize) -> usize {
    (size + align - 1) & !(align - 1)
}

