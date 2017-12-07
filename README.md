# Ditto

Ditto is a library for using [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type),
or conflict-free replicated data types. CRDTs are data structures
which can be replicated across multiple sites, edited concurrently,
and merged together without leading to conflicts. Ditto provides
a number of commonly used data types:

* **Register\<T\>:** A container for a single value.
* **Set\<T\>:** A HashSet-like collection of unique values
* **Map\<T\>:** A HashMap-like collection of key-value pairs
* **List\<T\>:** A Vec-like ordered sequence of elements
* **Text:** A String-like container for mutable text
* **Json:** A JSON value
* **Xml:** An XML document

Ditto's goal is to be as fast and easy to use as possible. If you have any
questions, suggestions, or other feedback, feel free to open an issue
or a pull request.

## Example

```rust
extern crate ditto;
extern crate serde_json;
use ditto::List;

fn main() {
    // Create a List CRDT. The site that creates the CRDT
    // is automatically assigned id 1.
    let mut list1 = List::from(vec![100,200,300]);

    // Send the list's state over a network to a second site with id 2.
    let encoded_state = serde_json::to_string(&list1.state()).unwrap();
    let decoded_state = serde_json::from_str(&encoded_state).unwrap();
    let mut list2 = List::from_state(decoded_state, Some(2)).unwrap();

    // Edit the list concurrently at both the first and second site.
    let op1 = list1.insert(0, 400).unwrap();
    let op2 = list2.remove(0).unwrap();

    // Each site sends its op to the other site for execution.
    // The encoding and decoding has been left out for brevity.
    list1.execute_remote(&op2);
    list2.execute_remote(&op1);

    // Now both sites have the same value:
    assert_eq!(list1.state(), list2.state());
    assert_eq!(list1.local_value(), vec![400, 200, 300]);
}
```

You can find more examples in the examples and tests directories in the
crate repo.

## Assigning Sites

A CRDT may be distributed across multiple *sites*. A site is
just a fancy distributed systems term for "client". Each
site that wishes to edit the CRDT must have a unique `u32`
identifier.

The site that creates the CRDT is automatically assigned to
id 1. ***You*** are responsible for assigning all other sites;
Ditto will not do it for you.

Here are some strategies for assigning site identifiers:

* Reuse existing site identifiers (e.g. numeric client ids)
* Use a central server to allocate site ids on a per-CRDT basis
* Use a consensus algorithm like Raft or Paxos decide on a new site's id

Site IDs can be allocated lazily. If a site only needs read access
to a CRDT, it doesn't need a site ID. If a site without an ID edits
the CRDT, the CRDT will update locally but all ops will be cached.
When the site receives an ID, that ID will be retroactively applied
to all of the site's edits, and the cached ops will be returned
to be sent over the network.

## Sending ops vs. sending state

Usually, the most compact way to send a change between sites
is to send an *op*. An op is just a fancy distributed systems
term for "change". Each time you edit a CRDT locally, you receive
an op that can be sent to other sites and executed.

However, there may be times when it is faster and more compact
to send the whole CRDT state to other sites (e.g. if you're sending
100 or 1000 edits at once). All Ditto CRDTs can merge with remote states.

**Note:** All ops from a site must be sent in the order they were
generated. That is, if a site performs edit A and then edit B, it
must send op A before it sends op B.

## Serializing CRDTs

All CRDTs and ops are serialized with [Serde](https://serde.rs).
Serialization is tested against `serde_json` and `rmp_serde` but
may work with other formats as well.

In general, you should distribute a CRDT to new sites by serializing
and sending its state, not the CRDT itself, because the CRDT struct
contains site-specific metadata.

## Other Notes

The root value of a `Json` CRDT (typically an object or array) cannot
be replaced. A Json CRDT that is created as an object will stay an object.
This has the effect that any `Json` CRDT with a numeric, boolean, or null
root is immutable.

CRDTs are inherently larger than their native equivalents. A CRDT persisted
with MsgPack uses up to 3x the space of its non-CRDT Rust equivalent.

## License

Ditto is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Ditto by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
