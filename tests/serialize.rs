extern crate ditto;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use ditto::{CRDT, Value};

#[test]
fn serialize_value_alone() {
    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":"The quick fox ran over the lazy dog."}"#).unwrap();
    let value: &Value = crdt.value();
    let json = serde_json::to_string(value).unwrap();
    assert!(json == r#"{"__TYPE__":"attrstr","text":"The quick fox ran over the lazy dog."}"#);
}

#[test]
fn serialize_value_inside_struct() {
    #[derive(Serialize)]
    struct SomeStruct<'a> {
        afield: &'a str,
        value: &'a Value,
    }

    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":"abcdefg"}"#).unwrap();
    let s = SomeStruct{
        afield: "hiya!",
        value: crdt.value(),
    };

    let json = serde_json::to_string(&s).unwrap();
    assert!(json == r#"{"afield":"hiya!","value":{"__TYPE__":"attrstr","text":"abcdefg"}}"#)
}
