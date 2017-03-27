use serde_json;
use std::num::ParseIntError;

#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    DuplicateUID,
    InvalidIndex,
    InvalidJson,
    InvalidLocalOp,
    InvalidPath,
    InvalidRemoteOp,
    KeyDoesNotExist,
    Noop,
    OutOfBounds,
    UIDDoesNotExist,
    VLQNoTerminatingByte,
}

impl From<ParseIntError> for Error {
    fn from(_: ParseIntError) -> Error {
        Error::InvalidIndex
    }
}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Error {
        Error::InvalidJson
    }
}
