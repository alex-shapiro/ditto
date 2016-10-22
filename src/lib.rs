#![feature(proc_macro)]

extern crate num;
extern crate rand;
extern crate rustc_serialize;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod array;
mod attributed_string;
mod crdt;
mod object;
mod op;
mod raw;
mod replica;
mod sequence;
mod value;
mod vlq;

pub use crdt::CRDT;
pub use replica::Replica;
pub use value::Value;
