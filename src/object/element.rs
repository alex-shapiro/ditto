use object::uid::UID;
use Value;

pub struct Element {
    uid: UID,
    value: Value,
}

impl Element {
    pub fn new(key: &str, value: Value, site: u32, counter: u32) -> Element {
        let uid = UID::new(key, site, counter);
        Element{uid: uid, value: value}
    }
}

#[test]
fn test_new() {
    let val = Value::Str("bar".to_string());
    let elt = Element::new("foo", val, 1, 1);
    assert!(elt.value == Value::Str("bar".to_string()));
}
