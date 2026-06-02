use core::result;

use crate::unmarshal::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum IterErr {
    EndOfIteration,
    Error(Error),
}

impl From<Error> for IterErr {
    fn from(value: Error) -> Self {
        IterErr::Error(value)
    }
}

pub type Result<T> = result::Result<T, Error>;
pub(super) type IterResult<T> = result::Result<T, IterErr>;

pub(super) fn flatten<T>(x: IterResult<T>) -> Option<Result<T>> {
    match x {
        Ok(x) => Some(Ok(x)),
        Err(IterErr::Error(e)) => Some(Err(e)),
        Err(IterErr::EndOfIteration) => None,
    }
}
