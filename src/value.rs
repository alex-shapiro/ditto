use array::Array;
use attributed_string::AttributedString;
use Error;
use object::{self, Object};
use op::remote::IncrementNumber;
use op::{self, RemoteOp, LocalOp};
use Replica;
use sequence;
use std::str::FromStr;

#[derive(Debug,PartialEq,Clone)]
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

    pub fn get_nested_local(&mut self, pointer: &str) -> Result<(&mut Value, String), Error> {
        let mut value = Some(self);
        let mut remote_pointer = String::new();

        if !(pointer.is_empty() || pointer.starts_with("/")) { return Err(Error::InvalidPath) }
        for escaped_key in pointer.split("/").skip(1) {
            let key = escaped_key.replace("~1", "/").replace("~0", "~");
            value = match *value.unwrap() {
                Value::Obj(ref mut object) => {
                    let mut element = object.get_by_key(&key)?;
                    remote_pointer.push('/');
                    remote_pointer.push_str(&element.uid.to_string());
                    Some(&mut element.value)
                },
                Value::Arr(ref mut array) => {
                    let index = usize::from_str(&key)?;
                    let element = array.get_by_index(index)?;
                    remote_pointer.push('/');
                    remote_pointer.push_str(&element.uid.to_string());
                    Some(&mut element.value)
                },
                _ =>
                    return Err(Error::ValueMismatch("pointer")),
            }
        }
        Ok((value.unwrap(), remote_pointer))
    }

    pub fn get_nested_remote(&mut self, pointer: &str) -> Result<(&mut Value, String), Error> {
        let mut value = Some(self);
        let mut local_pointer = String::new();

        if !(pointer.is_empty() || pointer.starts_with("/")) { return Err(Error::InvalidPath) }
        for encoded_uid in pointer.split("/").skip(1) {
            value = match *value.unwrap() {
                Value::Obj(ref mut object) => {
                    let uid = object::UID::from_str(encoded_uid)?;
                    let mut element = object.get_by_uid(&uid)?;
                    local_pointer.push('/');
                    local_pointer.push_str(&uid.key);
                    Some(&mut element.value)
                },
                Value::Arr(ref mut array) => {
                    let uid = sequence::uid::UID::from_str(encoded_uid)?;
                    let (mut element, index) = array.get_by_uid(&uid)?;
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

    pub fn execute_local(&mut self, op: LocalOp, replica: &Replica) -> Result<RemoteOp, Error> {
        match op {
            LocalOp::Put(op) => {
                let mut object = self.as_object()?;
                let remote_op = object.put(&op.key, op.value.to_value(replica), replica);
                Ok(RemoteOp::UpdateObject(remote_op))
            },
            LocalOp::Delete(op) => {
                let mut object = self.as_object()?;
                let remote_op = object.delete(&op.key)?;
                Ok(RemoteOp::UpdateObject(remote_op))
            },
            LocalOp::InsertItem(op) => {
                let mut array = self.as_array()?;
                let remote_op = array.insert(op.index, op.value.to_value(replica), replica)?;
                Ok(RemoteOp::UpdateArray(remote_op))
            },
            LocalOp::DeleteItem(op) => {
                let mut array = self.as_array()?;
                let remote_op = array.delete(op.index)?;
                Ok(RemoteOp::UpdateArray(remote_op))
            },
            LocalOp::InsertText(op) => {
                let mut attrstr = self.as_attributed_string()?;
                let remote_op = attrstr.insert_text(op.index, op.text, replica)?;
                Ok(RemoteOp::UpdateAttributedString(remote_op))
            },
            LocalOp::DeleteText(op) => {
                let mut attrstr = self.as_attributed_string()?;
                let remote_op = attrstr.delete_text(op.index, op.len, replica)?;
                Ok(RemoteOp::UpdateAttributedString(remote_op))
            },
            LocalOp::ReplaceText(op) => {
                let mut attrstr = self.as_attributed_string()?;
                let remote_op = attrstr.replace_text(op.index, op.len, op.text, replica)?;
                Ok(RemoteOp::UpdateAttributedString(remote_op))
            },
            LocalOp::IncrementNumber(op) => {
                let remote_op = self.increment(op.amount)?;
                Ok(RemoteOp::IncrementNumber(remote_op))
            },
        }
    }

    pub fn execute_remote(&mut self, remote_op: &RemoteOp) -> Result<Vec<LocalOp>, Error> {
        match (self, remote_op) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use array::Array;
    use object::Object;
    use Replica;
    use LocalValue;
    use serde_json;

    const REPLICA: Replica = Replica{site: 1, counter: 1};

    #[test]
    fn test_get_nested_local_root() {
        for v in &mut test_values() {
            assert!(v.clone().get_nested_local("") == Ok((v, String::new())));
        }
    }

    #[test]
    fn test_get_nested_local_object() {
        for v in &mut test_values() {
            let mut object = Object::new();
            let op = object.put("foo", v.clone(), &REPLICA);
            let uid = &op.inserts[0].uid;
            let mut root = Value::Obj(object);
            let remote_pointer = format!("/{}", uid.to_string());
            assert!(root.get_nested_local("/foo") == Ok((v, remote_pointer)));
        }
    }

    #[test]
    fn test_get_nested_local_array() {
        for v in &mut test_values() {
            let mut array = Array::new();
            let op = array.insert(0, v.clone(), &REPLICA).ok().unwrap();
            let ref uid = op.inserts[0].uid;
            let mut root = Value::Arr(array);
            let remote_pointer = format!("/{}", uid.to_string());
            assert!(root.get_nested_local("/0") == Ok((v, remote_pointer)));
        }
    }

    #[test]
    fn test_get_nested_local_object_array() {
        for v in &mut test_values() {
            let mut object = Object::new();
            let mut array  = Array::new();
            let op1 = array.insert(0, v.clone(), &REPLICA).ok().unwrap();
            let op2 = object.put("bar", Value::Arr(array), &REPLICA);
            let remote_pointer = {
                let uid2 = op2.inserts[0].uid.to_string();
                let uid1 = op1.inserts[0].uid.to_string();
                format!("/{}/{}", uid2, uid1)
            };
            let mut root = Value::Obj(object);
            assert!(root.get_nested_local("/bar/0") == Ok((v, remote_pointer)));
        }
    }

    #[test]
    fn test_get_nested_local_array_object() {
        for v in &mut test_values() {
            let mut array  = Array::new();
            let mut object = Object::new();
            let op1 = object.put("baz", v.clone(), &REPLICA);
            let op2 = array.insert(0, Value::Obj(object), &REPLICA).ok().unwrap();
            let remote_pointer = {
                let uid2 = op2.inserts[0].uid.to_string();
                let uid1 = op1.inserts[0].uid.to_string();
                format!("/{}/{}", uid2, uid1)
            };
            let mut root = Value::Arr(array);
            assert!(root.get_nested_local("/0/baz") == Ok((v, remote_pointer)));
        }
    }

    #[test]
    fn test_get_nested_local_invalid_path() {
        let mut root = Value::array();
        assert!(root.get_nested_local("x/0") == Err(Error::InvalidPath));
    }

    #[test]
    fn test_get_nested_remote_root() {
        for v in &mut test_values() {
            assert!(v.clone().get_nested_remote("") == Ok((v, String::new())));
        }
    }

    #[test]
    fn test_get_nested_remote_object() {
        for v in &mut test_values() {
            let mut object = Object::new();
            let op = object.put("foo", v.clone(), &REPLICA);
            let uid = &op.inserts[0].uid;
            let mut root = Value::Obj(object);
            let remote_pointer = format!("/{}", uid.to_string());
            assert!(root.get_nested_remote(&remote_pointer) == Ok((v, "/foo".to_string())));
        }
    }

    #[test]
    fn test_get_nested_remote_array() {
        for v in &mut test_values() {
            let mut array = Array::new();
            let op = array.insert(0, v.clone(), &REPLICA).ok().unwrap();
            let ref uid = op.inserts[0].uid;
            let mut root = Value::Arr(array);
            let remote_pointer = format!("/{}", uid.to_string());
            assert!(root.get_nested_remote(&remote_pointer) == Ok((v, "/0".to_string())));
        }
    }

    #[test]
    fn test_get_nested_remote_object_array() {
        for v in &mut test_values() {
            let mut object = Object::new();
            let mut array  = Array::new();
            let op1 = array.insert(0, v.clone(), &REPLICA).ok().unwrap();
            let op2 = object.put("bar", Value::Arr(array), &REPLICA);
            let remote_pointer = {
                let uid2 = op2.inserts[0].uid.to_string();
                let uid1 = op1.inserts[0].uid.to_string();
                format!("/{}/{}", uid2, uid1)
            };
            let mut root = Value::Obj(object);
            assert!(root.get_nested_remote(&remote_pointer) == Ok((v, "/bar/0".to_string())));
        }
    }

    #[test]
    fn test_get_nested_remote_array_object() {
        for v in &mut test_values() {
            let mut array  = Array::new();
            let mut object = Object::new();
            let op1 = object.put("baz", v.clone(), &REPLICA);
            let op2 = array.insert(0, Value::Obj(object), &REPLICA).ok().unwrap();
            let remote_pointer = {
                let uid2 = op2.inserts[0].uid.to_string();
                let uid1 = op1.inserts[0].uid.to_string();
                format!("/{}/{}", uid2, uid1)
            };
            let mut root = Value::Arr(array);
            assert!(root.get_nested_remote(&remote_pointer) == Ok((v, "/0/baz".to_string())));
        }
    }

    #[test]
    fn test_get_nested_remote_invalid_path() {
        let mut root = Value::array();
        assert!(root.get_nested_remote("x/x") == Err(Error::InvalidPath));
    }

    fn test_values() -> Vec<Value> {
        vec![
            Value::Null,
            Value::Bool(false),
            Value::Num(-1423.8304),
            Value::Str("wysiwyg".to_string()),
            from_str(r#"{"__TYPE__":"attrstr", "text":"huh?"}"#),
            from_str(r#"[true, false, true]"#),
            from_str(r#"{"a":1, "b":2}"#),
        ]
    }

    fn from_str(string: &str) -> Value {
        let local_value: LocalValue = serde_json::from_str(string).expect("invalid JSON!");
        local_value.into_value(&Replica::new(1,1))
    }
}
