//! # Ditto
//!
//! Ditto is a library for using [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type),
//! or conflict-free replicated data types. CRDTs are data structures
//! which can be replicated across multiple sites, edited concurrently,
//! and merged together without leading to conflicts. Ditto provides
//! a number of commonly used data types:
//!
//! * **[Register\<T\>](register/Register.t.html):** A replaceable value
//! * **[Counter](counter/Counter.t.html):** An i64 value that increments
//! * **[Set\<T\>](set/Set.t.html):** A HashSet-like collection of unique values
//! * **[Map\<K, V\>:](map/Map.t.html)** A HashMap-like collection of key-value pairs
//! * **[List\<T\>:](list/List.t.html)** A Vec-like ordered sequence of elements
//! * **[Text:](text/Text.t.html)** A String-like container for mutable text
//! * **[Json:](json/Json.t.html)** A JSON value
//! * **[Xml:](xml/Xml.t.html)** An XML document
//!
//! Ditto's goal is to be as fast and easy to use as possible. If you have any
//! questions, suggestions, or other feedback, feel free to open an issue
//! or a pull request or contact the Ditto developers directly.
//!
//! ## Example
//!
//! ```rust
//! extern crate ditto;
//! extern crate serde_json;
//! use ditto::List;
//!
//! fn main() {
//!     // Create a List CRDT. The site that creates the CRDT
//!     // is automatically assigned id 1.
//!     let mut list1 = List::from(vec![100,200,300]);
//!
//!     // Send the list's state over a network to a second site with id 2.
//!     let encoded_state = serde_json::to_string(&list1.state()).unwrap();
//!     let decoded_state = serde_json::from_str(&encoded_state).unwrap();
//!     let mut list2 = List::from_state(decoded_state, Some(2)).unwrap();
//!
//!     // Edit the list concurrently at both the first and second site.
//!     // Whenever you edit a CRDT, you receive an op that can be sent
//!     // to other sites.
//!     let op1 = list1.insert(0, 400).unwrap();
//!     let op2 = list2.remove(0).unwrap();
//!
//!     // Each site sends its op to the other site for execution.
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
//! You can find more examples in the examples and tests directories in the
//! crate repo.
//!
//! ## Using CRDTs
//!
//! Ditto CRDTs are designed to mimic standard data type APIs as much
//! as possible. You can `insert` and `remove` list and map
//! elements, `replace` text elements, etc. Each edit generates an op
//! that can be sent to other sites for execution. When you execute an
//! op sent from another site, you receive a `LocalOp` that shows
//! exactly how the CRDT's value has changed.
//!
//! The two complications of CRDTs that users have to worry about are:
//!
//!   * How to send ops/state from one site to another
//!   * How to assign a site id to each site.
//!
//! Sending changes and assigning sites are covered in the sections below.
//!
//! ### Sending ops and state
//!
//! CRDTs and ops are serializable with [Serde](https://serde.rs).
//! Serialization is tested against [`serde_json`](https://github.com/serde-rs/json)
//! (JSON) and [`rmp-serde`](https://github.com/3Hren/msgpack-rust)
//! (MsgPack) but may work with other formats as well.
//!
//! Ops must be sent in the order they were generated. That is, if
//! a site performs edit A and then edit B, it must replicate op A before
//! it replicates op B. State can be sent in any order.
//!
//! Similarly, ops must be sent over a network that guarantees in-order
//! delivery. TCP fits this requirement, so any protocol sitting atop
//! TCP (HTTP, WebSockets, SMTP, XMPP, etc.) will work as a transport
//! layer for op-based replication. State can be sent via a protocol
//! that does not guarantee in-order delivery.
//!
//! In general, when replicating a CRDT state you should send its
//! state struct, not the CRDT struct, because the CRDT struct includes
//! the site id. For example, to replicate a
//! `Json` CRDT you should send the serialized `JsonState`, which
//! can be created by calling `json_crdt.state()`.
//!
//! ### Assigning Sites
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
//! Here are some strategies for assigning site identifiers:
//!
//! * Reuse existing site identifiers (e.g. numeric client ids)
//! * Use a central server to assign site ids on a per-CRDT basis
//! * Use a consensus algorithm like Raft or Paxos decide on a new site's id
//!
//! Site ids can be assigned lazily. If a site only needs read access
//! to a CRDT, it doesn't need a site id. If a site without an id edits
//! the CRDT, the CRDT will update locally but ops will be cached and
//! unavailable to the user. When the site receives an id, that id
//! will be retroactively applied to the site's edits, and the cached ops
//! will be returned to be sent over the network.
//!
//! ### Do I need a centralized server to maintain consistency?
//!
//! CRDTs do not require a central server to ensure eventual
//! consistency; you can use them in peer-to-peer protocols,
//! client-server applications, federated services, or any other
//! environment. However you *do* need a way to assign unique site
//! identifiers, as explained in the section [Assigning Sites](#assigning-sites).
//! A centralized server is one way to do that, but not the only way.
//!
//! A server may also be useful as an op cache for unavailable
//! clients. If you are using CRDTs in an application where sites are
//! often offline (for instance, a mobile phone app), you can use
//! the server to store ops and state changes until they have been
//! received by all sites.
//!
//! ### Duplicate ops
//!
//! Ditto CRDTs are *idempotent* — executing an op twice
//! has no effect. As long as ops from a site are executed in
//! the order they were generated, the CRDT will maintain consistency.
//!
//! ### Sending ops vs. sending state
//!
//! Usually, the most compact way to send a change between sites
//! is to send an *op*. An op is just a fancy distributed systems
//! term for "change". Each time you edit a CRDT locally, you generate
//! an op that can be sent to other sites and executed.
//!
//! However, there may be times when it is faster and more compact
//! to send the whole CRDT state (e.g. if you're sending 100 or 1000
//! edits at once). You should replicate exclusively via state if you
//! cannot guarantee in-order op delivery.
//!
//! ### Other Notes
//!
//! Collection CRDTs are inherently larger than their native equivalents
//! because each element must have a unique id. Overhead is most
//! significant when storing a collection of very small values — a `List<u8>`
//! will be many times larger than a `Vec<u8>`. If the collection itself
//! is immutable, you can significantly reduce overhead by switching from
//! a `List<T>` or `Map<K,V>` to a `Register<Vec<T>>` or `Register<Map<K,V>>`.
//!
//! ## License
//!
//! Ditto is licensed under either of
//!
//! * Apache License, Version 2.0 (https://www.apache.org/licenses/LICENSE-2.0)
//! * MIT license (https://opensource.org/licenses/MIT)
//!
//! at your option.
//!
//! ### Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally
//! submitted for inclusion in Ditto by you, as defined in the Apache-2.0
//! license, shall be dual licensed as above, without any additional terms
//! or conditions.

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
#[macro_use] mod traits2;

pub mod counter;
pub mod json;
pub mod list;
pub mod map;
pub mod map2;
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

pub use counter::{Counter, CounterState};
pub use json::{Json, JsonState};
pub use list::{List, ListState};
pub use map::{Map, MapState};
pub use register::{Register, RegisterState};
pub use set::{Set, SetState};
pub use text::{Text, TextState};
pub use xml::{Xml, XmlState};
