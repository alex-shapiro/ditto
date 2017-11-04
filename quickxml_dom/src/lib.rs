extern crate quick_xml;

pub mod document;
pub mod element;
pub mod error;

pub use document::{Document, Declaration, Node, XmlVersion};
pub use element::{Element, Child};
pub use error::Error;
