extern crate ditto;

mod common;

use common::test_serde;
use ditto::Error;
use ditto::counter::*;

#[test]
fn test_new() {
    let counter = Counter::new(4012);
    assert_eq!(counter.get(), 4012);
    assert_eq!(counter.site_id(), 1);
}

#[test]
fn test_increment() {
    let mut counter = Counter::new(0);
    let _ = counter.increment(12).unwrap();
    let _ = counter.increment(-5).unwrap();
    assert_eq!(counter.get(), 7);
}

#[test]
fn test_increment_awaiting_site() {
    let mut counter = Counter::from_state(Counter::new(7).clone_state(), None).unwrap();
    assert_eq!(counter.increment(43), Err(Error::AwaitingSiteId));
    assert_eq!(counter.get(), 50);
}

#[test]
fn test_execute_op() {
    let mut counter1 = Counter::new(17);
    let mut counter2 = Counter::from_state(counter1.clone_state(), None).unwrap();
    assert_eq!(counter1.state(), counter2.state());

    let op1 = counter1.increment(-2).unwrap();
    let op2 = counter1.increment(5).unwrap();
    let op3 = counter1.increment(1000).unwrap();

    let local_op1 = counter2.execute_op(&op1).unwrap();
    let local_op2 = counter2.execute_op(&op2).unwrap();
    let local_op3 = counter2.execute_op(&op3).unwrap();

    assert_eq!(counter1.state(), counter2.state());
    assert_eq!(local_op1, -2);
    assert_eq!(local_op2, 5);
    assert_eq!(local_op3, 1000);
}

#[test]
fn test_execute_op_dupe() {
    let mut counter1 = Counter::new(17);
    let mut counter2 = Counter::from_state(counter1.clone_state(), None).unwrap();

    let op1 = counter1.increment(-2).unwrap();
    let op2 = counter1.increment(5).unwrap();

    let _ = counter2.execute_op(&op1).unwrap();
    let _ = counter2.execute_op(&op2).unwrap();

    assert_eq!(counter2.execute_op(&op1), None);
    assert_eq!(counter2.execute_op(&op2), None);
    assert_eq!(counter1.state(), counter2.state());
}

#[test]
fn test_execute_op_out_of_order() {
    let mut counter1 = Counter::new(17);
    let mut counter2 = Counter::from_state(counter1.clone_state(), Some(2)).unwrap();

    let op1 = counter1.increment(-2).unwrap();
    let op2 = counter1.increment(5).unwrap();
    let op3 = counter1.increment(1).unwrap();
    let op4 = counter2.increment(12).unwrap();
    let op5 = counter2.increment(1).unwrap();

    assert_eq!(counter2.execute_op(&op1), Some(-2));
    assert_eq!(counter2.execute_op(&op3), Some(6));
    assert_eq!(counter2.execute_op(&op2), None);
    assert_eq!(counter1.execute_op(&op5), Some(13));
    assert_eq!(counter1.execute_op(&op4), None);
    assert_eq!(counter1.state(), counter2.state());
}

#[test]
fn test_merge() {
    let mut counter1 = Counter::new(-99);
    let mut counter2 = Counter::from_state(counter1.clone_state(), Some(2)).unwrap();
    let mut counter3 = Counter::from_state(counter1.clone_state(), Some(3)).unwrap();

    let _ = counter1.increment(-1);
    let _ = counter1.increment(-2);
    let _ = counter2.increment(-3);
    let _ = counter2.increment(-4);
    let _ = counter3.increment(-5);
    let _ = counter3.increment(-6);

    counter1.merge(counter2.clone_state());
    counter1.merge(counter3.clone_state());
    counter2.merge(counter1.clone_state());
    counter2.merge(counter3.clone_state());
    counter3.merge(counter1.clone_state());
    counter3.merge(counter2.clone_state());

    assert_eq!(counter1.get(), -120);
    assert_eq!(counter1.state(), counter2.state());
    assert_eq!(counter1.state(), counter3.state());
}


#[test]
fn test_add_site_id() {
    let mut counter = Counter::from_state(Counter::new(0).clone_state(), None).unwrap();

    assert!(counter.increment(1).is_err());
    assert!(counter.increment(-2).is_err());
    assert!(counter.increment(3).is_err());

    let op = counter.add_site_id(123).unwrap().unwrap();
    assert_eq!(counter.site_id(), 123);
    assert_eq!(op.site_id(), 123);
}

#[test]
fn test_add_site_id_already_has_site_id() {
    let mut counter = Counter::from_state(Counter::new(123).clone_state(), Some(2)).unwrap();
    assert_eq!(counter.add_site_id(123), Err(Error::AlreadyHasSiteId));
}

#[test]
fn test_serialize() {
    test_serde(Counter::new(123));
}

#[test]
fn test_serialize_state() {
    test_serde(Counter::new(123).into_state());
}

#[test]
fn test_serialize_op() {
    let mut counter = Counter::new(123);
    test_serde(counter.increment(-142).unwrap());
}

#[test]
fn test_serialize_local_op() {
    let mut counter1 = Counter::new(123);
    let mut counter2 = Counter::from_state(counter1.clone_state(), None).unwrap();
    let op = counter1.increment(13).unwrap();
    let local_op = counter2.execute_op(&op).unwrap();
    test_serde(local_op);
}
