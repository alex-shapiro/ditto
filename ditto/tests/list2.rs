extern crate ditto;

mod common;
use ditto::Error;
use ditto::list2::*;

#[test]
fn test_new() {
    let list: List<i64> = List::new();
    assert_eq!(list.len(), 0);
}

#[test]
fn test_get() {
    let mut list: List<i64> = List::new();
    let _ = list.insert(0, 123).unwrap();
    assert!(list.get(0) == Some(&123));
}

#[test]
fn test_insert_prepend() {
    let mut list: List<i64> = List::new();
    let op1 = list.insert(0, 123).unwrap();
    let op2 = list.insert(0, 456).unwrap();
    let op3 = list.insert(0, 789).unwrap();

    assert_eq!(list.len(), 3);
    assert_eq!(list.get(0), Some(&789));
    assert_eq!(list.get(1), Some(&456));
    assert_eq!(list.get(2), Some(&123));

    let elt1 = op1.inserted_element().unwrap();
    let elt2 = op2.inserted_element().unwrap();
    let elt3 = op3.inserted_element().unwrap();

    assert!(elt1 > elt2);
    assert!(elt2 > elt3);
}

#[test]
fn test_insert_append() {
    let mut list: List<i64> = List::new();
    let op1 = list.insert(0, 123).unwrap();
    let op2 = list.insert(1, 456).unwrap();
    let op3 = list.insert(2, 789).unwrap();

    assert_eq!(list.get(0), Some(&123));
    assert_eq!(list.get(1), Some(&456));
    assert_eq!(list.get(2), Some(&789));

    let elt1 = op1.inserted_element().unwrap();
    let elt2 = op2.inserted_element().unwrap();
    let elt3 = op3.inserted_element().unwrap();

    assert!(elt1 < elt2);
    assert!(elt2 < elt3);
}

#[test]
fn test_insert_middle() {
    let mut list: List<i64> = List::new();
    let op1 = list.insert(0, 123).unwrap();
    let op2 = list.insert(1, 456).unwrap();
    let op3 = list.insert(1, 789).unwrap();

    assert_eq!(list.get(0), Some(&123));
    assert_eq!(list.get(1), Some(&789));
    assert_eq!(list.get(2), Some(&456));

    let elt1 = op1.inserted_element().unwrap();
    let elt2 = op2.inserted_element().unwrap();
    let elt3 = op3.inserted_element().unwrap();

    assert!(elt1 < elt2);
    assert!(elt3 < elt2);
}

#[test]
#[should_panic]
fn test_insert_out_of_bounds() {
    let mut list: List<i64> = List::new();
    let _ = list.insert(1, 123);
}

#[test]
fn test_remove() {
    let mut list: List<i64> = List::new();
    let _   = list.push(123).unwrap();
    let op1 = list.push(456).unwrap();
    let _   = list.push(789).unwrap();
    let op2 = list.remove(1).1.unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list.get(0), Some(&123));
    assert_eq!(list.get(1), Some(&789));

    let elt = op1.inserted_element().unwrap();
    let uid = op2.removed_uid().unwrap();
    assert_eq!(elt.uid, *uid);
}

#[test]
#[should_panic]
fn test_remove_out_of_bounds() {
    let mut list: List<i64> = List::new();
    let _ = list.remove(0);
}

#[test]
fn test_pop() {
    let mut list: List<i64> = List::new();
    let op1 = list.push(123).unwrap();
    let op2 = list.pop().unwrap().1.unwrap();
    let op3 = list.pop();

    let elt = op1.inserted_element().unwrap();
    let uid = op2.removed_uid().unwrap();

    assert_eq!(list.len(), 0);
    assert_eq!(elt.value, 123);
    assert_eq!(elt.uid, *uid);
    assert_eq!(op3, None);
}

#[test]
fn test_pop_out_of_bounds() {
    let mut list: List<i64> = List::new();
    assert_eq!(list.pop(), None);
}

#[test]
fn test_insert_remove_awaiting_site() {
    let mut list: List<i64> = List::from_state(List::new().state(), None).unwrap();
    assert_eq!(list.push(123), Err(Error::AwaitingSiteId));
    assert_eq!(list.cached_ops().len(), 1);
    assert_eq!(list.pop(), Some((123, Err(Error::AwaitingSiteId))));
    assert_eq!(list.cached_ops().len(), 2);
}

#[test]
fn test_execute_op_insert() {
    let mut list1: List<String> = List::new();
    let mut list2 = List::from_state(list1.state(), None).unwrap();
    let op1 = list1.push("a".into()).unwrap();
    let op2 = list2.execute_op(op1).unwrap();

    assert_eq!(list2.len(), 1);
    assert_eq!(list2.get(0), Some(&"a".into()));
    assert_eq!(op2, LocalOp::Insert{idx: 0, value: "a".into()})
}

#[test]
fn test_execute_op_insert_dupe() {
    let mut list1: List<&'static str> = List::new();
    let mut list2 = List::from_state(list1.state(), None).unwrap();
    let op = list1.insert(0, "a").unwrap();
    assert_eq!(list2.execute_op(op.clone()), Some(LocalOp::Insert{idx: 0, value: "a"}));
    assert_eq!(list2.execute_op(op.clone()), None);
    assert_eq!(list2.len(), 1);
}

#[test]
fn test_execute_op_remove() {
    let mut list1: List<&'static str> = List::new();
    let mut list2 = List::from_state(list1.state(), None).unwrap();
    let op1 = list1.push("a").unwrap();
    let op2 = list1.pop().unwrap().1.unwrap();

    assert_eq!(list2.execute_op(op1), Some(LocalOp::Insert{idx: 0, value: "a"}));
    assert_eq!(list2.execute_op(op2), Some(LocalOp::Remove{idx: 0}));
    assert_eq!(list2.len(), 0);
}

#[test]
fn test_execute_op_remove_dupe() {
    let mut list1: List<&'static str> = List::new();
    let mut list2 = List::from_state(list1.state(), None).unwrap();
    let op1 = list1.push("a").unwrap();
    let op2 = list1.pop().unwrap().1.unwrap();

    assert_eq!(list2.execute_op(op1), Some(LocalOp::Insert{idx: 0, value: "a"}));
    assert_eq!(list2.execute_op(op2.clone()), Some(LocalOp::Remove{idx: 0}));
    assert_eq!(list2.execute_op(op2.clone()), None);
    assert_eq!(list2.len(), 0);
}

#[test]
fn test_merge() {
    let mut list1 = List::new();
    let _ = list1.push(3);
    let _ = list1.push(6);
    let _ = list1.push(9);
    let _ = list1.remove(1);

    let mut list2 = List::from_state(list1.state(), Some(2)).unwrap();
    let _ = list2.remove(0);
    let _ = list2.insert(1, 12);
    let _ = list2.insert(2, 15);
    let _ = list1.remove(1);
    let _ = list1.insert(1, 12);

    let list1_state = list1.clone_state();
    list1.merge(list2.state()).unwrap();
    list2.merge(list1_state).unwrap();

    assert_eq!(list1.state(), list2.state());
    assert_eq!(list1.local_value(), [12, 12, 15]);
    assert!(list1.summary().contains_pair(1,3));
}

#[test]
fn test_add_site_id() {
    let mut list: List<u32> = List::from_state(List::new().state(), None).unwrap();
    let _ = list.push(51);
    let _ = list.push(52);
    let _ = list.pop();
    let ops = list.add_site_id(12).unwrap();

    let elt0 = ops[0].inserted_element().unwrap();
    let elt1 = ops[1].inserted_element().unwrap();
    let uid2 = ops[2].removed_uid().unwrap();

    assert_eq!(elt0.value, 51);
    assert_eq!(elt0.uid.site_id, 12);

    assert_eq!(elt1.value, 52);
    assert_eq!(elt1.uid.site_id, 12);

    assert_eq!(uid2.site_id, 12);
    assert!(elt0.uid < elt1.uid);
    assert!(elt1.uid == *uid2);

    assert_eq!(list.site_id(), 12);
}

#[test]
fn test_add_site_id_already_has_site_id() {
    let mut list: List<u32> = List::from_state(List::new().state(), Some(12)).unwrap();
    let _ = list.push(51).unwrap();
    let _ = list.push(52).unwrap();
    let _ = list.remove(0).1.unwrap();
    assert_eq!(list.add_site_id(13), Err(Error::AlreadyHasSiteId));
}

#[test]
fn test_serialize() {
    let mut list: List<String> = List::new();
    let _ = list.push("Bob".into());
    let _ = list.push("Sue".into());
    common::test_serde(list);
}

#[test]
fn test_serialize_crdt_and_state() {
    let mut list: List<String> = List::new();
    let _ = list.push("Bob".into());
    let _ = list.push("Sue".into());
    common::test_serde(list.state());
}

#[test]
fn test_serialize_op() {
    let mut list: List<i8> = List::new();
    let op1 = list.push(123).unwrap();
    let op2 = list.pop().unwrap().1.unwrap();
    common::test_serde(op1);
    common::test_serde(op2);
}

#[test]
fn test_serialize_local_op() {
    let op1 = LocalOp::Insert{idx: 123, value: "abc".to_owned()};
    let op2: LocalOp<String> = LocalOp::Remove{idx: 123};
    common::test_serde(op1);
    common::test_serde(op2);
}
