//! # Ditto
//!
//! Ditto is a library for using [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type),
//! or conflict-free replicated data types. It provides a number
//! of commonly-used data types:
//!
//! **Register\<T\>:** A container for a single value.
//!
//! **Set\<T\>:** A collection of unique values, (like `HashSet`())
//!
//! **Map\<T\>:** A collection of key-value pairs (like `HashMap`)
//!
//! **List\<T\>:** An ordered sequence of elements (like `Vec`)
//!
//! **Text:** A container for mutable text
//!
//! **Json:** A JSON value
//!
//! **Xml:** An XML document
//!
//! Ditto's goal is to be as fast and easy to use as possible. If you have any
//! questions, suggestions, or other feedback, feel free to open an issue
//! or a pull request. Contributions are encouraged!
//!
//! ## Example
//!
//! ```
//! extern crate ditto;
//! extern crate serde_json;
//! use ditto::List;
//!
//! fn main() {
//!     // create a List CRDT
//!     let mut list1 = List::from(vec![100,200,300]);
//!
//!     // Send the list's state over a network to a second site
//!     let encoded_state = serde_json::to_string(&list1.state()).unwrap();
//!     let decoded_state = serde_json::from_str(&encoded_state).unwrap();
//!     let mut list2 = List::from_state(decoded_state, Some(2)).unwrap();
//!
//!     // edit the list concurrently at both the first and second site
//!     let op1 = list1.insert(0, 400).unwrap();
//!     let op2 = list2.remove(0).unwrap();
//!
//!     // each site sends its op to the other site for execution.
//!     // The encoding and decoding has been left out for brevity.
//!     list1.execute_remote(&op2);
//!     list2.execute_remote(&op1);
//!
//!     // Now both sites have the same value:
//!     assert_eq!(list1.state(), list2.state());
//!     assert_eq!(list1.local_value(), vec![400, 200, 300]);
//! }
//! ```
//!
//! You can find more examples in the tests directory.
//!
//! ## Assigning Sites
//!
//! A CRDT may be distributed across multiple *sites*. A site is
//! just a fancy distributed systems term for "client". Each
//! site that wishes to edit the CRDT must have a unique `u32`
//! identifier.
//!
//! The site that creates the CRDT is automatically assigned to
//! id 1. ***You*** are responsible for assigning all other sites;
//! Ditto will not do it for you.
//!
//! There are a number of viable strategies for assigning site
//! identifiers:
//!
//! * If your system has a fixed number of clients each with an id,
//!   you can reuse that ID.
//! * If you have a central server, use that server to allocate site ids.
//! * If you are in a truly distributed environment where nodes are mostly
//!   available, you can use a consensus algorithm like Raft to elect
//!   new site ids.
//!
//! Site IDs can be allocated lazily. If a site only needs read access
//! to a CRDT, it doesn't need a site ID. If a site without an ID edits
//! the CRDT, the CRDT will update locally but all ops will be cached.
//! When the site receives an ID, that ID will be retroactively applied
//! to all of the site's edits, and the cached ops will be returned
//! to be sent over the network.
//!
//! ## Sending ops vs. sending state
//!
//! Usually, the most compact way to send a change between sites
//! is to send an *op*. An op is just a fancy distributed systems
//! term for "change". Each time you edit a CRDT locally, you receive
//! an op that can be sent to other sites and executed.
//!
//! However, there may be times when it is faster and more compact
//! to send the whole CRDT state to other sites (e.g. if you're sending
//! 100 or 1000 edits at once). All Ditto CRDTs can merge with remote states.
//!
//! **Note:** All ops from a site must be sent in the order they were
//! generated. That is, if a site performs edit A and then edit B, it
//! must send op A before it sends op B.
//!
//! ## Serializing CRDTs
//!
//! All CRDTs and ops are serialized with [Serde](https://serde.rs).
//! Serialization is tested against `serde_json` and `rmp_serde`.
//!
//! In general, you should distribute a CRDT to new sites by sending its
//! state, not by sending the CRDT itself, because the CRDT struct contains
//! site-specific metadata.
//!
//! ## Other Notes
//!
//! The root value of a `Json` CRDT cannot be replaced. This means that if
//! you create a `Json` CRDT with a `Number` or `Bool` root type, your
//! CRDT is immutable.
//!
//! CRDTs are inherently larger than their native equivalents. A `Text` or
//! `List` CRDT may use up to 3x the space of an equivalent `String` or
//! `Vec`. If the only operation you need for text is full replacement,
//! consider using `Register<String>` instead.

extern crate base64;
extern crate char_fns;
extern crate either;
#[macro_use] extern crate lazy_static;
extern crate num;
extern crate order_statistic_tree;
extern crate quickxml_dom;
extern crate rand;
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
pub mod xml;

mod error;
mod map_tuple_vec;
mod replica;
mod sequence;
mod vlq;

pub use traits::CrdtRemoteOp;
pub use error::Error;
pub use replica::{Replica, Tombstones};

pub use json::{Json, JsonState};
pub use list::{List, ListState};
pub use map::{Map, MapState};
pub use register::{Register, RegisterState};
pub use set::{Set, SetState};
pub use text::{Text, TextState};
pub use xml::{Xml, XmlState};
