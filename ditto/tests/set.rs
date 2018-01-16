extern crate ditto;

mod common;
use ditto::Error;
use ditto::Replica;
use ditto::set::*;

#[test]
fn test_new() {
    let set: Set<u8> = Set::new();
    assert_eq!(set.site_id(), 1);
    assert_eq!(set.contains(&41), false);
    assert_eq!(set.summary().get(1), 0);
}

#[test]
fn test_contains() {
    let mut set: Set<u8> = Set::new();
    assert_eq!(set.contains(&41), false);
    let _ = set.insert(41).unwrap();
    assert_eq!(set.contains(&41), true);
}

#[test]
fn test_insert() {
    let mut set: Set<u32> = Set::new();
    let op = set.insert(123).unwrap();
    assert_eq!(op.value(), &123);
    assert_eq!(op.inserted_dot(), Some(&Replica::new(1, 1)));
    assert_eq!(op.removed_dots(), []);
}

#[test]
fn test_insert_already_exists() {
    let mut set: Set<u32> = Set::new();
    let op1 = set.insert(123).unwrap();
    let op2 = set.insert(123).unwrap();
    assert_eq!(op2.removed_dots().first(), op1.inserted_dot());
}

#[test]
fn test_insert_awaiting_site() {
    let set1: Set<u32> = Set::new();
    let mut set2: Set<u32> = Set::from_state(set1.clone_state(), None).unwrap();
    assert!(set2.insert(123).unwrap_err() == Error::AwaitingSiteId);
    assert!(set2.contains(&123));
}

#[test]
fn test_remove() {
    let mut set: Set<u32> = Set::new();
    let op1 = set.insert(123).unwrap();
    let op2 = set.remove(&123).unwrap().unwrap();

    assert_eq!(op1.value(), &123);
    assert_eq!(op1.inserted_dot(), Some(&Replica::new(1,1)));
    assert_eq!(op1.removed_dots(), []);

    assert_eq!(op2.value(), &123);
    assert_eq!(op2.inserted_dot(), None);
    assert_eq!(op2.removed_dots(), [Replica::new(1,1)]);
}

#[test]
fn test_remove_does_not_exist() {
    let mut set: Set<u32> = Set::new();
    assert_eq!(set.remove(&123), None);
}

#[test]
fn test_remove_awaiting_site() {
    let set1: Set<u32> = Set::new();
    let mut set2: Set<u32> = Set::from_state(set1.clone_state(), None).unwrap();
    let _ = set2.insert(123);
    assert_eq!(set2.remove(&123), Some(Err(Error::AwaitingSiteId)));
    assert_eq!(set2.contains(&123), false);
}

#[test]
fn test_site_id() {
    let set1: Set<u64> = Set::new();
    let set2: Set<u64> = Set::from_state(set1.clone_state(), Some(8403)).unwrap();
    assert_eq!(set1.site_id(), 1);
    assert_eq!(set2.site_id(), 8403);
}

#[test]
fn execute_remote_insert() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(8403)).unwrap();
    let op = set1.insert(22).unwrap();
    let local_op = set2.execute_op(op).unwrap();
    assert_eq!(local_op, LocalOp::Insert(22));
}

#[test]
fn execute_remote_insert_value_already_exists() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();

    let op1 = set1.insert(22).unwrap();
    let op2 = set1.remove(&22).unwrap().unwrap();
    let   _ = set2.insert(22).unwrap();

    assert_eq!(set2.execute_op(op1), None);
    assert_eq!(set2.execute_op(op2), None);
    assert!(set2.contains(&22));
}

#[test]
fn execute_remote_insert_dupe() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let op = set1.insert(22).unwrap();
    let _  = set2.execute_op(op.clone()).unwrap();
    assert_eq!(set2.execute_op(op), None);
}

#[test]
fn execute_remote_remove() {
    let mut set1: Set<u64> = Set::new();
    let _ = set1.insert(10).unwrap();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let op = set1.remove(&10).unwrap().unwrap();
    let local_op = set2.execute_op(op).unwrap();

    assert!(!set2.contains(&10));
    assert_eq!(local_op, LocalOp::Remove(10));
}

#[test]
fn execute_remote_remove_does_not_exist() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let _ = set1.insert(10).unwrap();
    let op = set1.remove(&10).unwrap().unwrap();
    assert_eq!(set2.execute_op(op), None);
    assert!(!set2.contains(&10));
}

#[test]
fn execute_remote_remove_some_dots_remain() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let _ = set1.insert(10).unwrap();
    let _ = set2.insert(10).unwrap();
    let op = set1.remove(&10).unwrap().unwrap();
    assert_eq!(set2.execute_op(op), None);
    assert!(set2.contains(&10));
}

#[test]
fn execute_remote_remove_dupe() {
    let mut set1: Set<u64> = Set::new();
    let mut set2: Set<u64> = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let op1 = set1.insert(10).unwrap();
    let op2 = set1.remove(&10).unwrap().unwrap();

    assert_eq!(set2.execute_op(op1), Some(LocalOp::Insert(10)));
    assert_eq!(set2.execute_op(op2.clone()), Some(LocalOp::Remove(10)));
    assert_eq!(set2.execute_op(op2), None);
    assert!(!set2.contains(&10));
}

#[test]
fn test_merge() {
    let mut set1: Set<u32> = Set::new();
    let _ = set1.insert(1);
    let _ = set1.insert(2);
    let _ = set1.remove(&2);

    let mut set2 = Set::from_state(set1.clone_state(), Some(2)).unwrap();
    let _ = set1.insert(3);
    let _ = set2.insert(3);
    let _ = set2.insert(4);
    let _ = set2.remove(&3);

    let set1_state = set1.clone_state();
    set1.merge(set2.state()).unwrap();
    set2.merge(set1_state).unwrap();
    assert_eq!(set1.state(), set2.state());

    assert!(set1.contains(&1));
    assert!(!set1.contains(&2));
    assert!(set1.contains(&3));
    assert!(set1.contains(&4));

    let op1 = set1.remove(&1).unwrap().unwrap();
    assert_eq!(op1.removed_dots(), [Replica::new(1, 1)]);

    let op2 = set1.remove(&3).unwrap().unwrap();
    assert_eq!(op2.removed_dots(), [Replica::new(1, 3)]);

    let op3 = set1.remove(&4).unwrap().unwrap();
    assert_eq!(op3.removed_dots(), [Replica::new(2, 2)]);

    assert!(set1.summary().contains_pair(1, 3));
    assert!(set1.summary().contains_pair(2, 2));
}

#[test]
fn test_add_site_id() {
    let mut set: Set<u64> = Set::from_state(Set::new().clone_state(), None).unwrap();
    let _ = set.insert(10);
    let _ = set.insert(20);
    let _ = set.remove(&10);
    let ops = set.add_site_id(5).unwrap();

    assert_eq!(ops.len(), 3);
    assert_eq!(ops[0].value(), &10);
    assert_eq!(ops[0].inserted_dot(), Some(&Replica::new(5,1)));
    assert_eq!(ops[0].removed_dots(), []);

    assert_eq!(ops[1].value(), &20);
    assert_eq!(ops[1].inserted_dot(), Some(&Replica::new(5,2)));
    assert_eq!(ops[1].removed_dots(), []);

    assert_eq!(ops[2].value(), &10);
    assert_eq!(ops[2].inserted_dot(), None);
    assert_eq!(ops[2].removed_dots(), [Replica::new(5,1)]);
}

#[test]
fn test_add_site_id_already_has_site_id() {
    let mut set: Set<u64> = Set::from_state(Set::new().clone_state(), Some(123)).unwrap();
    let _ = set.insert(10);
    let _ = set.insert(20);
    let _ = set.remove(&10);
    assert_eq!(set.add_site_id(42), Err(Error::AlreadyHasSiteId));
}

#[test]
fn test_serialize() {
    let mut set: Set<String> = Set::new();
    let _ = set.insert("a".into()).unwrap();
    let _ = set.insert("b".into()).unwrap();
    let _ = set.insert("c".into()).unwrap();
    common::test_serde(set);
}

#[test]
fn test_serialize_state() {
    let mut set: Set<String> = Set::new();
    let _ = set.insert("a".into()).unwrap();
    let _ = set.insert("b".into()).unwrap();
    let _ = set.insert("c".into()).unwrap();
    common::test_serde(set.state());
}

#[test]
fn test_serialize_op() {
    let mut set1: Set<i64> = Set::new();
    common::test_serde(set1.insert(123).unwrap());
    common::test_serde(set1.remove(&123).unwrap().unwrap());
}

#[test]
fn test_serialize_local_op() {
    let mut set1: Set<i64> = Set::new();
    let mut set2 = Set::from_state(set1.state(), None).unwrap();
    let op = set1.insert(123).unwrap();
    common::test_serde(set2.execute_op(op).unwrap());
}
