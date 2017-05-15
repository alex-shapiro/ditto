Ditto
=====

Ditto is a CRDT library focusing on simplicity. It contains `Register`, `Set`, `Map`, `List`, `Text`, and `Json` CRDTs, all of which present a consistent interface for generating local ops, processing remote ops, serializing, and deserializing. All CRDTs are tombstoneless.

For usage examples, take a look at the integration tests.

## CRDT Types

**Register&lt;T&gt;** stores a single value. Its supported operation is `update`, which replaces the value with a new value.

**Set&lt;T&gt;** stores a collection of distinct elements. Its supported operations are `insert` and `remove`, which insert and remove items, respectively.

**Map&lt;K,V&gt;** stores a collection of key-value pairs. The values in the map are immutable. Its supported operations are `insert` and `remove`, which insert and remove key-value pairs, respectively.

**List&lt;T&gt;** stores an ordered sequence of values. Values in the list are immutable. Its supported operations are `insert` and `remove`, which insert and remove list value, respectively.

**Text** is a string-like CRDT for mutable text. Its supported operations are `insert`, `remove`, and `replace`.

**Json** is a container for any kind of data that can be represented via Json - objects, arrays, text, numbers, bools, and null. Its supported operations are `object_insert`, `object_remove`, `array_insert`, `array_remove`, `string_insert`, `string_remove`, and `string_replace`. Numbers, bools, and nulls are immutable.

## Notes

Although Ditto manages a CRDT's site, it does not provide *site allocation* or any other networking feature. Site allocation in particular must be handled carefully; if two or more clients use the same site concurrently you WILL have consistency errors.

Ditto CRDTs are all op-based. Therefore, all remote operations received from some site **S** must be executed in the order that they were generated at **S**. Out-of-order remote execution WILL lead to consistency errors.

The root value of a `Json` CRDT cannot be replaced. This means that if you create a `Json` CRDT with a `Number` or `Bool` root type, your CRDT is immutable!

CRDTs have significant memory and storage overhead compared to standard types. A Text CRDT may  require 5x the storage of a regular string. If the only operation you need for a string is full replacement, consider using the `Register<String>` CRDT instead.
