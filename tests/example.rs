extern crate ditto;

use ditto::CRDT;

#[test]
fn example() {
    // bob creates a CRDT and dumps its value
    let mut crdt1 = CRDT::create("[1, 2, 3]").unwrap();
    let crdt1_value = crdt1.dump();

    // bob sends crdt1_value to alice, who loads it into crdt2.
    let mut crdt2 = CRDT::load(&crdt1_value, 2, 0).unwrap();

    // crdt1 executes some operations
    let op1 = crdt1.delete_item("", 1).unwrap();
    let op2 = crdt1.insert_item("", 1, "true").unwrap();
    let op3 = crdt1.insert_item("", 1, "32.0").unwrap();

    // crdt2 executes some operations concurrently with crdt1
    let op4 = crdt2.delete_item("", 1).unwrap();
    let op5 = crdt2.insert_item("", 1, "\"Hello!\"").unwrap();
    let op6 = crdt2.insert_item("", 2, "true").unwrap();

    // alice sends crdt2's operations to bob. He executes them on crdt1.
    let _ = crdt1.execute_remote(op4);
    let _ = crdt1.execute_remote(op5);
    let _ = crdt1.execute_remote(op6);

    // bob sends crdt1's operations to alice. She executes them on crdt2.
    let _ = crdt2.execute_remote(op1);
    let _ = crdt2.execute_remote(op2);
    let _ = crdt2.execute_remote(op3);

    // after all operations are replicated at both sites, the sites are identical.
    assert!(crdt1.value() == crdt2.value());
}
