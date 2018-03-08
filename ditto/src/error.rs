use serde_json;
use std::num::ParseIntError;

#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    AlreadyHasSiteId,
    AwaitingSiteId,
    CannotMerge,
    DeserializeSequenceUid,
    DoesNotExist,
    DuplicateUid,
    InvalidIndex,
    InvalidJson,
    InvalidLocalOp,
    InvalidOp,
    InvalidPointer,
    InvalidSiteId,
    KeyDoesNotExist,
    Noop,
    OutOfBounds,
    UidDoesNotExist,
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

impl From<::ditto_tree::Error> for Error {
    fn from(err: ::ditto_tree::Error) -> Error {
        match err {
            ::ditto_tree::Error::OutOfBounds => Error::OutOfBounds,
            ::ditto_tree::Error::DuplicateId => Error::DuplicateUid,
        }
    }
}
