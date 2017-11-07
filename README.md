Ditto
=====

Ditto is a CRDT library focused on simplicity. It contains `Register`, `Set`, `Map`, `List`, `Text`, `Json`, and `Xml` CRDTs and provides a standard interface for creating, updating, and serializing. All Ditto CRDTs have the following properties:

* They can be updated by both remote operations and remote state merges.
* All remote operations and merges are idempotent.
* Remote operations must be sent in the order they were generated.

## Usage

```rust
use ditto::List;

// site 1 creates the CRDT and sends its original value to site 2.
let mut list1: List<u32> = List::new();
let mut list2: List<u32> = List::from_state(list1.clone_state(), 2);

// each site concurrently inserts a different number at index 0
let remote_op1 = list1.insert(0, 7).unwrap();
let remote_op2 = list2.insert(0, 11).unwrap();

// each site executes the other's op
let _ = list1.execute_remote(&remote_op2).unwrap();
let _ = list2.execute_remote(&remote_op1).unwrap();

// now the two sites have lists that contain either [7,11]
// or [11, 7]. In either case, the lists have the same order.
assert!(list1.value() == list2.value());
assert!(list1.value)
```

For more examples, take a look at the integration tests.

## CRDT Types

**Register&lt;T&gt;** stores a single value. Its supported operation is `update`, which replaces the value with a new value.

**Set&lt;T&gt;** stores a collection of distinct elements. Its supported operations are `insert` and `remove`, which insert and remove items, respectively.

**Map&lt;K,V&gt;** stores a collection of key-value pairs. The values in the map are immutable. Its supported operations are `insert` and `remove`, which insert and remove key-value pairs, respectively.

**List&lt;T&gt;** stores an ordered sequence of values. Values in the list are immutable. Its supported operations are `insert` and `remove`, which insert and remove list value, respectively.

**Text** is a string-like CRDT for mutable text. Its supported operation is `replace`, which replaces a range of unicode characters with a new string.

**Json** is a container for any kind of data that can be represented via Json - objects, arrays, text, numbers, bools, and null. Its supported operations are `insert`, `remove`, and `replace_text`. Numbers, bools, and nulls are immutable.

**Xml** is a container for XML documents. It supports both XML 1.0 and 1.1. Its supported operations are `insert`, `remove`, `insert_attribute`, `remove_attribute`, and `replace_text`.

## Notes

Although Ditto CRDTs handle pre-site operations and site addition gracefully, they do not provide site allocation or any other networking feature. Site allocation in particular must be handled carefully; if two or more clients use the same site concurrently you WILL have consistency errors.

Ditto CRDTs are all op-based. Therefore, all remote operations received from some site **S** must be executed in the order that they were generated at **S**. Out-of-order remote execution WILL lead to consistency errors.

The root value of a `Json` CRDT cannot be replaced. This means that if you create a `Json` CRDT with a `Number` or `Bool` root type, your CRDT is immutable.

CRDTs are inherently larger than their native equivalents. A `Text` or `List` CRDT may use up to 3x the space of an equivalent `String` or `Vec`. If the only operation you need for text is full replacement, consider using `Register<String>` instead.
