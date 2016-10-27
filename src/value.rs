use array::Array;
use attributed_string::AttributedString;
use Error;
use object::{self, Object};
use op::remote::IncrementNumber;
use op::{self, RemoteOp, LocalOp};
use sequence;
use std::fmt::{self, Debug};
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

    pub fn as_object<'a>(&'a mut self) -> Result<&'a mut Object, Error> {
        match *self {
            Value::Obj(ref mut object) => Ok(object),
            _ => Err(Error::ValueMismatch("object")),
        }
    }

    pub fn as_array<'a>(&'a mut self) -> Result<&'a mut Array, Error> {
        match *self {
            Value::Arr(ref mut array) => Ok(array),
            _ => Err(Error::ValueMismatch("array")),
        }
    }

    pub fn as_attributed_string<'a>(&'a mut self) -> Result<&'a mut AttributedString, Error> {
        match *self {
            Value::AttrStr(ref mut string) => Ok(string),
            _ => Err(Error::ValueMismatch("attrstr")),
        }
    }

    pub fn increment<'a>(&'a mut self, amount: f64) -> Result<IncrementNumber, Error> {
        match *self {
            Value::Num(ref mut n) => {
                *n += amount;
                Ok(IncrementNumber::new(amount))
            },
            _ => Err(Error::ValueMismatch("number")),
        }
    }

    pub fn increment_remote<'a>(&'a mut self, amount: f64) -> Result<Vec<LocalOp>, Error> {
        match *self {
            Value::Num(ref mut n) => {
                *n += amount;
                let op = op::local::IncrementNumber::new(amount);
                let op_wrapper = LocalOp::IncrementNumber(op);
                Ok(vec![op_wrapper])
            },
            _ => Err(Error::InvalidRemoteOp),
        }
    }

    pub fn get_nested<'a>(&'a mut self, pointer: &str) -> Result<&'a mut Value, Error> {
        let mut value = Some(self);

        for escaped_key in pointer.split("/").skip(1) {
            let key = escaped_key.replace("~1", "/").replace("~0", "~");
            if value.is_none() { return Err(Error::ValueMismatch("pointer")) }
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
        match value {
            Some(v) => Ok(v),
            None => Err(Error::ValueMismatch("pointer"))
        }
    }

    pub fn get_nested_remote(&mut self, pointer: &str) -> Result<(&mut Value, String), Error> {
        let mut value = Some(self);
        let mut local_pointer = String::new();

        for encoded_uid in pointer.split("/").skip(1) {
            value = match *value.unwrap() {
                Value::Obj(ref mut object) => {
                    let uid = try!(object::UID::from_str(encoded_uid));
                    let mut element = try!(object.get_by_uid(&uid));
                    local_pointer.push('/');
                    local_pointer.push_str(&uid.key);
                    Some(&mut element.value)
                },
                Value::Arr(ref mut array) => {
                    let uid = try!(sequence::uid::UID::from_str(encoded_uid));
                    let (mut element, index) = try!(array.get_by_uid(&uid));
                    local_pointer.push_str(&format!("/{}", index));
                    Some(&mut element.value)
                },
                _ => {
                    return Err(Error::ValueMismatch("pointer"))
                }
            }
        }
        Ok((value.unwrap(), local_pointer))
    }

    pub fn execute_remote(&mut self, op: &RemoteOp) -> Result<Vec<LocalOp>, Error> {
        match (self, op) {
            (&mut Value::Obj(ref mut object), &RemoteOp::UpdateObject(ref op)) =>
                Ok(vec![object.execute_remote(op)]),
            (&mut Value::Arr(ref mut array), &RemoteOp::UpdateArray(ref op)) =>
                Ok(array.execute_remote(op)),
            (&mut Value::AttrStr(ref mut attrstr), &RemoteOp::UpdateAttributedString(ref op)) =>
                Ok(attrstr.execute_remote(op)),
            (ref mut value @ &mut Value::Num(_), &RemoteOp::IncrementNumber(ref op)) =>
                value.increment_remote(op.amount),
            _ =>
                Err(Error::InvalidRemoteOp),
        }
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
    use array::Array;
    use Error;
    use object::Object;
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
            assert!(v.clone().get_nested("") == Ok(v));
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
        assert!(value.get_nested("/") == Ok(&mut bool_value));
        assert!(value.get_nested("/~1") == Ok(&mut num_value));
        assert!(value.get_nested("/101") == Ok(&mut array));
        assert!(value.get_nested("/101/0") == Ok(&mut array_0));
        assert!(value.get_nested("/101/1") == Ok(&mut array_1));
        assert!(value.get_nested("/a") == Ok(&mut attrstr));
        assert!(value.get_nested("/a%b") == Ok(&mut nested_object));

        assert!(value.get_nested("/asdf") == Err(Error::ValueMismatch("pointer")));
        assert!(value.get_nested("/~1/a") == Err(Error::ValueMismatch("pointer")));
        assert!(value.get_nested("/101/-1") == Err(Error::ValueMismatch("pointer")));
        assert!(value.get_nested("/101/2") == Err(Error::ValueMismatch("pointer")));
    }
}
