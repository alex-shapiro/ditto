Ditto
=====

Ditto is a CRDT library focusing on simplicity. Its goal is to allow real-time collaboration with JSON-like data.

## Usage

```rust
use ditto::CRDT;

let mut crdt1 = CRDT::new_str("[]", site: 1);
let mut crdt2 = CRDT::new_str("[]", site: 2);

let op1 = crdt1.insert_item_str("", 0, "1");
let op2 = crdt1.insert_item_str("", 1, "\"Hello!\"");
let op3 = crdt2.insert_item_str("", 2, "true");

crdt2.execute_remote(op1);
crdt2.execute_remote(op2);
crdt2.execute_remote(op3);
assert!(crdt1 == crdt2);
```

## Supported Types

**Object**, a mutable key-value data structure with string-typed keys and any supported type as a value. It functions like a JSON object. Supported functions are `put` and `delete`.

**Array**, a mutable vec-like data structure that can hold items of any supported type. It functions like a JSON array. Supported functions are `insert_item` and `delete_item`.

**AttributedString**, a mutable string-like data structure. Supported functions are `insert_text`, `delete_text`, and `replace_text`.

**String**, an immutable string. Strings, unlike AttributeStrings, do not support any functions.

**Number**, a mutable 64-bit float. It supports one function, `increment`.

**Boolean**, an immutable boolean value. Booleans do not support any functions.

**Null** an immutable null value. Nulls do not support any functions.

## Limitations

Ditto does not provide functions for site allocation or network connections. Site allocation must be handled carefully; Ditto may fail to maintain consistency if multiple clients use the same site concurrently.

Ditto requires that each site send its messages to other sites in order; otherwise it may fail to maintain consistency between sites.

The root value of a CRDT cannot be replaced. This means that your root value type is permanent; if you create a CRDT with a String or Bool root type, that means your CRDT is immutable!

Mutable container types **Object**, **Array**, and **AttributedString** have significant memory and storage overhead associated with both the container and each element. A CRDT, when stored, may take over 3x the size of the equivalent non-CRDT JSON structure. This overhead is due to CRDT requirements of unique IDs for each item.
