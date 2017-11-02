extern crate quick_xml;

pub mod document;
pub mod element;
pub mod error;

pub use document::{Document, Declaration, Node};
pub use element::{Element, Child};
