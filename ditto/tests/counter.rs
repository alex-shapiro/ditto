extern crate ditto;

mod common;

use common::test_serde;
use ditto::Error;
use ditto::counter::*;

#[test]
fn test_new() {
    let counter = Counter::new(4012);
    assert_eq!(counter.get(), 4012);
    assert_eq!(counter.site(), 1);
    assert_eq!(counter.counter(), 1);
}

#[test]
fn test_increment() {
    let mut counter = Counter::new(0);
    let _ = counter.increment(12).unwrap();
    let _ = counter.increment(-5).unwrap();
    assert_eq!(counter.get(), 7);
    assert_eq!(counter.counter(), 3);
}

#[test]
fn test_increment_awaiting_site() {
    let mut counter = Counter::from_state(Counter::new(7).clone_state(), None).unwrap();
    assert_eq!(counter.increment(43), Err(Error::AwaitingSite));
    assert_eq!(counter.get(), 50);
}

#[test]
fn test_execute_remote() {
    let mut counter1 = Counter::new(17);
    let mut counter2 = Counter::from_state(counter1.clone_state(), None).unwrap();
    assert_eq!(counter1.value(), counter2.value());

    let op1 = counter1.increment(-2).unwrap();
    let op2 = counter1.increment(5).unwrap();
    let op3 = counter1.increment(1000).unwrap();

    let local_op1 = counter2.execute_remote(&op1).unwrap();
    let local_op2 = counter2.execute_remote(&op2).unwrap();
    let local_op3 = counter2.execute_remote(&op3).unwrap();

    assert_eq!(counter1.value(), counter2.value());
    assert_eq!(local_op1, LocalOp(-2));
    assert_eq!(local_op2, LocalOp(5));
    assert_eq!(local_op3, LocalOp(1000));
}

#[test]
fn test_execute_remote_dupe() {
    let mut counter1 = Counter::new(17);
    let mut counter2 = Counter::from_state(counter1.clone_state(), None).unwrap();

    let op1 = counter1.increment(-2).unwrap();
    let op2 = counter1.increment(5).unwrap();

    let _ = counter2.execute_remote(&op1).unwrap();
    let _ = counter2.execute_remote(&op2).unwrap();

    assert_eq!(counter2.execute_remote(&op1), None);
    assert_eq!(counter2.execute_remote(&op2), None);
    assert_eq!(counter1.value(), counter2.value());
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
    assert_eq!(counter1.value(), counter2.value());
    assert_eq!(counter1.value(), counter3.value());
    assert_eq!(counter1.tombstones(), counter2.tombstones());
    assert_eq!(counter1.tombstones(), counter3.tombstones());
}


#[test]
fn test_add_site() {
    let mut counter = Counter::from_state(Counter::new(0).clone_state(), None).unwrap();

    assert!(counter.increment(1).is_err());
    assert!(counter.increment(-2).is_err());
    assert!(counter.increment(3).is_err());

    let ops = counter.add_site(123).unwrap();
    assert_eq!(counter.site(), 123);

    assert_eq!(ops[0].site(), 123);
    assert_eq!(ops[1].site(), 123);
    assert_eq!(ops[2].site(), 123);
}

#[test]
fn test_add_site_already_has_site() {
    let mut counter = Counter::from_state(Counter::new(123).clone_state(), Some(2)).unwrap();
    assert_eq!(counter.add_site(123), Err(Error::AlreadyHasSite));
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
    let local_op = counter2.execute_remote(&op).unwrap();
    test_serde(local_op);
}
