extern crate char_fns;
#[macro_use] extern crate lazy_static;
extern crate num;
extern crate rand;
extern crate rustc_serialize;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate serde_json;

#[cfg(not(test))]
extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(test)]
extern crate rmp_serde;

#[macro_use] mod macros;
#[macro_use] mod traits;

pub mod json;
pub mod list;
pub mod map;
pub mod register;
pub mod set;
pub mod text;

mod error;
mod map_tuple_vec;
mod replica;
mod sequence;
mod vlq;

pub use traits::{CrdtValue, CrdtRemoteOp};
pub use error::Error;
pub use replica::{Replica, Tombstones};

pub use json::{Json, JsonState};
pub use list::{List, ListState};
pub use map::{Map, MapState};
pub use register::{Register, RegisterState};
pub use set::{Set, SetState};
pub use text::{Text, TextState};
