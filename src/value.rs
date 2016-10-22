use object::Object;
use array::Array;
use attributed_string::AttributedString;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;
use op::remote::IncrementNumber;
use serde;

#[derive(PartialEq,Clone)]
pub enum Value {
    Obj(Object),
    Arr(Array),
    AttrStr(AttributedString),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl Value {
    pub fn object() -> Self {
        Value::Obj(Object::new())
    }

    pub fn array() -> Self {
        Value::Arr(Array::new())
    }

    pub fn attrstr() -> Self {
        Value::AttrStr(AttributedString::new())
    }

    pub fn as_object<'a>(&'a mut self) -> Option<&'a mut Object> {
        match *self {
            Value::Obj(ref mut object) => Some(object),
            _ => None,
        }
    }

    pub fn as_array<'a>(&'a mut self) -> Option<&'a mut Array> {
        match *self {
            Value::Arr(ref mut array) => Some(array),
            _ => None,
        }
    }

    pub fn as_attributed_string<'a>(&'a mut self) -> Option<&'a mut AttributedString> {
        match *self {
            Value::AttrStr(ref mut string) => Some(string),
            _ => None,
        }
    }

    pub fn increment<'a>(&'a mut self, amount: f64) -> Option<IncrementNumber> {
        match *self {
            Value::Num(ref mut n) => {
                *n += amount;
                Some(IncrementNumber::new(amount)) },
            _ => None,
        }
    }

    pub fn get_nested<'a>(&'a mut self, pointer: &str) -> Option<&'a mut Value> {
        let mut value = Some(self);

        for escaped_key in pointer.split("/").skip(1) {
            let key = escaped_key.replace("~1", "/").replace("~0", "~");
            if value.is_none() { return None }
            value = match *value.unwrap() {
                Value::Obj(ref mut object) =>
                    object
                    .get_by_key(&key)
                    .and_then(|e| Some(&mut e.value)),
                Value::Arr(ref mut array) => {
                    let index = usize::from_str(&key).ok();
                    if index.is_some() {
                        array.get_by_index(index.unwrap()).and_then(|e| Some(&mut e.value))
                    } else {
                        None
                    }
                },
                _ =>
                    None,
            }
        }
        value
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Value::Obj(_) =>
                write!(f, "<object>"),
            &Value::Arr(_) =>
                write!(f, "<array>"),
            &Value::AttrStr(_) =>
                write!(f, "<attributed string>"),
            &Value::Str(ref str) =>
                write!(f, "\"{}\">", str),
            &Value::Num(n) =>
                write!(f, "{}", n),
            &Value::Bool(b) =>
                write!(f, "{}", b),
            &Value::Null =>
                write!(f, "null"),
        }
    }
}

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
    where S: serde::Serializer {
        match *self {
            Value::Obj(_) =>
                serializer.serialize_none(),
            Value::Arr(_) =>
                serializer.serialize_none(),
            Value::AttrStr(_) =>
                serializer.serialize_none(),
            Value::Str(ref string) =>
                serializer.serialize_str(string),
            Value::Num(number) =>
                serializer.serialize_f64(number),
            Value::Bool(boolean) =>
                serializer.serialize_bool(boolean),
            Value::Null =>
                serializer.serialize_none(),
        }
    }
}

impl serde::Deserialize for Value {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: serde::Deserializer {

        struct ValueVisitor;
        impl serde::de::Visitor for ValueVisitor {
            type Value = Value;
            fn visit_none<E>(&mut self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_bool<E>(&mut self, value: bool) -> Result<Value, E> {
                Ok(Value::Bool(value))
            }
            fn visit_f64<E>(&mut self, value: f64) -> Result<Value, E> {
                Ok(Value::Num(value))
            }
            fn visit_str<E>(&mut self, value: &str) -> Result<Value, E>
                where E: serde::de::Error {
                self.visit_string(String::from(value))
            }
            fn visit_string<E>(&mut self, value: String) -> Result<Value, E> {
                Ok(Value::Str(value))
            }

            fn visit_map<V>(&mut self, visitor: V) -> Result<Value, V::Error>
                where V: serde::de::MapVisitor {
                Ok(Value::object())
                // let maptype: Option<String> = None;
                // let elements:
                // let map = try!(visitor.visit());
                // let maptype = map.get("t");
                // let elements = map.get("e").unwrap_or
                // match map.get("t") {
                //     "attrstr" =>
                //         try!(AttributedStringVisitor::new().visit_map(visitor)),
                //     _ =>
                // }
                // let values = try!(MapVisitor::new().visit_map(visitor));
                // Ok(Value::Obj(values))
            }
        }

        deserializer.deserialize(ValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object::Object;
    use array::Array;
    use Replica;

    #[test]
    fn test_get_nested_trivial() {
        let mut values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Num(3.2),
            Value::Str("hello".to_string()),
            Value::attrstr(),
            Value::array(),
            Value::object()];

        for v in &mut values {
            assert!(v.clone().get_nested("") == Some(v));
        }
    }

    #[test]
    fn test_get_nested() {
        let replica = Replica::new(1,1);
        let mut object = Object::new();

        // insert a value whose key is empty
        let mut bool_value = Value::Bool(true);
        object.put("", bool_value.clone(), &replica);

        // insert a value whose key contains '/'
        let mut num_value = Value::Num(1.0);
        object.put("/", num_value.clone(), &replica);

        // insert a nested array
        let mut array = Array::new();
        let mut array_0 = Value::Num(1.0);
        let mut array_1 = Value::Num(2.0);
        array.insert(0, array_0.clone(), &replica);
        array.insert(1, array_1.clone(), &replica);
        let mut array = Value::Arr(array);
        object.put("101", array.clone(), &replica);

        // insert a nested attribute string
        let mut attrstr = Value::attrstr();
        object.put("a", attrstr.clone(), &replica);

        // insert a nested object
        let mut nested_object = Value::object();
        object.put("a%b", nested_object.clone(), &replica);

        let mut value = Value::Obj(object);
        assert!(value.get_nested("/") == Some(&mut bool_value));
        assert!(value.get_nested("/~1") == Some(&mut num_value));
        assert!(value.get_nested("/101") == Some(&mut array));
        assert!(value.get_nested("/101/0") == Some(&mut array_0));
        assert!(value.get_nested("/101/1") == Some(&mut array_1));
        assert!(value.get_nested("/a") == Some(&mut attrstr));
        assert!(value.get_nested("/a%b") == Some(&mut nested_object));

        assert!(value.get_nested("/asdf") == None);
        assert!(value.get_nested("/~1/a") == None);
        assert!(value.get_nested("/101/-1") == None);
        assert!(value.get_nested("/101/2") == None);
    }
}
