//! A CRDT that stores a JSON value.

use {Error, Replica, Tombstones};
use list::{self, ListValue};
use map::{self, MapValue};
use text::{self, TextValue};
use sequence;
use traits::*;

use serde_json::{self, Value as SJValue};
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Json {
    value: JsonValue,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonState<'a> {
    value: Cow<'a, JsonValue>,
    tombstones: Cow<'a, Tombstones>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JsonValue {
    Object(MapValue<String, JsonValue>),
    Array(ListValue<JsonValue>),
    String(TextValue),
    Number(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp {
    pointer: Vec<RemoteUID>,
    op: RemoteOpInner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteOpInner {
    Object(map::RemoteOp<String, JsonValue>),
    Array(list::RemoteOp<JsonValue>),
    String(text::RemoteOp),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteUID {
    Object(String, Replica),
    Array(sequence::uid::UID),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all="snake_case")]
pub enum LocalOp {
    Insert{pointer: Vec<LocalUID>, value: SJValue},
    Remove{pointer: Vec<LocalUID>},
    ReplaceText{pointer: Vec<LocalUID>, changes: Vec<text::LocalChange>},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocalUID {
    Object(String),
    Array(usize),
}

pub trait IntoJson {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error>;
}

impl Json {

    /// Constructs and returns a new `Json` CRDT from any value that
    /// satisfies the [`IntoJson`](IntoJson.t.html) trait.
    pub fn new<T: IntoJson>(local_value: T) -> Result<Self, Error> {
        let mut replica = Replica::new(1, 0);
        let value = local_value.into_json(&replica)?;
        let tombstones = Tombstones::new();
        replica.counter += 1;
        Ok(Json{value, replica, tombstones, awaiting_site: vec![]})
    }

    /// Constructs and returns a new `Json` CRDT from an unparsed
    /// JSON string. The CRDT has site 1.
    pub fn from_str(json_str: &str) -> Result<Self, Error> {
        let local_value: SJValue = serde_json::from_str(json_str)?;
        let crdt = Json::new(local_value)?;
        Ok(crdt)
    }

    /// Inserts a value into the Json CRDT at the given json pointer.
    /// The enclosing value may be an object or an array and the
    /// inserted value must satisfy the [`IntoJson`](IntoJson.t.html) trait.
    ///
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert<T: IntoJson>(&mut self, pointer: &str, value: T) -> Result<RemoteOp, Error> {
        let value = value.into_json(&self.replica)?;
        let op = self.value.insert(pointer, value, &self.replica)?;
        self.after_op(op)
    }

    /// Inserts a value into the Json CRDT at the given json pointer.
    /// The enclosing value may be an object or an array and the
    /// value being inserted is stringified JSON.
    ///
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert_str(&mut self, pointer: &str, value: &str) -> Result<RemoteOp, Error> {
        let json: SJValue = serde_json::from_str(&value)?;
        self.insert(pointer, json)
    }

    /// Removes a value at the given JSON pointer from the Json CRDT.
    /// If the enclosing value is an object, it deletes the key-value
    /// pair. If the enclosing value is an array, it deletes the value
    /// at the array index.
    ///
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, pointer: &str) -> Result<RemoteOp, Error> {
        let op = self.value.remove(pointer)?;
        self.after_op(op)
    }

    /// Replaces a text range in a text value in the Json CRDT.
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str) -> Result<RemoteOp, Error> {
        let op = self.value.replace_text(pointer, index, len, text, &self.replica)?;
        self.after_op(op)
    }

    crdt_impl!(Json, JsonState, JsonState, JsonState<'static>, JsonValue);
}

impl JsonValue {
    pub fn insert<T: IntoJson>(&mut self, pointer: &str, value: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let mut pointer = Self::split_pointer(pointer)?;
        let key = pointer.pop().ok_or(Error::DoesNotExist)?;
        let (json_value, remote_pointer) = self.get_nested_local(&pointer)?;
        let value = value.into_json(&replica)?;

        match *json_value {
            JsonValue::Object(ref mut map_value) => {
                let remote_op = map_value.insert(key.into(), value, &replica)?;
                let remote_op = RemoteOpInner::Object(remote_op);
                Ok(RemoteOp{pointer: remote_pointer, op: remote_op})
            }
            JsonValue::Array(ref mut list_value) => {
                let idx = usize::from_str(key)?;
                let remote_op = list_value.insert(idx, value, &replica)?;
                let remote_op = RemoteOpInner::Array(remote_op);
                Ok(RemoteOp{pointer: remote_pointer, op: remote_op})
            }
            _ => Err(Error::DoesNotExist),
        }
    }

    pub fn remove(&mut self, pointer: &str) -> Result<RemoteOp, Error> {
        let mut pointer = Self::split_pointer(pointer)?;
        let key = pointer.pop().ok_or(Error::DoesNotExist)?;
        let (json_value, remote_pointer) = self.get_nested_local(&pointer)?;

        match *json_value {
            JsonValue::Object(ref mut map_value) => {
                let remote_op = map_value.remove(key)?;
                Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Object(remote_op)})
            }
            JsonValue::Array(ref mut list_value) => {
                let idx = usize::from_str(key)?;
                let remote_op = list_value.remove(idx)?;
                Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Array(remote_op)})
            }
            _ => Err(Error::DoesNotExist),
        }
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        let pointer = Self::split_pointer(pointer)?;
        let (json_value, remote_pointer) = self.get_nested_local(&pointer)?;
        let text_value = json_value.as_text()?;
        let remote_op = text_value.replace(index, len, text, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::String(remote_op)})
    }

    pub fn execute_remote(&mut self, remote_op: &RemoteOp) -> Option<LocalOp> {
        let (json_value, mut pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
        match (json_value, &remote_op.op) {
            (&mut JsonValue::Object(ref mut map), &RemoteOpInner::Object(ref op)) => {
                match try_opt!(map.execute_remote(op)) {
                    map::LocalOp::Insert{key, value} => {
                        pointer.push(LocalUID::Object(key));
                        Some(LocalOp::Insert{pointer, value: value.local_value()})
                    }
                    map::LocalOp::Remove{key} => {
                        pointer.push(LocalUID::Object(key));
                        Some(LocalOp::Remove{pointer})
                    }
                }
            }
            (&mut JsonValue::Array(ref mut list), &RemoteOpInner::Array(ref op)) => {
                match try_opt!(list.execute_remote(op)) {
                    list::LocalOp::Insert{index, value} => {
                        pointer.push(LocalUID::Array(index));
                        Some(LocalOp::Insert{pointer, value: value.local_value()})
                    }
                    list::LocalOp::Remove{index} => {
                        pointer.push(LocalUID::Array(index));
                        Some(LocalOp::Remove{pointer})
                    }
                }
            }
            (&mut JsonValue::String(ref mut text), &RemoteOpInner::String(ref op)) => {
                let text_op = try_opt!(text.execute_remote(op));
                Some(LocalOp::ReplaceText{pointer, changes: text_op.0})
            }
            _ => None,
        }
    }

    pub fn merge(&mut self, other: JsonValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.nested_merge(other, self_tombstones, other_tombstones).unwrap()
    }

    fn split_pointer(pointer_str: &str) -> Result<Vec<&str>, Error> {
        if !(pointer_str.is_empty() || pointer_str.starts_with("/")) {
            return Err(Error::DoesNotExist)
        }
        Ok(pointer_str.split("/").skip(1).collect())
    }

    fn get_nested_local(&mut self, pointer: &[&str]) -> Result<(&mut JsonValue, Vec<RemoteUID>), Error> {
        let mut value = Some(self);
        let mut remote_pointer = vec![];

        for key in pointer {
            match value.unwrap() {
                &mut JsonValue::Object(ref mut map_value) => {
                    let element = map_value.get_mut(*key).ok_or(Error::DoesNotExist)?;
                    let uid = RemoteUID::Object((*key).to_owned(), element.0.clone());
                    remote_pointer.push(uid);
                    value = Some(&mut element.1)
                }
                &mut JsonValue::Array(ref mut list_value) => {
                    let index = usize::from_str(key)?;
                    let element = list_value.0.get_mut_elt(index)?.0;
                    let uid = RemoteUID::Array(element.0.clone());
                    remote_pointer.push(uid);
                    value = Some(&mut element.1)
                }
                _ => return Err(Error::DoesNotExist),
            }
        }

        Ok((value.unwrap(), remote_pointer))
    }

    fn get_nested_remote(&mut self, pointer: &[RemoteUID]) -> Option<(&mut JsonValue, Vec<LocalUID>)> {
        let mut value = Some(self);
        let mut local_pointer = vec![];

        for uid in pointer {
            value = match (value.unwrap(), uid) {
                (&mut JsonValue::Object(ref mut map_value), &RemoteUID::Object(ref key, ref replica)) => {
                    let element = try_opt!(map_value.get_mut_element(key, replica));
                    local_pointer.push(LocalUID::Object(key.clone()));
                    Some(&mut element.1)
                }
                (&mut JsonValue::Array(ref mut list_value), &RemoteUID::Array(ref uid)) => {
                    let index = try_opt!(list_value.0.get_idx(uid));
                    let element = try_opt!(list_value.0.lookup_mut(uid));
                    local_pointer.push(LocalUID::Array(index));
                    Some(&mut element.1)
                }
                _ => return None
            }
        }

        Some((value.unwrap(), local_pointer))
    }

    fn as_map(&mut self) -> Result<&mut MapValue<String, JsonValue>, Error> {
        match *self {
            JsonValue::Object(ref mut map_value) => Ok(map_value),
            _ => Err(Error::WrongJsonType)
        }
    }

    fn as_list(&mut self) -> Result<&mut ListValue<JsonValue>, Error> {
        match *self {
            JsonValue::Array(ref mut list_value) => Ok(list_value),
            _ => Err(Error::WrongJsonType)
        }
    }

    fn as_text(&mut self) -> Result<&mut TextValue, Error> {
        match *self {
            JsonValue::String(ref mut text_value) => Ok(text_value),
            _ => Err(Error::WrongJsonType)
        }
    }
}

impl CrdtValue for JsonValue {
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;
    type LocalValue = SJValue;

    fn local_value(&self) -> Self::LocalValue {
        match *self {
            JsonValue::Object(ref map_value) => {
                let mut map = serde_json::Map::with_capacity(map_value.len());
                for (k, v) in map_value.iter() {
                    let _ = map.insert(k.clone(), v[0].1.local_value());
                }
                SJValue::Object(map)
            }
            JsonValue::Array(ref list_value) =>
                SJValue::Array(list_value.iter().map(|v| v.1.local_value()).collect()),
            JsonValue::String(ref text_value) =>
                SJValue::String(text_value.local_value()),
            JsonValue::Number(float) => {
                let number = serde_json::Number::from_f64(float).unwrap();
                SJValue::Number(number)
            }
            JsonValue::Bool(bool_value) =>
                SJValue::Bool(bool_value),
            JsonValue::Null =>
                SJValue::Null,
        }
    }

    fn add_site(&mut self, op: &RemoteOp, site: u32) {
        self.nested_add_site(op, site)
    }

    fn add_site_to_all(&mut self, site: u32) {
        self.nested_add_site_to_all(site)
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        self.nested_validate_site(site)
    }

    fn merge(&mut self, other: JsonValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.nested_merge(other, self_tombstones, other_tombstones).unwrap()
    }
}

impl NestedCrdtValue for JsonValue {
    fn nested_add_site(&mut self, op: &RemoteOp, site: u32) {
        let (value, _) = some!(self.get_nested_remote(&op.pointer));
        match (value, &op.op) {
            (&mut JsonValue::Object(ref mut m), &RemoteOpInner::Object(ref op)) => m.nested_add_site(op, site),
            (&mut JsonValue::Array(ref mut l), &RemoteOpInner::Array(ref op)) => l.nested_add_site(op, site),
            (&mut JsonValue::String(ref mut t), &RemoteOpInner::String(ref op)) => t.add_site(op, site),
            _ => return,
        }
    }

    fn nested_add_site_to_all(&mut self, site: u32) {
        match *self {
            JsonValue::Object(ref mut m) => m.nested_add_site_to_all(site),
            JsonValue::Array(ref mut l) => l.nested_add_site_to_all(site),
            JsonValue::String(ref mut t) => t.add_site_to_all(site),
            _ => return,
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        match *self {
            JsonValue::Object(ref m) => m.nested_validate_site(site),
            JsonValue::Array(ref l) => l.nested_validate_site(site),
            JsonValue::String(ref t) => t.validate_site(site),
            _ => Ok(())
        }
    }

    fn nested_merge(&mut self, other: JsonValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) -> Result<(), Error> {
        match other {
            JsonValue::Object(other_map) =>
                self.as_map()?.nested_merge(other_map, self_tombstones, other_tombstones),
            JsonValue::Array(other_list) =>
                self.as_list()?.nested_merge(other_list, self_tombstones, other_tombstones),
            JsonValue::String(other_text) => {
                self.as_text()?.merge(other_text, self_tombstones, other_tombstones);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn deleted_replicas(&self) -> Vec<Replica> {
        match self.op {
            RemoteOpInner::Object(ref op) => op.deleted_replicas(),
            RemoteOpInner::Array(ref op) => op.deleted_replicas(),
            RemoteOpInner::String(ref op) => op.deleted_replicas(),
        }
    }

    fn add_site(&mut self, site: u32) {
        self.nested_add_site(site)
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        self.nested_validate_site(site)
    }
}

impl NestedCrdtRemoteOp for RemoteOp {
    fn nested_add_site(&mut self, site: u32) {
        // update sites in the pointer
        for uid in self.pointer.iter_mut() {
            match *uid {
                RemoteUID::Object(_, ref mut replica) => {
                    if replica.site == 0 { replica.site = site; }
                }
                RemoteUID::Array(ref mut uid) => {
                    if uid.site == 0 { uid.site = site; }
                }
            }
        }

        // update sites in the op
        match self.op {
            RemoteOpInner::Object(ref mut op) => op.nested_add_site(site),
            RemoteOpInner::Array(ref mut op) => op.nested_add_site(site),
            RemoteOpInner::String(ref mut op) => op.add_site(site),
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        match self.op {
            RemoteOpInner::Object(ref op) => op.nested_validate_site(site),
            RemoteOpInner::Array(ref op) => op.nested_validate_site(site),
            RemoteOpInner::String(ref op) => op.validate_site(site),
        }
    }
}


impl IntoJson for JsonValue {
    #[inline]
    fn into_json(self, _: &Replica) -> Result<JsonValue, Error> {
        Ok(self)
    }
}

impl IntoJson for SJValue {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        match self {
            SJValue::Object(map) => {
                let mut map_value = MapValue::new();
                for (key, value) in map.into_iter() {
                    let _ = map_value.insert(key, value.into_json(replica)?, replica);
                }
                Ok(JsonValue::Object(map_value))
            }
            SJValue::Array(vec) =>
                vec.into_json(replica),
            SJValue::String(string) =>
                string.into_json(replica),
            SJValue::Number(number) =>
                number.as_f64().ok_or(Error::InvalidJson)?.into_json(replica),
            SJValue::Bool(bool_value) =>
                Ok(JsonValue::Bool(bool_value)),
            SJValue::Null =>
                Ok(JsonValue::Null),
        }
    }
}

impl<S: Into<String> + Hash + Eq, T: IntoJson> IntoJson for HashMap<S, T> {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        let mut map_value = MapValue::new();
        for (key, value) in self.into_iter() {
            let _ = map_value.insert(key.into(), value.into_json(replica)?, replica);
        }
        Ok(JsonValue::Object(map_value))
    }
}

impl<T: IntoJson> IntoJson for Vec<T> {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        let mut list_value = ListValue::new();
        for (idx, elt) in self.into_iter().enumerate() {
            let _ = list_value.insert(idx, elt.into_json(replica)?, replica);
        }
        Ok(JsonValue::Array(list_value))
    }
}

impl<'a> IntoJson for &'a str {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        let mut text_value = TextValue::new();
        if !self.is_empty() {
            text_value.replace(0, 0, self, replica)?;
        }
        text_value.1 = None;
        Ok(JsonValue::String(text_value))
    }
}

impl IntoJson for f64 {
    fn into_json(self, _: &Replica) -> Result<JsonValue, Error> {
        match f64::is_finite(self) {
            true => Ok(JsonValue::Number(self)),
            false => Err(Error::InvalidJson),
        }
    }
}

impl IntoJson for i64 {
    fn into_json(self, _: &Replica) -> Result<JsonValue, Error> {
        Ok(JsonValue::Number(self as f64))
    }
}

impl IntoJson for bool {
    fn into_json(self, _: &Replica) -> Result<JsonValue, Error> {
        Ok(JsonValue::Bool(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp_serde;

    #[test]
    fn test_from_str() {
        let crdt = Json::from_str(r#"{"foo":123, "bar":true, "baz": [1.0,2.0,3.0]}"#).unwrap();
        assert_matches!(crdt.value, JsonValue::Object(_));
        assert!(crdt.replica.site == 1);
        assert!(crdt.replica.counter == 1);
        assert!(crdt.awaiting_site.is_empty());
    }

    #[test]
    fn test_from_str_invalid() {
        let invalid_json_str = r#"{"foo":123, "bar":true, "baz": [1.0,2.0,3.0]"#;
        assert!(Json::from_str(invalid_json_str).unwrap_err() == Error::InvalidJson);
    }

    #[test]
    fn test_object_insert() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let remote_op1 = crdt.insert_str("/foo", r#"{"bar": 3.5}"#).unwrap();
        let remote_op2 = crdt.insert("/foo/baz", true).unwrap();

        assert!(crdt.replica.counter == 3);
        assert!(*nested_value(&mut crdt, "/foo/bar").unwrap() == JsonValue::Number(3.5));
        assert!(*nested_value(&mut crdt, "/foo/baz").unwrap() == JsonValue::Bool(true));

        assert!(remote_op1.pointer.is_empty());
        assert_matches!(remote_op1.op, RemoteOpInner::Object(map::RemoteOp::Insert{key: _, element: _, removed: _}));

        assert!(remote_op2.pointer.len() == 1);
        assert!(remote_op2.pointer[0] == RemoteUID::Object("foo".to_owned(), Replica::new(1,1)));
        assert_matches!(remote_op2.op, RemoteOpInner::Object(map::RemoteOp::Insert{key: _, element: _, removed: _}));
    }

    #[test]
    fn test_object_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let result = crdt.insert_str("/foo/bar", r#"{"bar": 3.5}"#);
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_insert_replaces_value() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let _ = crdt.insert("/foo", 19.7).unwrap();
        let remote_op = crdt.insert("/foo", 4.6).unwrap();

        assert!(crdt.replica.counter == 3);
        assert!(*nested_value(&mut crdt, "/foo").unwrap() == JsonValue::Number(4.6));

        assert!(remote_op.pointer.is_empty());
        let (key, element, removed) = map_insert_op_fields(remote_op);
        assert!(key == "foo");
        assert!(element.0 == Replica::new(1,2));
        assert!(element.1 == JsonValue::Number(4.6));
        assert!(removed[0] == Replica::new(1,1));
    }

    #[test]
    fn test_object_insert_same_value() {
        let mut crdt = Json::from_str("{}").unwrap();
        assert!(crdt.insert("/foo", 19.7).is_ok());
        assert!(crdt.insert("/foo", 19.7).unwrap_err() == Error::AlreadyExists);
    }

    #[test]
    fn test_object_insert_awaiting_site() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let result = crdt2.insert("/foo", 19.7);

        assert!(result.unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(*nested_value(&mut crdt2, "/foo").unwrap() == JsonValue::Number(19.7));
    }

    #[test]
    fn test_object_remove() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let remote_op = crdt.remove("/abc/2/def").unwrap();

        assert!(nested_value(&mut crdt, "abc/2/def").is_none());
        assert!(remote_op.pointer.len() == 2);
        assert!(remote_op.pointer[0] == RemoteUID::Object("abc".to_owned(), Replica::new(1,0)));
        assert_matches!(remote_op.pointer[1], RemoteUID::Array(_));

        let (key, removed) = map_remove_op_fields(remote_op);
        assert!(key == "def");
        assert!(removed.len() == 1);
        assert!(removed[0] == Replica::new(1,0));
    }

    #[test]
    fn test_object_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let result = crdt.remove("/uhoh/11/def");
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_remove_does_not_exist() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let result = crdt.remove("/abc/2/zebra!");
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert!(crdt2.remove("/abc/2/def").unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(nested_value(&mut crdt2, "/abc/2/def").is_none());
    }

    #[test]
    fn test_array_insert() {
        let mut crdt = Json::from_str(r#"{"things":[1,[],2,3]}"#).unwrap();
        let remote_op = crdt.insert("/things/1/0", true).unwrap();
        let element = list_insert_op_element(remote_op);
        assert!(*nested_value(&mut crdt, "/things/1/0").unwrap() == JsonValue::Bool(true));
        assert!(crdt.replica.counter == 2);
        assert!(element.1 == JsonValue::Bool(true));
    }

    #[test]
    fn test_array_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        assert!(crdt.insert("/others/1", true).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_array_insert_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        assert!(crdt.insert("/things/4", true).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_array_insert_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert!(crdt2.insert("/things/1", true).unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == JsonValue::Bool(true));
    }

    #[test]
    fn test_array_remove() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let remote_op = crdt.remove("/things/1/2").unwrap();
        let uid = list_remove_op_uid(remote_op);
        assert!(nested_value(&mut crdt, "/things/1/2").is_none());
        assert!(crdt.replica.counter == 2);
        assert!(uid.site == 1 && uid.counter == 0);
    }

    #[test]
    fn test_array_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        assert!(crdt.remove("/things/5/2").unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_array_remove_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        assert!(crdt.remove("/things/1/3").unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_array_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert!(crdt2.remove("/things/1").unwrap_err() == Error::AwaitingSite);

        let remote_op = crdt2.awaiting_site.pop().unwrap();
        let uid = list_remove_op_uid(remote_op);
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == JsonValue::Number(2.0));
        assert!(uid.site == 1 && uid.counter == 0);
    }

    #[test]
    fn test_replace_text() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let remote_op = crdt.replace_text("/1", 1, 2, "åⱡ").unwrap();
        let remote_op = text_remote_op(remote_op);
        assert!(local_json(crdt.value()) == r#"[5.0,"håⱡlo"]"#);
        assert!(remote_op.removes.len() == 1);
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "åⱡ");
        assert!(remote_op.inserts[2].text == "lo");
    }

    #[test]
    fn test_replace_text_invalid_pointer() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.replace_text("/0", 1, 2, "åⱡ").unwrap_err() == Error::WrongJsonType);
    }

    #[test]
    fn test_replace_text_out_of_bounds() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.replace_text("/1", 1, 6, "åⱡ").unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_replace_text_awaiting_site() {
        let remote_crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let mut crdt = Json::from_state(remote_crdt.clone_state(), None).unwrap();
        assert!(crdt.replace_text("/1", 1, 2, "åⱡ").unwrap_err() == Error::AwaitingSite);
        assert!(local_json(crdt.value()) == r#"[5.0,"håⱡlo"]"#);

        let remote_op = text_remote_op(crdt.awaiting_site.pop().unwrap());
        assert!(remote_op.removes.len() == 1);
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "åⱡ");
        assert!(remote_op.inserts[2].text == "lo");
    }

    #[test]
    fn test_execute_remote_object() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let remote_op = crdt1.insert("/baz", 54.0).unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::Insert{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUID::Object("baz".to_owned())]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_remote_array() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let remote_op = crdt1.insert("/foo/0", 54.0).unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();


        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::Insert{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUID::Object("foo".to_owned()),LocalUID::Array(0)]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_remote_string() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let remote_op = crdt1.replace_text("/foo/2", 1, 2, "ab").unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::ReplaceText{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUID::Object("foo".to_owned()), LocalUID::Array(2)]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_remote_missing_pointer() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let remote_op = crdt1.remove("/bar").unwrap();
        let _         = crdt2.remove("/bar").unwrap();
        assert!(crdt2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_merge() {
        let mut crdt1 = Json::from_str(r#"{"x":[{"a": 1},{"b": 2},{"c":true},{"d":false}]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let _ = crdt1.insert("/x/0/e", 222.0).unwrap();
        let _ = crdt1.insert("/x/3/e", 333.0).unwrap();
        let _ = crdt1.remove("/x/2").unwrap();
        let _ = crdt2.insert("/x/1/e", 444.0).unwrap();
        let _ = crdt2.remove("/x/3").unwrap();

        let crdt1_state = crdt1.clone_state();
        crdt1.merge(crdt2.clone_state());
        crdt2.merge(crdt1_state);

        assert!(crdt1.value == crdt2.value);
        assert!(crdt1.tombstones == crdt2.tombstones);
        assert!(crdt1.local_value() == json!({"x":[{"a": 1.0, "e": 222.0}, {"b": 2.0, "e": 444.0}]}));
    }

    #[test]
    fn test_add_site() {
        let crdt1 = Json::from_str(r#"{"foo":[1,2,3],"bar":"hello"}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let _ = crdt2.insert("/baz", json!({"abc":[true, false, 84.0]}));
        let _ = crdt2.insert("/baz/abc/1", 61.0);

        println!("\nAAA\n{:#?}", crdt2);

        let _ = crdt2.replace_text("/bar", 5, 0, " everyone!");

        println!("\nBBB\n{:#?}", crdt2);

        let _ = crdt2.replace_text("/bar", 0, 1, "");

        println!("\nCCC\n{:#?}", crdt2);

        let _ = crdt2.remove("/baz/abc/2");
        let _ = crdt2.remove("/foo");

        let mut remote_ops = crdt2.add_site(11).unwrap().into_iter();

        assert!(crdt2.local_value() == json!({"bar":"ello everyone!", "baz":{"abc":[true, 61.0, 84.0]}}));
        assert!(crdt2.site() == 11);

        // check that the CRDT's elements have the correct sites

        {
            let map = as_map(&crdt2.value);
            assert!(map.0.get("foo").is_none());
            assert!(map.0.get("bar").unwrap()[0].0.site == 1);
            assert!(map.0.get("baz").unwrap()[0].0.site == 11);
        }
        {
            let text = as_text(nested_value(&mut crdt2, "/bar").unwrap());

            println!("{:#?}", text);

            let mut text_elements = text.0.iter();
            assert!(text_elements.next().unwrap().uid.site == 11);
            assert!(text_elements.next().unwrap().uid.site == 11);
        }
        {
            let list = as_list(nested_value(&mut crdt2, "/baz/abc").unwrap());
            assert!((list.0.get_elt(0).unwrap().0).0.site == 11);
            assert!((list.0.get_elt(1).unwrap().0).0.site == 11);
            assert!((list.0.get_elt(2).unwrap().0).0.site == 11);
        }

        // check that the remote ops' elements have the correct sites
        let (_, element, replicas) = map_insert_op_fields(remote_ops.next().unwrap());
        assert!(element.0.site == 11);
        assert!(element.1.validate_site(11).is_ok());
        assert!(replicas.is_empty());

        let element = list_insert_op_element(remote_ops.next().unwrap());
        assert!(element.0.site == 11);
        assert!(element.1.validate_site(11).is_ok());

        let element = text_remote_op(remote_ops.next().unwrap());
        assert!(element.removes.is_empty());
        assert!(element.inserts[0].uid.site == 11);

        let element = text_remote_op(remote_ops.next().unwrap());
        assert!(element.removes[0].site == 1);
        assert!(element.inserts[0].uid.site == 11);

        let uid = list_remove_op_uid(remote_ops.next().unwrap());
        assert!(uid.site == 11);

        let (_, replicas) = map_remove_op_fields(remote_ops.next().unwrap());
        assert!(replicas[0].site == 1);
    }

    #[test]
    fn test_add_site_nested() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let _ = crdt2.insert("/foo", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        }));

        let mut remote_ops = crdt2.add_site(22).unwrap().into_iter();
        assert!(crdt2.site() == 22);

        let object = nested_value(&mut crdt2, "/foo").unwrap();
        assert!(object.validate_site(22).is_ok());

        let (_, element, replicas) = map_insert_op_fields(remote_ops.next().unwrap());
        assert!(element.0.site == 22);
        assert!(element.1.validate_site(22).is_ok());
        assert!(replicas.is_empty());
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut crdt = Json::from_str("{}").unwrap();
        let _ = crdt.insert("/foo", vec![1.0]).unwrap();
        let _ = crdt.insert("/foo/0", "hello").unwrap();
        let _ = crdt.replace_text("/foo/0", 5, 0, " everybody!").unwrap();
        assert!(crdt.add_site(33).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let remote_op = crdt1.remove("/bar").unwrap();
        assert!(crdt2.execute_remote(&remote_op).is_some());
        assert!(crdt2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_serialize() {
        let crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();

        let s_json = serde_json::to_string(&crdt1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&crdt1).unwrap();
        let crdt2: Json = serde_json::from_str(&s_json).unwrap();
        let crdt3: Json = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(crdt1 == crdt2);
        assert!(crdt1 == crdt3);
    }

    #[test]
    fn test_serialize_value() {
        let crdt = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();

        let s_json = serde_json::to_string(crdt.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(crdt.value()).unwrap();
        let value2: JsonValue = serde_json::from_str(&s_json).unwrap();
        let value3: JsonValue = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*crdt.value() == value2);
        assert!(*crdt.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut crdt = Json::from_str(r#"{"foo":{}}"#).unwrap();
        let remote_op1 = crdt.insert("/foo/bar", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        })).unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let mut crdt1 = Json::from_str(r#"{"foo":{}}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let remote_op = crdt1.insert("/foo/bar", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        })).unwrap();
        let local_op1 = crdt2.execute_remote(&remote_op).unwrap();

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert_eq!(s_json, r#"{"op":"insert","pointer":["foo","bar"],"value":{"a":[[1.0],["hello everyone!"],{"x":3.0}],"b":{"cat":true,"dog":false}}}"#);
        assert_eq!(local_op1, local_op2);
        assert_eq!(local_op1, local_op3);
    }

    fn nested_value<'a>(crdt: &'a mut Json, pointer: &str) -> Option<&'a JsonValue> {
        let pointer = try_opt!(JsonValue::split_pointer(pointer).ok());
        let (value, _) = try_opt!(crdt.value.get_nested_local(&pointer).ok());
        Some(value)
    }

    fn local_json(json_value: &JsonValue) -> String {
        serde_json::to_string(&json_value.local_value()).unwrap()
    }

    fn map_insert_op_fields(remote_op: RemoteOp) -> (String, map::Element<JsonValue>, Vec<Replica>) {
        match remote_op.op {
            RemoteOpInner::Object(map::RemoteOp::Insert{key: k, element: e, removed: r}) => (k, e, r),
            _ => panic!(),
        }
    }

    fn map_remove_op_fields(remote_op: RemoteOp) -> (String, Vec<Replica>) {
        match remote_op.op {
            RemoteOpInner::Object(map::RemoteOp::Remove{key: k, removed: r}) => (k, r),
            _ => panic!(),
        }
    }

    fn list_insert_op_element(remote_op: RemoteOp) -> list::Element<JsonValue> {
        match remote_op.op {
            RemoteOpInner::Array(list::RemoteOp::Insert(element)) => element,
            _ => panic!(),
        }
    }

    fn list_remove_op_uid(remote_op: RemoteOp) -> sequence::uid::UID {
        match remote_op.op {
            RemoteOpInner::Array(list::RemoteOp::Remove(uid)) => uid,
            _ => panic!(),
        }
    }

    fn text_remote_op(remote_op: RemoteOp) -> text::RemoteOp {
        match remote_op.op {
            RemoteOpInner::String(op) => op,
            _ => panic!(),
        }
    }

    fn as_map(json_value: &JsonValue) -> &MapValue<String, JsonValue> {
        match *json_value {
            JsonValue::Object(ref map_value) => map_value,
            _ => panic!(),
        }
    }

    fn as_list(json_value: &JsonValue) -> &ListValue<JsonValue> {
        match *json_value {
            JsonValue::Array(ref list_value) => list_value,
            _ => panic!(),
        }
    }

    fn as_text(json_value: &JsonValue) -> &TextValue {
        match *json_value {
            JsonValue::String(ref text_value) => text_value,
            _ => panic!(),
        }
    }
}
