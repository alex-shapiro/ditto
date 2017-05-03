extern crate ditto;
extern crate serde;
extern crate serde_json;
extern crate rmp_serde as rmps;

use ditto::{Set, Crdt};
use ditto::set::LocalOp;

#[test]
fn test_set() {
    let mut set1: Set<i32> = Set::new();
    let mut set2: Set<i32> = Set::from_value(set1.clone_value(), 2);
    let mut set3: Set<i32> = Set::from_value(set1.clone_value(), 3);

    let remote_op1 = set1.insert(10).unwrap();
    let remote_op2 = set2.insert(10).unwrap();
    let remote_op3 = set3.insert(20).unwrap();
    let remote_op4 = set1.remove(&10).unwrap();

    let local_op11 = set1.execute_remote(&via_json(&remote_op2)).unwrap();
    let local_op12 = set1.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    assert!(local_op11 == LocalOp::Insert(10));
    assert!(local_op12 == LocalOp::Insert(20));

    let local_op21 = set2.execute_remote(&via_json(&remote_op1));
    let local_op22 = set2.execute_remote(&via_json(&remote_op3)).unwrap();
    let local_op23 = set2.execute_remote(&via_json(&remote_op4));
    assert!(local_op21.is_none());
    assert!(local_op22 == LocalOp::Insert(20));
    assert!(local_op23.is_none());
    assert!(set1.value() == set2.value());

    let local_op31 = set3.execute_remote(&via_msgpack(&remote_op1)).unwrap();
    let local_op32 = set3.execute_remote(&via_msgpack(&remote_op4)).unwrap();
    let local_op33 = set3.execute_remote(&via_msgpack(&remote_op2)).unwrap();
    assert!(local_op31 == LocalOp::Insert(10));
    assert!(local_op32 == LocalOp::Remove(10));
    assert!(local_op33 == LocalOp::Insert(10));
    assert!(set1.value() == set3.value());
    assert!(set2.value() == set3.value());
}

fn via_json<T>(value: &T) -> T
    where T: serde::Serialize + serde::de::DeserializeOwned
{
    let json = serde_json::to_string(value).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn via_msgpack<T>(value: &T) -> T
    where T: serde::Serialize + serde::de::DeserializeOwned
{
    let msgpack = rmps::to_vec(value).unwrap();
    rmps::from_slice(&msgpack).unwrap()
}
