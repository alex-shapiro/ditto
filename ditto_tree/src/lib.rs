#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#[cfg(test)]
extern crate rand;
extern crate serde;

mod tree;

pub use tree::Tree;
pub use tree::Element;
pub use tree::Error;
pub use tree::Iter;
