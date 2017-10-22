#[macro_use] extern crate assert_matches;
extern crate ditto;
extern crate serde;
extern crate serde_json;
extern crate rmp_serde as rmps;

use ditto::{Json, List, Map, Register, Set, Text};
use ditto::{list, map, set};

#[test]
fn test_list() {
    let mut list1: List<i64> = List::new();
    let mut list2: List<i64> = List::from_state(list1.clone_state(), 2);
    let mut list3: List<i64> = List::from_state(list1.clone_state(), 3);

    let remote_op1 = list1.insert(0, 5).unwrap();
    let remote_op2 = list2.insert(0, 10).unwrap();
    let remote_op3 = list3.insert(0, 15).unwrap();
    let remote_op4 = list1.remove(0).unwrap();

    let local_op11 = list1.execute_remote(&via_json(&remote_op2)).unwrap();
    let local_op12 = list1.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    assert_matches!(local_op11, list::LocalOp::Insert{index: 0, value: 10});
    assert_matches!(local_op12, list::LocalOp::Insert{index: _, value: 15});

    let local_op21 = list2.execute_remote(&via_msgpack(&remote_op1)).unwrap();
    let local_op22 = list2.execute_remote(&via_json(&remote_op3)).unwrap();
    let local_op23 = list2.execute_remote(&via_msgpack(&remote_op4)).unwrap();
    assert_matches!(local_op21, list::LocalOp::Insert{index: _, value: 5});
    assert_matches!(local_op22, list::LocalOp::Insert{index: _, value: 15});
    assert_matches!(local_op23, list::LocalOp::Remove{index: _});

    let local_op31 = list3.execute_remote(&via_json(&remote_op1)).unwrap();
    let local_op32 = list3.execute_remote(&via_msgpack(&remote_op2)).unwrap();
    let local_op33 = list3.execute_remote(&via_json(&remote_op4)).unwrap();
    assert_matches!(local_op31, list::LocalOp::Insert{index: _, value: 5});
    assert_matches!(local_op32, list::LocalOp::Insert{index: _, value: 10});
    assert_matches!(local_op33, list::LocalOp::Remove{index: _});

    assert!(list1.value() == list2.value());
    assert!(list1.value() == list3.value());
}

#[test]
fn test_map() {
    let mut map1: Map<i32, bool> = Map::new();
    let mut map2: Map<i32, bool> = Map::from_state(map1.clone_state(), 2);
    let mut map3: Map<i32, bool> = Map::from_state(map1.clone_state(), 3);

    let remote_op1 = map1.insert(0, true).unwrap();
    let remote_op2 = map2.insert(0, false).unwrap();
    let remote_op3 = map3.insert(1, true).unwrap();
    let remote_op4 = map1.remove(&0).unwrap();

    let local_op11 = map1.execute_remote(&via_json(&remote_op2)).unwrap();
    let local_op12 = map1.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    assert_matches!(local_op11, map::LocalOp::Insert{key: 0, value: false});
    assert_matches!(local_op12, map::LocalOp::Insert{key: 1, value: true});

    let local_op21 = map2.execute_remote(&via_json(&remote_op1)).unwrap();
    let local_op22 = map2.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    let local_op23 = map2.execute_remote(&via_json(&remote_op4)).unwrap();
    assert_matches!(local_op21, map::LocalOp::Insert{key: 0, value: true});
    assert_matches!(local_op22, map::LocalOp::Insert{key: 1, value: true});
    assert_matches!(local_op23, map::LocalOp::Insert{key: 0, value: false});

    let local_op31 = map3.execute_remote(&via_msgpack(&remote_op1)).unwrap();
    let local_op32 = map3.execute_remote(&via_json(&remote_op2));
    let local_op33 = map3.execute_remote(&via_msgpack(&remote_op4)).unwrap();
    assert_matches!(local_op31, map::LocalOp::Insert{key: 0, value: true});
    assert_matches!(local_op32, None);
    assert_matches!(local_op33, map::LocalOp::Insert{key: 0, value: false});

    assert!(map1.value() == map2.value());
    assert!(map1.value() == map3.value());
}

#[test]
fn test_register() {
    let mut register1: Register<u32> = Register::new(56);
    let mut register2: Register<u32> = Register::from_state(register1.clone_state(), 2);
    let mut register3: Register<u32> = Register::from_state(register1.clone_state(), 3);

    let remote_op1 = register2.update(41).unwrap();
    let remote_op2 = register1.update(32).unwrap();
    let remote_op3 = register3.update(28).unwrap();

    let local_op11 = register1.execute_remote(&via_json(&remote_op1));
    let local_op12 = register1.execute_remote(&via_json(&remote_op3));
    assert_matches!(local_op11, None);
    assert_matches!(local_op12, None);

    let local_op21 = register2.execute_remote(&via_json(&remote_op2)).unwrap();
    let local_op22 = register2.execute_remote(&via_json(&remote_op3));
    assert!(local_op21.new_value == 32);
    assert_matches!(local_op22, None);

    let local_op31 = register3.execute_remote(&via_json(&remote_op1)).unwrap();
    let local_op32 = register3.execute_remote(&via_json(&remote_op2)).unwrap();
    assert!(local_op31.new_value == 41);
    assert!(local_op32.new_value == 32);

    assert!(register1.value() == register2.value());
    assert!(register1.value() == register3.value());
}

#[test]
fn test_set() {
    let mut set1: Set<i32> = Set::new();
    let mut set2: Set<i32> = Set::from_state(set1.clone_state(), 2);
    let mut set3: Set<i32> = Set::from_state(set1.clone_state(), 3);

    let remote_op1 = set1.insert(10).unwrap();
    let remote_op2 = set2.insert(10).unwrap();
    let remote_op3 = set3.insert(20).unwrap();
    let remote_op4 = set1.remove(&10).unwrap();

    let local_op11 = set1.execute_remote(&via_json(&remote_op2)).unwrap();
    let local_op12 = set1.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    assert!(local_op11 == set::LocalOp::Insert(10));
    assert!(local_op12 == set::LocalOp::Insert(20));

    let local_op21 = set2.execute_remote(&via_json(&remote_op1));
    let local_op22 = set2.execute_remote(&via_json(&remote_op3)).unwrap();
    let local_op23 = set2.execute_remote(&via_json(&remote_op4));
    assert!(local_op21.is_none());
    assert!(local_op22 == set::LocalOp::Insert(20));
    assert!(local_op23.is_none());
    assert!(set1.value() == set2.value());

    let local_op31 = set3.execute_remote(&via_msgpack(&remote_op1)).unwrap();
    let local_op32 = set3.execute_remote(&via_msgpack(&remote_op4)).unwrap();
    let local_op33 = set3.execute_remote(&via_msgpack(&remote_op2)).unwrap();
    assert!(local_op31 == set::LocalOp::Insert(10));
    assert!(local_op32 == set::LocalOp::Remove(10));
    assert!(local_op33 == set::LocalOp::Insert(10));

    assert!(set1.value() == set3.value());
    assert!(set2.value() == set3.value());
}

#[test]
fn test_text() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.clone_state(), 2);
    let mut text3 = Text::from_state(text1.clone_state(), 3);

    let remote_op1 = text1.replace(0, 0, "Hello! ").unwrap();
    let remote_op2 = text2.replace(0, 0, "Bonjour. ").unwrap();
    let remote_op3 = text3.replace(0, 0, "Buenos dias. ").unwrap();

    let _ = text1.execute_remote(&via_json(&remote_op2)).unwrap();
    let _ = text1.execute_remote(&via_msgpack(&remote_op3)).unwrap();
    let _ = text2.execute_remote(&via_msgpack(&remote_op1)).unwrap();
    let _ = text2.execute_remote(&via_json(&remote_op3)).unwrap();
    let _ = text3.execute_remote(&via_json(&remote_op1)).unwrap();
    let _ = text3.execute_remote(&via_msgpack(&remote_op2)).unwrap();

    assert!(text1.value() == text2.value());
    assert!(text1.value() == text3.value());
}

#[test]
fn test_json() {
    let mut crdt1 = Json::from_str("{}").unwrap();
    let mut crdt2 = Json::from_state(crdt1.clone_state(), 2);
    let mut crdt3 = Json::from_state(crdt1.clone_state(), 3);

    let remote_op1 = crdt1.insert("/foo", 1.0).unwrap();
    let remote_op2 = crdt2.insert("/foo", 2.0).unwrap();
    let remote_op3 = crdt3.insert("/bar", 3.0).unwrap();

    let _ = crdt1.execute_remote(&via_msgpack(&remote_op2));
    let _ = crdt1.execute_remote(&via_msgpack(&remote_op3));
    let _ = crdt2.execute_remote(&via_msgpack(&remote_op1));
    let _ = crdt2.execute_remote(&via_json(&remote_op3));
    let _ = crdt3.execute_remote(&via_json(&remote_op1));
    let _ = crdt3.execute_remote(&via_json(&remote_op2));

    assert!(crdt1.value() == crdt2.value());
    assert!(crdt1.value() == crdt3.value());
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
