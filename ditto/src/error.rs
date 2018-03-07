use serde_json;
use std::num::ParseIntError;

#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    AlreadyExists,
    AlreadyHasSite,
    AlreadyHasSiteId,
    AwaitingSite,
    AwaitingSiteId,
    CannotMerge,
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    DoesNotExist,
    DuplicateUID,
    InvalidIndex,
    InvalidJson,
    InvalidLocalOp,
    InvalidOp,
    InvalidPointer,
    InvalidRemoteOp,
    InvalidSite,
    InvalidSiteId,
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

impl From<::order_statistic_tree::Error> for Error {
    fn from(err: ::order_statistic_tree::Error) -> Error {
        match err {
            ::order_statistic_tree::Error::OutOfBounds => Error::OutOfBounds,
            ::order_statistic_tree::Error::DuplicateId => Error::DuplicateUID,
        }
    }
}
