use Object;
use Array;
use AttributedString;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;

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

    pub fn get_nested<'a>(&'a self, pointer: &str) -> Option<&'a Value> {
        let mut value = Some(self);

        for escaped_key in pointer.split("/").skip(1) {
            let key = escaped_key.replace("~1", "/").replace("~0", "~");
            value = match value {
                Some(&Value::Obj(ref object)) =>
                    object
                    .get_by_key(&key)
                    .and_then(|e| Some(& e.value)),
                Some(&Value::Arr(ref array)) =>
                    usize::from_str(&key)
                    .ok()
                    .and_then(|index| array.get_by_index(index))
                    .and_then(|e| Some(&e.value)),
                _ =>
                    return None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use object::Object;
    use array::Array;
    use Replica;

    #[test]
    fn test_get_nested_trivial() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Num(3.2),
            Value::Str("hello".to_string()),
            Value::attrstr(),
            Value::array(),
            Value::object()];

        for v in values {
            println!("{:?}", v.get_nested(""));
            assert!(v.get_nested("") == Some(&v));
        }
    }

    #[test]
    fn test_get_nested() {
        let replica = Replica::new(1,1);
        let mut object = Object::new();

        // insert a value whose key is empty
        let bool_value = Value::Bool(true);
        object.put("", bool_value.clone(), &replica);

        // insert a value whose key contains '/'
        let num_value = Value::Num(1.0);
        object.put("/", num_value.clone(), &replica);

        // insert a nested array
        let mut array = Array::new();
        let array_0 = Value::Num(1.0);
        let array_1 = Value::Num(2.0);
        array.insert(0, array_0.clone(), &replica);
        array.insert(1, array_1.clone(), &replica);
        let array = Value::Arr(array);
        object.put("101", array.clone(), &replica);

        // insert a nested attribute string
        let attrstr = Value::attrstr();
        object.put("a", attrstr.clone(), &replica);

        // insert a nested object
        let nested_object = Value::object();
        object.put("a%b", nested_object.clone(), &replica);

        let value = Value::Obj(object);
        assert!(value.get_nested("") == Some(&value));
        assert!(value.get_nested("/") == Some(&bool_value));
        assert!(value.get_nested("/~1") == Some(&num_value));
        assert!(value.get_nested("/101") == Some(&array));
        assert!(value.get_nested("/101/0") == Some(&array_0));
        assert!(value.get_nested("/101/1") == Some(&array_1));
        assert!(value.get_nested("/a") == Some(&attrstr));
        assert!(value.get_nested("/a%b") == Some(&nested_object));

        assert!(value.get_nested("/asdf") == None);
        assert!(value.get_nested("/~1/a") == None);
        assert!(value.get_nested("/101/-1") == None);
        assert!(value.get_nested("/101/2") == None);
    }
}
