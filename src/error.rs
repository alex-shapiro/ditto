use serde_json;
use std::num::ParseIntError;

#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    AlreadyExists,
    AlreadyHasSite,
    AwaitingSite,
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    DoesNotExist,
    DuplicateUID,
    InvalidIndex,
    InvalidJson,
    InvalidLocalOp,
    InvalidPath,
    InvalidPointer,
    InvalidRemoteOp,
    KeyDoesNotExist,
    Noop,
    OutOfBounds,
    UIDDoesNotExist,
    VLQNoTerminatingByte,
    WrongJsonType,
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
