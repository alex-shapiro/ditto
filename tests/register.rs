extern crate ditto;

mod common;
use ditto::Error;
use ditto::register::*;

#[test]
fn test_new() {
    let register = Register::new(8142i64);
    assert_eq!(register.get(), &8142);
    assert_eq!(register.site_id(), 1);
    assert_eq!(register.summary().get(1), 1);
}

#[test]
fn test_new_with_id() {
    let register = Register::new_with_id(8142i64,5);
    assert_eq!(register.site_id(), 5);
}

#[test]
fn test_update() {
    let mut register = Register::new(8142i64);
    let op = register.update(42).unwrap();

    assert_eq!(register.get(), &42);
    assert_eq!(register.summary().get(1), 2);
    assert_eq!(op.site_id(), 1);
    assert_eq!(op.counter(), 2);
    assert_eq!(op.value(), &42);
    assert_eq!(op.removed_dots(), []);
}

#[test]
fn test_execute_op() {
    let mut register1 = Register::new("a");
    let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();
    let op = register1.update("b").unwrap();

    assert_eq!(register2.execute_op(op), &"b");
    assert_eq!(register2.state(), register1.state());
}

#[test]
fn test_execute_op_concurrent() {
    let mut register1 = Register::new("a");
    let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();
    let mut register3 = Register::from_state(register1.clone_state(), Some(3)).unwrap();

    let op1 = register1.update("b").unwrap();
    let op2 = register2.update("c").unwrap();
    let op3 = register3.update("d").unwrap();

    assert_eq!(register1.execute_op(op2.clone()), &"b");
    assert_eq!(register1.execute_op(op3.clone()), &"b");
    assert_eq!(register2.execute_op(op3), &"c");
    assert_eq!(register2.execute_op(op1.clone()), &"b");
    assert_eq!(register3.execute_op(op2), &"c");
    assert_eq!(register3.execute_op(op1), &"b");

    assert_eq!(register1.state(), register2.state());
    assert_eq!(register1.state(), register3.state());
}

#[test]
fn test_execute_remote_dupe() {
    let mut register1 = Register::new("a");
    let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();
    let op = register1.update("b").unwrap();

    assert_eq!(register2.execute_op(op.clone()), &"b");
    assert_eq!(register2.execute_op(op), &"b");
    assert_eq!(register1.state(), register2.state());
}

#[test]
fn test_merge() {
    let mut register1 = Register::new("a");
    let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();
    let _ = register1.update("b");
    let _ = register2.update("c");

    let r1_state = register1.clone_state();
    register1.merge(register2.state());
    register2.merge(r1_state);
    assert_eq!(register1.state(), register2.state());
}

#[test]
fn test_add_site_id() {
    let mut register1 = Register::new(123);
    let mut register2 = Register::from_state(register1.clone_state(), None).unwrap();
    assert_eq!(register2.update(456).unwrap_err(), Error::AwaitingSiteId);

    let op = register2.add_site_id(2).unwrap().unwrap();
    assert_eq!(register2.site_id(), 2);
    assert_eq!(register1.execute_op(op), &456);
    assert_eq!(register1.state(), register2.state());
}

#[test]
fn test_add_site_id_already_has_site() {
    let register1 = Register::new(123);
    let mut register2 = Register::from_state(register1.state(), Some(42)).unwrap();
    assert_eq!(register2.add_site_id(44), Err(Error::AlreadyHasSiteId));
}

#[test]
fn test_serialize() {
    common::test_serde(Register::new("hello".to_owned()))
}

#[test]
fn test_serialize_state() {
    common::test_serde(Register::new("hello".to_owned()).into_state())
}

#[test]
fn test_serialize_op() {
    let mut register = Register::new(123);
    common::test_serde(register.update(456).unwrap());
}
