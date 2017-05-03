extern crate char_fns;
#[macro_use] extern crate lazy_static;
extern crate num;
extern crate rand;
extern crate rustc_serialize;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(test)]
extern crate rmp_serde;

#[macro_use]
mod macros;

pub mod map;
pub mod set;

mod array;
mod attributed_string;
mod counter;
mod crdt;
mod error;
mod list;
mod local_value;
mod map_tuple_vec;
mod object;
mod op;
mod register;
mod replica;
mod sequence;
mod serializer;
mod traits;
mod util;
mod value;
mod vlq;

pub use crdt::CRDT;
pub use traits::{Crdt, CrdtValue, CrdtRemoteOp};
pub use error::Error;
pub use replica::Replica;
pub use value::IntoValue;
pub use value::Value;
pub use local_value::LocalValue;
pub use op::NestedLocalOp;
pub use op::NestedRemoteOp;

pub use list::List;
pub use map::Map;
pub use register::{Register, RegisterValue};
pub use set::Set;
