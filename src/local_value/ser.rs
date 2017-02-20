use super::LocalValue;
use serde::ser::{Serialize, Serializer, SerializeMap};

impl Serialize for LocalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match *self {
            LocalValue::Obj(ref hashmap) => {
                let mut obj = serializer.serialize_map(Some(hashmap.len()))?;
                for (key, value) in hashmap {
                    obj.serialize_entry(key, value)?;
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
