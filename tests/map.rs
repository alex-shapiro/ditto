extern crate ditto;

mod common;
use ditto::Error;
use ditto::dot::Dot;
use ditto::map::*;

#[test]
fn test_new() {
    let map: Map<i64, bool> = Map::new();
    assert_eq!(map.site_id(), 1);
    assert_eq!(map.contains_key(&412), false);
    assert_eq!(map.summary().get(1), 0);
}

#[test]
fn test_contains_key() {
    let mut map: Map<usize, isize> = Map::new();
    assert!(!map.contains_key(&123));
    let _ = map.insert(123, -123).unwrap();
    assert!(map.contains_key(&123));
}

#[test]
fn test_insert() {
    let mut map: Map<u32, String> = Map::new();
    let op = map.insert(123, "abc".into()).unwrap();
    assert_eq!(op.key(), &123);
    assert_eq!(op.inserted_element().as_ref().unwrap().value, "abc");
    assert_eq!(op.inserted_element().as_ref().unwrap().dot, Dot::new(1,1));
    assert_eq!(op.removed_dots(), []);
}

#[test]
fn test_insert_already_exists() {
    let mut map: Map<String, String> = Map::new();
    let _ = map.insert("a".into(), "x".into()).unwrap();
    let op2 = map.insert("a".into(), "y".into()).unwrap();

    assert_eq!(op2.inserted_element().as_ref().unwrap().value, "y");
    assert_eq!(op2.inserted_element().as_ref().unwrap().dot, Dot::new(1,2));
    assert_eq!(op2.removed_dots(), [Dot::new(1,1)]);
}

#[test]
fn test_insert_awaiting_site() {
    let mut map: Map<u32, String> = Map::from_state(Map::new().clone_state(), None).unwrap();
    assert_eq!(map.insert(123, "abc".into()), Err(Error::AwaitingSiteId));
    assert_eq!(map.get(&123), Some(&"abc".into()));
    assert_eq!(map.cached_ops().len(), 1);
}

#[test]
fn test_remove() {
    let mut map: Map<i8, bool> = Map::new();
    let _  = map.insert(3, true).unwrap();
    let op = map.remove(&3).unwrap().unwrap();
    assert_eq!(op.key(), &3);
    assert_eq!(op.inserted_element(), None);
    assert_eq!(op.removed_dots(), [Dot::new(1,1)]);
}

#[test]
fn test_remove_does_not_exist() {
    let mut map: Map<i8, bool> = Map::new();
    assert_eq!(map.remove(&3), None);
}

#[test]
fn test_remove_awaiting_site() {
    let mut map: Map<i8, bool> = Map::from_state(Map::new().clone_state(), None).unwrap();
    let _ = map.insert(3, true);
    assert_eq!(map.remove(&3), Some(Err(Error::AwaitingSiteId)));
    assert_eq!(map.contains_key(&3), false);
    assert_eq!(map.cached_ops().len(), 2);
}

#[test]
fn test_site_id() {
    let map1: Map<i8, bool> = Map::new();
    let map2: Map<i8, bool> = Map::from_state(map1.state(), None).unwrap();
    let map3: Map<i8, bool> = Map::from_state(map1.state(), Some(999)).unwrap();
    assert_eq!(map1.site_id(), 1);
    assert_eq!(map2.site_id(), 0);
    assert_eq!(map3.site_id(), 999);
}

#[test]
fn test_execute_op_insert() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let op = map1.insert(123, 1010).unwrap();
    assert_eq!(map2.execute_op(op), LocalOp::Insert{key: 123, value: 1010});
    assert_eq!(map2.get(&123).unwrap(), &1010);
}

#[test]
fn test_execute_op_insert_concurrent() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let op1 = map1.insert(123, 2222).unwrap();
    let op2 = map2.insert(123, 1111).unwrap();

    assert_eq!(map1.execute_op(op2), LocalOp::Insert{key: 123, value: 2222});
    assert_eq!(map2.execute_op(op1), LocalOp::Insert{key: 123, value: 2222});
    assert_eq!(map1.get(&123), Some(&2222));
    assert_eq!(map1.state(), map2.state());
}

#[test]
fn test_execute_op_insert_dupe() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let op = map1.insert(123, 2222).unwrap();
    let _  = map2.execute_op(op.clone());

    let state = map2.clone_state();
    assert_eq!(map2.execute_op(op), LocalOp::Insert{key: 123, value: 2222});
    assert_eq!(map2.state(), state);
}

#[test]
fn test_execute_op_remove() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let op1 = map1.insert(123, 2222).unwrap();
    let op2 = map1.remove(&123).unwrap().unwrap();
    let _   = map2.execute_op(op1);

    assert_eq!(map2.execute_op(op2), LocalOp::Remove{key: 123});
    assert_eq!(map2.state(), map1.state());
}

#[test]
fn test_execute_op_remove_does_not_exist() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let _  = map1.insert(123, 2222);
    let op = map1.remove(&123).unwrap().unwrap();

    let state = map2.clone_state();
    assert_eq!(map2.execute_op(op), LocalOp::Remove{key: 123});
    assert_eq!(map2.state(), state);
}

#[test]
fn test_execute_op_remove_some_dots_remain() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let mut map3: Map<i32, u64> = Map::from_state(Map::new().state(), Some(3)).unwrap();
    let op1 = map2.insert(123, 1111).unwrap();
    let op2 = map1.insert(123, 2222).unwrap();
    let op3 = map1.remove(&123).unwrap().unwrap();

    assert_eq!(map3.execute_op(op1), LocalOp::Insert{key: 123, value: 1111});
    assert_eq!(map3.execute_op(op2), LocalOp::Insert{key: 123, value: 2222});
    assert_eq!(map3.execute_op(op3), LocalOp::Insert{key: 123, value: 1111});
}

#[test]
fn test_execute_op_remove_dupe() {
    let mut map1: Map<i32, u64> = Map::new();
    let mut map2: Map<i32, u64> = Map::from_state(Map::new().state(), Some(2)).unwrap();
    let op1 = map1.insert(123, 2222).unwrap();
    let op2 = map1.remove(&123).unwrap().unwrap();

    assert_eq!(map2.execute_op(op1), LocalOp::Insert{key: 123, value: 2222});
    assert_eq!(map2.execute_op(op2.clone()), LocalOp::Remove{key: 123});
    assert_eq!(map2.execute_op(op2), LocalOp::Remove{key: 123});
}

#[test]
fn test_merge() {
    let mut map1: Map<u32, bool> = Map::new();
    let _ = map1.insert(1, true);
    let _ = map1.insert(2, true);
    let _ = map1.remove(&2);
    let _ = map1.insert(3, true);

    let mut map2 = Map::from_state(map1.state(), Some(2)).unwrap();
    let _ = map2.remove(&3);
    let _ = map2.insert(4, true);
    let _ = map2.remove(&4);
    let _ = map2.insert(5, true);
    let _ = map1.insert(4, true);
    let _ = map1.insert(5, true);

    let map1_state = map1.clone_state();
    map1.merge(map2.clone_state()).unwrap();
    map2.merge(map1_state).unwrap();

    assert_eq!(map1.state(), map2.state());
    assert_eq!(map1.get(&1), Some(&true));
    assert_eq!(map1.get(&2), None);
    assert_eq!(map1.get(&3), None);
    assert_eq!(map1.get(&4), Some(&true));
    assert_eq!(map1.get(&5), Some(&true));
    assert!(map1.summary().contains_pair(1, 4));
    assert!(map1.summary().contains_pair(2, 2));
}

#[test]
fn test_add_site_id() {
    let mut map: Map<i32, u64> = Map::from_state(Map::new().state(), None).unwrap();
    let _ = map.insert(10, 56);
    let _ = map.insert(20, 57);
    let _ = map.remove(&10);
    let ops = map.add_site_id(5).unwrap();

    assert_eq!(ops[0].key(), &10);
    assert_eq!(ops[0].inserted_element().unwrap().value, 56);
    assert_eq!(ops[0].inserted_element().unwrap().dot, Dot::new(5,1));

    assert_eq!(ops[1].key(), &20);
    assert_eq!(ops[1].inserted_element().unwrap().value, 57);
    assert_eq!(ops[1].inserted_element().unwrap().dot, Dot::new(5,2));

    assert_eq!(ops[2].key(), &10);
    assert_eq!(ops[2].inserted_element(), None);
    assert_eq!(ops[2].removed_dots(), [Dot::new(5,1)]);
}

#[test]
fn test_add_site_id_already_has_site_id() {
    let mut map: Map<i32, u64> = Map::from_state(Map::new().state(), Some(123)).unwrap();
    let _ = map.insert(10, 56).unwrap();
    let _ = map.insert(20, 57).unwrap();
    let _ = map.remove(&10).unwrap().unwrap();
    assert_eq!(map.add_site_id(3), Err(Error::AlreadyHasSiteId));
}

#[test]
fn test_serialize() {
    let mut map: Map<String, usize> = Map::new();
    let _ = map.insert("a".into(), 100);
    let _ = map.insert("b".into(), 110);
    let _ = map.insert("c".into(), 111);
    common::test_serde(map);
}

#[test]
fn test_serialize_state() {
    let mut map: Map<i32, i64> = Map::new();
    let _ = map.insert(1, 1);
    let _ = map.insert(2, 3);
    let _ = map.insert(5, 8);
    common::test_serde(map.state());
}

#[test]
fn test_serialize_op() {
    let mut map: Map<String, bool> = Map::new();
    let op1 = map.insert("abc".into(), true).unwrap();
    let op2 = map.remove(&"abc".into()).unwrap().unwrap();
    common::test_serde(op1);
    common::test_serde(op2);
}

#[test]
fn test_serialize_local_op() {
    let op1 = LocalOp::Insert{key: "abc".to_owned(), value: 103};
    let op2: LocalOp<String, i32> = LocalOp::Remove{key: "abc".to_owned()};
    common::test_serde(op1);
    common::test_serde(op2);
}
