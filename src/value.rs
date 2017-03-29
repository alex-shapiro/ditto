use array::Array;
use attributed_string::AttributedString;
use counter::Counter;
use Error;
use object::{self, Object};
use op::{RemoteOp, LocalOp};
use Replica;
use sequence;
use std::str::FromStr;
use serde_json;

#[derive(Debug,PartialEq,Clone)]
pub enum Value {
    Obj(Object),
    Arr(Array),
    AttrStr(AttributedString),
    Counter(Counter),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

pub trait IntoValue {
    fn into_value(self) -> Result<Value, Error>;
}

impl Value {
    pub fn get_nested_local(&mut self, pointer: &str) -> Result<(&mut Value, String), Error> {
        let mut value = Some(self);
        let mut remote_pointer = String::new();

        if !(pointer.is_empty() || pointer.starts_with("/")) { return Err(Error::InvalidPath) }
        for key in pointer.split("/").skip(1) {
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
                    return Err(Error::InvalidPath),
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
                    return Err(Error::InvalidPath)
                }
            }
        }
        Ok((value.unwrap(), local_pointer))
    }

    pub fn execute_local(&mut self, local_op: LocalOp, replica: &Replica) -> Result<RemoteOp, Error> {
        match local_op {
            LocalOp::Put(op)         => self.put(&op.key, op.value.to_value(replica), replica),
            LocalOp::Delete(op)      => self.delete(&op.key),
            LocalOp::InsertItem(op)  => self.insert_item(op.index, op.value.to_value(replica), replica),
            LocalOp::DeleteItem(op)  => self.delete_item(op.index),
            LocalOp::InsertText(op)  => self.insert_text(op.index, op.text, replica),
            LocalOp::DeleteText(op)  => self.delete_text(op.index, op.len, replica),
            LocalOp::ReplaceText(op) => self.replace_text(op.index, op.len, op.text, replica),
            LocalOp::Increment(op)   => self.increment(op.amount, replica),
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
            (&mut Value::Counter(ref mut counter), &RemoteOp::IncrementCounter(ref op)) =>
                Ok(counter.execute_remote(op).map_or(vec![], |local_op| vec![local_op])),
            _ =>
                Err(Error::InvalidRemoteOp),
        }
    }

    pub fn update_site(&mut self, remote_op: &RemoteOp, site: u32) -> Result<(), Error> {
        match (self, remote_op) {
            (&mut Value::Obj(ref mut object), &RemoteOp::UpdateObject(ref op)) =>
                Ok(object.update_site(op, site)),

            (&mut Value::Arr(ref mut array), &RemoteOp::UpdateArray(ref op)) =>
                Ok(array.update_site(op, site)),

            (&mut Value::AttrStr(ref mut attrstr), &RemoteOp::UpdateAttributedString(ref op)) =>
                Ok(attrstr.update_site(op, site)),

            (&mut Value::Counter(ref mut counter), &RemoteOp::IncrementCounter(ref op)) =>
                Ok(counter.update_site(op, site)),

            _ =>
                Err(Error::InvalidPath),
        }
    }

    fn put(&mut self, key: &str, value: Self, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::Obj(ref mut o) => Ok(RemoteOp::UpdateObject(o.put(key, value, replica))),
            _ => Err(Error::InvalidLocalOp),
        }
    }

    fn delete(&mut self, key: &str) -> Result<RemoteOp, Error> {
        match *self {
            Value::Obj(ref mut o) => Ok(RemoteOp::UpdateObject(o.delete(key)?)),
            _ => Err(Error::InvalidLocalOp),
        }
    }

    fn insert_item(&mut self, index: usize, value: Self, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::Arr(ref mut a) => Ok(RemoteOp::UpdateArray(a.insert(index, value, replica)?)),
            _ => Err(Error::InvalidLocalOp)
        }
    }

    fn delete_item(&mut self, index: usize) -> Result<RemoteOp, Error> {
        match *self {
            Value::Arr(ref mut a) => Ok(RemoteOp::UpdateArray(a.delete(index)?)),
            _ => Err(Error::InvalidLocalOp)
        }
    }

    fn insert_text(&mut self, index: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::AttrStr(ref mut s) => Ok(RemoteOp::UpdateAttributedString(s.insert_text(index, text, replica)?)),
            _ => Err(Error::InvalidLocalOp)
        }
    }

    fn delete_text(&mut self, index: usize, len: usize, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::AttrStr(ref mut s) => Ok(RemoteOp::UpdateAttributedString(s.delete_text(index, len, replica)?)),
            _ => Err(Error::InvalidLocalOp)
        }
    }

    fn replace_text(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::AttrStr(ref mut s) => Ok(RemoteOp::UpdateAttributedString(s.replace_text(index, len, text, replica)?)),
            _ => Err(Error::InvalidLocalOp)
        }
    }

    fn increment(&mut self, amount: f64, replica: &Replica) -> Result<RemoteOp, Error> {
        match *self {
            Value::Counter(ref mut c) => Ok(RemoteOp::IncrementCounter(c.increment(amount, replica))),
            _ => Err(Error::InvalidLocalOp)
        }
    }
}

impl IntoValue for Value {
    fn into_value(self) -> Result<Value, Error> { Ok(self) }
}

impl<'a> IntoValue for &'a str {
    fn into_value(self) -> Result<Value, Error> {
        Ok(serde_json::from_str(self)?)
    }
}

impl<'a> IntoValue for &'a String {
    fn into_value(self) -> Result<Value, Error> {
        Ok(serde_json::from_str(self)?)
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
        let mut root = Value::Arr(Array::new());
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
        let mut root = Value::Arr(Array::new());
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
        local_value.to_value(&Replica::new(1,1))
    }
}
