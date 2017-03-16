Ditto
=====

Ditto is a CRDT library focusing on simplicity. Its goal is to allow real-time collaboration with JSON-like data.

## Usage

```rust
use ditto::CRDT;

// bob creates a CRDT and dumps its value
let mut crdt1 = CRDT::create("[1, 2, 3]");
let crdt1_value = crdt1.dump();

// bob sends crdt1_value to alice, who loads it into crdt2.
let mut crdt2 = CRDT::load(crdt1_value, 2, 0).unwrap();

// crdt1 executes some operations
let op1 = crdt1.delete_item("", 1).unwrap();
let op2 = crdt1.insert_item("", 1, "true").unwrap();
let op3 = crdt1.insert_item("", 1, "32.0").unwrap();

// crdt2 executes some operations concurrently with crdt1
let op4 = crdt2.delete_item("", 1).unwrap();
let op5 = crdt2.insert_item("", 1, "\"Hello!\"").unwrap();
let op6 = crdt2.insert_item("", 2, "true").unwrap();

// alice sends crdt2's operations to bob. He executes them on crdt1.
crdt1.execute_remote(&op4);
crdt1.execute_remote(&op5);
crdt1.execute_remote(&op6);

// bob sends crdt1's operations to alice. She executes them on crdt2.
crdt2.execute_remote(&op1);
crdt2.execute_remote(&op2);
crdt2.execute_remote(&op3);

// after all operations are replicated at both sites, the sites are identical.
assert!(crdt1.value() == crdt2.value());
```

## Supported Types

**Object**, a mutable key-value data structure with string-typed keys and any supported type as a value. It functions like a JSON object. Supported functions are `put` and `delete`. The `__TYPE__` key is restricted; any attempt to set this key will fail.

**Array**, a mutable vec-like data structure that can hold items of any supported type. It functions like a JSON array. Supported functions are `insert_item` and `delete_item`.

**AttributedString** stores and efficiently edits large mutable strings. Indexed by unicode character. Supported functions are `insert_text`, `delete_text`, and `replace_text`.

**Counter**, a mutable 64-bit float. Supports the `increment` function. Counters require more space than Numbers and should only be used when concurrent increments are required.

**String**, an immutable string.

**Number**, an immutable 64-bit float.

**Boolean**, an immutable boolean value.

**Null** an immutable null value.

## Limitations

Ditto does not provide site allocation or networking features. Site allocation in particular must be handled carefully; two or more clients using the same site concurrently will lead to consistency errors.

All remote operations received from some site **S** must be executed in the order that they were generated at **S**. Out-of-order remote execution will lead to consistency errors.

The root value of a CRDT cannot be replaced. This means that your root value type is permanent; if you create a CRDT with a String or Bool root type, that means your CRDT is immutable!

Mutable container types **Object**, **Array**, and **AttributedString** have significant memory and storage overhead associated with both the container and each element. A CRDT, when stored, may take over 3x the size of the equivalent non-CRDT JSON structure. This overhead is due to CRDT requirements of unique IDs for each item.

Ditto is closely bound to `serde_json` at the moment. It is a required crate; this may change over time as more compact binary encoder are evaluated for serialization to disk and network.
