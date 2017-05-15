Ditto
=====

Ditto is a CRDT library focused on simplicity. It contains `Register`, `Set`, `Map`, `List`, `Text`, and `Json` CRDTs and provides a standard interface for generating ops, executing remote ops, and serialization. All CRDTs are op-based and tombstoneless.

## Usage

```rust
use ditto::List;

// site 1 creates the CRDT and sends its original value to site 2.
let mut list1: List<u32> = List::new();
let mut list2: List<u32> = List::from_value(list1.clone_value(), 2);

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

**Text** is a string-like CRDT for mutable text. Its supported operations are `insert`, `remove`, and `replace`.

**Json** is a container for any kind of data that can be represented via Json - objects, arrays, text, numbers, bools, and null. Its supported operations are `object_insert`, `object_remove`, `array_insert`, `array_remove`, `string_insert`, `string_remove`, and `string_replace`. Numbers, bools, and nulls are immutable.

## Notes

Although Ditto CRDTs handle pre-site operations and site addition gracefully, they do not provide site allocation or any other networking feature. Site allocation in particular must be handled carefully; if two or more clients use the same site concurrently you WILL have consistency errors.

Ditto CRDTs are all op-based. Therefore, all remote operations received from some site **S** must be executed in the order that they were generated at **S**. Out-of-order remote execution WILL lead to consistency errors.

The root value of a `Json` CRDT cannot be replaced. This means that if you create a `Json` CRDT with a `Number` or `Bool` root type, your CRDT is immutable.

CRDTs are much larger than their equivalent native types. The `Text` and `List` CRDTs in particular may require 5x or more memory than `String` or `Vec`. If the only operation you need for a `Text` CRDT is full replacement, consider using `Register<String>` instead.
