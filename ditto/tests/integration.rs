#[macro_use] extern crate assert_matches;
extern crate ditto;
extern crate serde;
extern crate serde_json;
extern crate rmp_serde as rmps;

use ditto::{Json, List, Map, Register, Set, Text};
use ditto::list;
use ditto::map;
use ditto::set;

#[test]
fn test_list() {
    let mut list1: List<i64> = List::new();
    let mut list2: List<i64> = List::from_state(list1.clone_state(), Some(2)).unwrap();
    let mut list3: List<i64> = List::from_state(list1.clone_state(), Some(3)).unwrap();

    let op1 = list1.insert(0, 5).unwrap();
    let op2 = list2.insert(0, 10).unwrap();
    let op3 = list3.insert(0, 15).unwrap();
    let op4 = list1.remove(0).1.unwrap();

    let local_op11 = list1.execute_op(via_json(&op2)).unwrap();
    let local_op12 = list1.execute_op(via_msgpack(&op3)).unwrap();
    assert_matches!(local_op11, list::LocalOp::Insert{idx: 0, value: 10});
    assert_matches!(local_op12, list::LocalOp::Insert{idx: _, value: 15});

    let local_op21 = list2.execute_op(via_msgpack(&op1)).unwrap();
    let local_op22 = list2.execute_op(via_json(&op3)).unwrap();
    let local_op23 = list2.execute_op(via_msgpack(&op4)).unwrap();
    assert_matches!(local_op21, list::LocalOp::Insert{idx: _, value: 5});
    assert_matches!(local_op22, list::LocalOp::Insert{idx: _, value: 15});
    assert_matches!(local_op23, list::LocalOp::Remove{idx: _});

    let local_op31 = list3.execute_op(via_json(&op1)).unwrap();
    let local_op32 = list3.execute_op(via_msgpack(&op2)).unwrap();
    let local_op33 = list3.execute_op(via_json(&op4)).unwrap();
    assert_matches!(local_op31, list::LocalOp::Insert{idx: _, value: 5});
    assert_matches!(local_op32, list::LocalOp::Insert{idx: _, value: 10});
    assert_matches!(local_op33, list::LocalOp::Remove{idx: _});

    assert_eq!(list1.state(), list2.state());
    assert_eq!(list1.state(), list3.state());
}

#[test]
fn test_map() {
    let mut map1: Map<i32, bool> = Map::new();
    let mut map2: Map<i32, bool> = Map::from_state(map1.clone_state(), Some(2)).unwrap();
    let mut map3: Map<i32, bool> = Map::from_state(map1.clone_state(), Some(3)).unwrap();

    let op1 = map1.insert(0, true).unwrap();
    let op2 = map2.insert(0, false).unwrap();
    let op3 = map3.insert(1, true).unwrap();
    let op4 = map1.remove(&0).unwrap().unwrap();

    let local_op11 = map1.execute_op(via_json(&op2));
    let local_op12 = map1.execute_op(via_msgpack(&op3));
    assert_matches!(local_op11, map::LocalOp::Insert{key: 0, value: false});
    assert_matches!(local_op12, map::LocalOp::Insert{key: 1, value: true});

    let local_op21 = map2.execute_op(via_json(&op1));
    let local_op22 = map2.execute_op(via_msgpack(&op3));
    let local_op23 = map2.execute_op(via_json(&op4));
    assert_matches!(local_op21, map::LocalOp::Insert{key: 0, value: true});
    assert_matches!(local_op22, map::LocalOp::Insert{key: 1, value: true});
    assert_matches!(local_op23, map::LocalOp::Insert{key: 0, value: false});

    let local_op31 = map3.execute_op(via_msgpack(&op1));
    let local_op32 = map3.execute_op(via_json(&op2));
    let local_op33 = map3.execute_op(via_msgpack(&op4));
    assert_eq!(local_op31, map::LocalOp::Insert{key: 0, value: true});
    assert_eq!(local_op32, map::LocalOp::Insert{key: 0, value: true});
    assert_eq!(local_op33, map::LocalOp::Insert{key: 0, value: false});

    assert!(map1.state() == map2.state());
    assert!(map1.state() == map3.state());
}

#[test]
fn test_register() {
    let mut register1 = Register::new(56u32);
    let mut register2 = Register::from_state(register1.state(), Some(2)).unwrap();
    let mut register3 = Register::from_state(register1.state(), Some(3)).unwrap();

    let op1 = register1.update(32).unwrap();
    let op2 = register2.update(41).unwrap();
    let op3 = register3.update(28).unwrap();

    assert_eq!(&32, register1.execute_op(via_json(&op2)));
    assert_eq!(&32, register1.execute_op(via_json(&op3)));
    assert_eq!(&41, register2.execute_op(via_json(&op3)));
    assert_eq!(&32, register2.execute_op(via_json(&op1)));
    assert_eq!(&41, register3.execute_op(via_json(&op2)));
    assert_eq!(&32, register3.execute_op(via_json(&op1)));

    assert_eq!(register1.state(), register2.state());
    assert_eq!(register1.state(), register3.state());
}

#[test]
fn test_set() {
    let mut set1: Set<i32> = Set::new();
    let mut set2: Set<i32> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let mut set3: Set<i32> = Set::from_state(set1.clone_state(), Some(3)).unwrap();

    let op1 = set1.insert(10).unwrap();
    let op2 = set2.insert(10).unwrap();
    let op3 = set3.insert(20).unwrap();
    let op4 = set1.remove(&10).unwrap().unwrap();

    assert_eq!(set1.execute_op(via_msgpack(&op2)), Some(set::LocalOp::Insert(10)));
    assert_eq!(set1.execute_op(via_msgpack(&op3)), Some(set::LocalOp::Insert(20)));

    assert_eq!(set2.execute_op(via_json(&op1)), None);
    assert_eq!(set2.execute_op(via_json(&op3)), Some(set::LocalOp::Insert(20)));
    assert_eq!(set2.execute_op(via_json(&op4)), None);

    assert_eq!(set3.execute_op(via_msgpack(&op1)), Some(set::LocalOp::Insert(10)));
    assert_eq!(set3.execute_op(via_msgpack(&op4)), Some(set::LocalOp::Remove(10)));
    assert_eq!(set3.execute_op(via_msgpack(&op2)), Some(set::LocalOp::Insert(10)));

    assert_eq!(set1.state(), set2.state());
    assert_eq!(set1.state(), set3.state());
}

#[test]
fn test_text() {
    let mut text1 = Text::new();
    let mut text2 = Text::from_state(text1.clone_state(), Some(2)).unwrap();
    let mut text3 = Text::from_state(text1.clone_state(), Some(3)).unwrap();

    let op1 = text1.replace(0, 0, "Hello! ").unwrap().unwrap();
    let op2 = text2.replace(0, 0, "Bonjour. ").unwrap().unwrap();
    let op3 = text3.replace(0, 0, "Buenos dias. ").unwrap().unwrap();

    let _ = text1.execute_op(via_json(&op2));
    let _ = text1.execute_op(via_msgpack(&op3));
    let _ = text2.execute_op(via_msgpack(&op1));
    let _ = text2.execute_op(via_json(&op3));
    let _ = text3.execute_op(via_json(&op1));
    let _ = text3.execute_op(via_msgpack(&op2));

    assert!(text1.state() == text2.state());
    assert!(text1.state() == text3.state());
}

#[test]
fn test_json() {
    let mut crdt1 = Json::from_str("{}").unwrap();
    let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
    let mut crdt3 = Json::from_state(crdt1.clone_state(), Some(3)).unwrap();

    let op1 = crdt1.insert("/foo", 1.0).unwrap();
    let op2 = crdt2.insert("/foo", 2.0).unwrap();
    let op3 = crdt3.insert("/bar", 3.0).unwrap();

    let _ = crdt1.execute_op(via_msgpack(&op2));
    let _ = crdt1.execute_op(via_msgpack(&op3));
    let _ = crdt2.execute_op(via_msgpack(&op1));
    let _ = crdt2.execute_op(via_json(&op3));
    let _ = crdt3.execute_op(via_json(&op1));
    let _ = crdt3.execute_op(via_json(&op2));

    assert_eq!(crdt1.state(), crdt2.state());
    assert_eq!(crdt1.state(), crdt3.state());
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
