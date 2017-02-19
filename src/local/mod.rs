use Replica;
use Value;
use serde::ser::{Serialize, Serializer, SerializeMap};
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

impl Serialize for LocalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match *self {
            LocalValue::Obj(ref hashmap) => {
                let mut obj = serializer.serialize_map(Some(hashmap.len()))?;
                for (key, value) in hashmap {
                    let encoded_key = key.replace("~","~0").replace("__TYPE__","~1");
                    obj.serialize_entry(&encoded_key, value)?;
                }
                obj.end()
            },
            LocalValue::AttrStr(ref string) => {
                let mut obj = serializer.serialize_map(Some(2))?;
                obj.serialize_entry("__TYPE__", "attrstr")?;
                obj.serialize_entry("text", string)?;
                obj.end()
            },
            LocalValue::Arr(ref array) =>
                serializer.serialize_some(array),
            LocalValue::Str(ref string) =>
                serializer.serialize_str(string),
            LocalValue::Num(number) =>
                serializer.serialize_f64(number),
            LocalValue::Bool(bool_value) =>
                serializer.serialize_bool(bool_value),
            LocalValue::Null =>
                serializer.serialize_unit(),
        }
    }
}
