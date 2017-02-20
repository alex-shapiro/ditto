//! LocalValue is a strongly-typed intermediate value between
//! user-generated JSON and CRDT values.

mod ser;
mod de;

use Replica;
use Value;
use object::Object;
use attributed_string::AttributedString;
use array::Array;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
enum LocalValue {
    Obj(HashMap<String, LocalValue>),
    AttrStr(String),
    Arr(Vec<LocalValue>),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl LocalValue {
    pub fn to_value(self, replica: &Replica) -> Value {
        match self {
            LocalValue::Obj(map) => {
                let mut object = Object::new();
                for (key, local_value) in map.into_iter() {
                    object.put(&key, local_value.to_value(replica), replica);
                }
                Value::Obj(object)
            },
            LocalValue::AttrStr(string) => {
                let mut attrstr = AttributedString::new();
                let _ = attrstr.insert_text(0, string, replica);
                Value::AttrStr(attrstr)
            },
            LocalValue::Arr(items) => {
                let mut array = Array::new();
                for (i, local_value) in items.into_iter().enumerate() {
                    let _ = array.insert(i, local_value.to_value(replica), replica);
                }
                Value::Arr(array)
            },
            LocalValue::Str(string) =>
                Value::Str(string),
            LocalValue::Num(number) =>
                Value::Num(number),
            LocalValue::Bool(bool_value) =>
                Value::Bool(bool_value),
            LocalValue::Null =>
                Value::Null,
        }
    }
}

impl From<Value> for LocalValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Obj(object) => {
                let mut map: HashMap<String, LocalValue> = HashMap::new();
                for (key, elements) in object.into_elements() {
                    let value = elements.into_iter().min_by_key(|e| e.uid.site).unwrap().value;
                    map.insert(key, value.into());
                };
                LocalValue::Obj(map)
            },
            Value::Arr(array) => {
                let items: Vec<LocalValue> =
                    array
                    .into_elements()
                    .into_iter()
                    .map(|e| e.value.into())
                    .collect();
                LocalValue::Arr(items)
            },
            Value::AttrStr(attrstr) =>
                LocalValue::AttrStr(attrstr.to_string()),
            Value::Str(string) =>
                LocalValue::Str(string),
            Value::Num(number) =>
                LocalValue::Num(number),
            Value::Bool(bool_value) =>
                LocalValue::Bool(bool_value),
            Value::Null =>
                LocalValue::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    const REPLICA: Replica = Replica{site: 5, counter: 8};

    #[test]
    fn test_null() {
        let original = LocalValue::Null;
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "null");
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        assert!(value == Value::Null);

        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_bool() {
        let original = LocalValue::Bool(true);
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "true");
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        assert!(value == Value::Bool(true));

        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_number() {
        let original = LocalValue::Num(843.0);
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str("843").unwrap();
        assert!(encoded == "843.0");
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        assert!(value == Value::Num(843.0));

        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_string() {
        let original = LocalValue::Str("hi!".to_owned());
        let encoded  = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "\"hi!\"");
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        assert!(value == Value::Str("hi!".to_owned()));

        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_attrstr() {
        let original = LocalValue::AttrStr("The quick fox".to_owned());
        let encoded  = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == r#"{"__TYPE__":"attrstr","text":"The quick fox"}"#);
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_array() {
        let original = LocalValue::Arr(vec![LocalValue::Num(1.3), LocalValue::Num(2.4)]);
        let encoded  = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "[1.3,2.4]");
        assert!(decoded == original);

        let value = decoded.to_value(&REPLICA);
        let from_value: LocalValue = value.into();
        assert!(from_value == original);
    }

    #[test]
    fn test_object() {
        let mut map = HashMap::new();
        map.insert("foo".to_owned(), LocalValue::Str("x".to_owned()));
        map.insert("bar".to_owned(), LocalValue::Num(-483.8));
        map.insert("baz".to_owned(), LocalValue::AttrStr("Hello!".to_owned()));

        let original = LocalValue::Obj(map);
        let encoded  = serde_json::to_string(&original).unwrap();
        let decoded: LocalValue = serde_json::from_str(&encoded).unwrap();

        let encoded1: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        let encoded2: serde_json::Value = serde_json::from_str(r#"{"foo":"x","bar":-483.8,"baz":{"__TYPE__":"attrstr","text":"Hello!"}}"#).unwrap();

        assert!(encoded1 == encoded2);
        assert!(decoded == original);
    }

    #[test]
    fn test_invalid_special_type() {
        assert!(serde_json::from_str::<LocalValue>(r#"{"__TYPE__":"mytype"}"#).is_err());
    }

    #[test]
    fn test_invalid_attrstr() {
        assert!(serde_json::from_str::<LocalValue>(r#"{"__TYPE__":"attrstr"}"#).is_err());
        assert!(serde_json::from_str::<LocalValue>(r#"{"__TYPE__":"attrstr","text": 3}"#).is_err());
    }
}
