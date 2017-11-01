extern crate quick_xml;

pub mod document;
pub mod element;
pub mod error;

pub use document::{Document, Node};
pub use document::Declaration;
pub use element::Element;
