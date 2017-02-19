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
