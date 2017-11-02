use quick_xml;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum Error {
    BadWrite,
    InvalidEncoding(Utf8Error),
    InvalidPointer,
    InvalidXml,
    QuickXml(quick_xml::errors::Error),
}

impl From<quick_xml::errors::Error> for Error {
    fn from(err: quick_xml::errors::Error) -> Self {
        Error::QuickXml(err)
    }
}

impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        Error::InvalidEncoding(err)
    }
}

impl From<::std::string::FromUtf8Error> for Error {
    fn from(err: ::std::string::FromUtf8Error) -> Self {
        err.utf8_error().into()
    }
}
