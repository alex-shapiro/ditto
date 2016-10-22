use object::uid::UID;
use Replica;
use Value;

#[derive(Clone,PartialEq,Serialize,Deserialize)]
pub struct Element {
    #[serde(rename = "u")]
    pub uid: UID,
    #[serde(rename = "v")]
    pub value: Value,
}

impl Element {
    pub fn new(key: &str, value: Value, replica: &Replica) -> Element {
        Element{uid: UID::new(key, replica), value: value}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;
    use Value;
    use serde_json;

    #[test]
    fn test_new() {
        let replica = Replica{site: 1, counter: 1};
        let val = Value::Str("bar".to_string());
        let elt = Element::new("foo", val, &replica);
        assert!(elt.value == Value::Str("bar".to_string()));
    }

    #[test]
    fn test_serialize_deserialize() {
        let element = Element::new("hey",Value::Bool(true), &Replica{site: 1, counter: 4});
        let serialized = serde_json::to_string(&element).unwrap();
        let deserialized: Element = serde_json::from_str(&serialized).unwrap();
        assert!(serialized == r#"{"u":"AQQ,hey","v":true}"#);
        assert!(deserialized == element);
    }
}
