use std::num::ParseIntError;

#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    InvalidIndex,
    InvalidRemoteOp,
    KeyDoesNotExist,
    Noop,
    OutOfBounds,
    UIDDoesNotExist,
    ValueMismatch(&'static str),
    VLQNoTerminatingByte,
}

impl From<ParseIntError> for Error {
    fn from(_: ParseIntError) -> Error {
        Error::InvalidIndex
    }
}
