#[macro_use]
extern crate serde_json;
extern crate ditto;

mod common;
use ditto::json::*;

#[test]
fn test_serialize() {
    let crdt = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
    let state = crdt.clone_state();
    common::test_serde(crdt);
    common::test_serde(state);
}

#[test]
fn test_serialize_op() {
    let mut crdt = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
    let op = crdt.insert("/foo/0", json!({
        "a": [[1.0],["hello everyone!"],{"x": 3.0}],
        "b": {"cat": true, "dog": false}
    })).unwrap();

    common::test_serde(op);
}

#[test]
fn test_serialize_local_op() {
    let local_op = LocalOp::Insert{
        pointer: vec![LocalUid::Array(123), LocalUid::Object("abcd".into())],
        value: json!(["a", 1, "b", true, "c", {}, "d"]),
    };

    common::test_serde(local_op);
}
