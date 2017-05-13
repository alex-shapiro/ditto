//! A `Json` CRDT stores any value that can be represented
//! as JSON - objects, arrays, text, numbers, bools, and null.

use Error;
use Replica;
use list::{self, ListValue};
use map::{self, MapValue};
use text::{self, TextValue};
use sequence;
use traits::*;

use serde_json::{self, Value as SJValue};
use std::collections::HashMap;
use std::hash::Hash;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Json {
    value: JsonValue,
    replica: Replica,
    awaiting_site: Vec<RemoteOp>,
}

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
pub struct LocalOp {
    pointer: Vec<LocalUID>,
    op: LocalOpInner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOpInner {
    Object(map::LocalOp<String, JsonValue>),
    Array(list::LocalOp<JsonValue>),
    String(text::LocalOp),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalUID {
    Object(String),
    Array(usize),
}

pub trait IntoJson {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error>;
}

impl Json {

    /// Constructs and returns a new `Json` CRDT from a JSON string.
    /// The crdt has site 1 and counter 0.
    pub fn from_str(json_str: &str) -> Result<Self, Error> {
        let mut replica = Replica::new(1, 0);
        let local_json: SJValue = serde_json::from_str(json_str)?;
        let value = local_json.into_json(&replica)?;
        replica.counter += 1;
        Ok(Json{value, replica, awaiting_site: vec![]})
    }

    /// Inserts a key-value pair into an object in the Json CRDT and
    /// returns an op that can be sent to remote sites for replication.
    /// If the CRDT does not have a site allocated, it caches the op
    /// and returns an `AwaitingSite` error.
    pub fn object_insert<T: IntoJson>(&mut self, pointer: &str, key: String, value: T) -> Result<RemoteOp, Error> {
        let value = value.into_json(&self.replica)?;
        let remote_op = self.value.object_insert(pointer, key, value, &self.replica)?;
        self.after_op(remote_op)
    }

    /// Inserts a key-value pair into an object in the Json CRDT, where
    /// the value being inserted is encoded as a JSON `&str`.
    pub fn object_insert_json(&mut self, pointer: &str, key: String, value: &str) -> Result<RemoteOp, Error> {
        let json: SJValue = serde_json::from_str(&value)?;
        self.object_insert(pointer, key, json)
    }

    /// Deletes a key-value pair from an object in the Json CRDT.
    pub fn object_remove(&mut self, pointer: &str, key: &str) -> Result<RemoteOp, Error> {
        let remote_op = self.value.object_remove(pointer, key)?;
        self.after_op(remote_op)
    }

    /// Inserts an element into an array in the Json CRDT.
    pub fn array_insert<T: IntoJson>(&mut self, pointer: &str, index: usize, value: T) -> Result<RemoteOp, Error> {
        let value = value.into_json(&self.replica)?;
        let remote_op = self.value.array_insert(pointer, index, value, &self.replica)?;
        self.after_op(remote_op)
    }

    /// Inserts an element into an array in the Json CRDT, where the
    /// value being inserted is encoded as a JSON `&str`.
    pub fn array_insert_json(&mut self, pointer: &str, index: usize, value: &str) -> Result<RemoteOp, Error> {
        let json: SJValue = serde_json::from_str(&value)?;
        self.array_insert(pointer, index, json)
    }

    /// Removes an element from an array in the Json CRDT.
    pub fn array_remove(&mut self, pointer: &str, index: usize) -> Result<RemoteOp, Error> {
        let remote_op = self.value.array_remove(pointer, index)?;
        self.after_op(remote_op)
    }

    /// Inserts new text into a text node in the Json CRDT.
    pub fn string_insert(&mut self, pointer: &str, index: usize, text: String) -> Result<RemoteOp, Error> {
        let remote_op = self.value.string_insert(pointer, index, text, &self.replica)?;
        self.after_op(remote_op)
    }

    /// Removes a text range from a text node in the Json CRDT.
    pub fn string_remove(&mut self, pointer: &str, index: usize, len: usize) -> Result<RemoteOp, Error> {
        let remote_op = self.value.string_remove(pointer, index, len, &self.replica)?;
        self.after_op(remote_op)
    }

    /// Replaces a text range in a text node in the Json CRDT.
    pub fn string_replace(&mut self, pointer: &str, index: usize, len: usize, text: String) -> Result<RemoteOp, Error> {
        let remote_op = self.value.string_replace(pointer, index, len, text, &self.replica)?;
        self.after_op(remote_op)
    }

    fn after_op(&mut self, op: RemoteOp) -> Result<RemoteOp, Error> {
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl Crdt for Json {
    crdt_impl!(Json, JsonValue);
}

impl JsonValue {
    pub fn object_insert<T: IntoJson>(&mut self, pointer: &str, key: String, value: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let map_value = json_value.as_map()?;
        let remote_op = map_value.insert(key, value.into_json(&replica)?, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Object(remote_op)})
    }

    pub fn object_remove(&mut self, pointer: &str, key: &str) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let map_value = json_value.as_map()?;
        let remote_op = map_value.remove(key)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Object(remote_op)})
    }

    pub fn array_insert<T: IntoJson>(&mut self, pointer: &str, index: usize, value: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let list_value = json_value.as_list()?;
        let remote_op = list_value.insert(index, value.into_json(&replica)?, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Array(remote_op)})
    }

    pub fn array_remove(&mut self, pointer: &str, index: usize) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let list_value = json_value.as_list()?;
        let remote_op = list_value.remove(index)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Array(remote_op)})
    }

    pub fn string_insert(&mut self, pointer: &str, index: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let text_value = json_value.as_text()?;
        let remote_op = text_value.insert(index, text, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::String(remote_op)})
    }

    pub fn string_remove(&mut self, pointer: &str, index: usize, len: usize, replica: &Replica) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let text_value = json_value.as_text()?;
        let remote_op = text_value.remove(index, len, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::String(remote_op)})
    }

    pub fn string_replace(&mut self, pointer: &str, index: usize, len: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        let (json_value, remote_pointer) = self.get_nested_local(pointer)?;
        let text_value = json_value.as_text()?;
        let remote_op = text_value.replace(index, len, text, &replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::String(remote_op)})
    }

    pub fn execute_remote(&mut self, remote_op: &RemoteOp) -> Option<LocalOp> {
        let (json_value, local_pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
        match (json_value, &remote_op.op) {
            (&mut JsonValue::Object(ref mut map), &RemoteOpInner::Object(ref op)) => {
                let local_op = try_opt!(map.execute_remote(op));
                Some(LocalOp{pointer: local_pointer, op: LocalOpInner::Object(local_op)})
            }
            (&mut JsonValue::Array(ref mut list), &RemoteOpInner::Array(ref op)) => {
                let local_op = try_opt!(list.execute_remote(op));
                Some(LocalOp{pointer: local_pointer, op: LocalOpInner::Array(local_op)})
            }
            (&mut JsonValue::String(ref mut text), &RemoteOpInner::String(ref op)) => {
                let local_op = try_opt!(text.execute_remote(op));
                Some(LocalOp{pointer: local_pointer, op: LocalOpInner::String(local_op)})
            }
            _ => None,
        }
    }

    fn get_nested_local(&mut self, pointer: &str) -> Result<(&mut JsonValue, Vec<RemoteUID>), Error> {
        if !(pointer.is_empty() || pointer.starts_with("/")) {
            return Err(Error::DoesNotExist)
        }

        let mut value = Some(self);
        let mut remote_pointer = vec![];

        for key in pointer.split("/").skip(1) {
            match value.unwrap() {
                &mut JsonValue::Object(ref mut map_value) => {
                    let mut element = map_value.get_mut(key).ok_or(Error::DoesNotExist)?;
                    let uid = RemoteUID::Object(key.to_owned(), element.0.clone());
                    remote_pointer.push(uid);
                    value = Some(&mut element.1)
                }
                &mut JsonValue::Array(ref mut list_value) => {
                    let index = usize::from_str(key)?;
                    let mut element = list_value.get_mut(index).ok_or(Error::DoesNotExist)?;
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
                    let mut element = try_opt!(map_value.get_mut_element(key, replica));
                    local_pointer.push(LocalUID::Object(key.clone()));
                    Some(&mut element.1)
                }
                (&mut JsonValue::Array(ref mut list_value), &RemoteUID::Array(ref uid)) => {
                    let index = try_opt!(list_value.find_index(uid).ok());
                    let mut element = try_opt!(list_value.get_mut(index));
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
        let (value, _) = some!(self.get_nested_remote(&op.pointer));
        match (value, &op.op) {
            (&mut JsonValue::Object(ref mut m), &RemoteOpInner::Object(ref op)) => add_site_map(m, op, site),
            (&mut JsonValue::Array(ref mut l), &RemoteOpInner::Array(ref op)) => add_site_list(l, op, site),
            (&mut JsonValue::String(ref mut t), &RemoteOpInner::String(ref op)) => t.add_site(op, site),
            _ => return,
        }
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn add_site(&mut self, site: u32) {
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
            RemoteOpInner::Object(ref mut op) => add_site_map_op(op, site),
            RemoteOpInner::Array(ref mut op) => add_site_list_op(op, site),
            RemoteOpInner::String(ref mut op) => op.add_site(site),
        }
    }
}

impl AddSiteToAll for JsonValue {
    fn add_site_to_all(&mut self, site: u32) {
        match *self {
            JsonValue::Object(ref mut m) => m.add_site_to_all(site),
            JsonValue::Array(ref mut l) => l.add_site_to_all(site),
            JsonValue::String(ref mut s) => s.add_site_to_all(site),
            _ => return,
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

impl IntoJson for String {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        let mut text_value = TextValue::new();
        let _ = text_value.insert(0, self, replica);
        Ok(JsonValue::String(text_value))
    }
}

impl<'a> IntoJson for &'a str {
    fn into_json(self, replica: &Replica) -> Result<JsonValue, Error> {
        self.to_owned().into_json(replica)
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

impl IntoJson for bool {
    fn into_json(self, _: &Replica) -> Result<JsonValue, Error> {
        Ok(JsonValue::Bool(self))
    }
}

fn add_site_map(map_value: &mut MapValue<String, JsonValue>, op: &map::RemoteOp<String, JsonValue>, site: u32) {
    if let map::RemoteOp::Insert{ref key, ref element, ..} = *op {
        let elements = some!(map_value.inner.get_mut(key));
        let index = some!(elements.binary_search_by(|e| e.0.cmp(&element.0)).ok());
        let ref mut element = elements[index];
        element.0.site = site;
        element.1.add_site_to_all(site);
    }
}

fn add_site_list(list_value: &mut ListValue<JsonValue>, op: &list::RemoteOp<JsonValue>, site: u32) {
    if let list::RemoteOp::Insert(list::Element(ref uid, _)) = *op {
        let index = some!(list_value.find_index(uid).ok());
        let ref mut element = list_value.0[index];
        element.0.site = site;
        element.1.add_site_to_all(site);
    }
}

fn add_site_map_op(op: &mut map::RemoteOp<String, JsonValue>, site: u32) {
    match *op {
        map::RemoteOp::Insert{ref mut element, ref mut removed, ..} => {
            element.0.site = site;
            element.1.add_site_to_all(site);
            for replica in removed {
                if replica.site == 0 { replica.site = site; }
            }
        }
        map::RemoteOp::Remove{ref mut removed, ..} => {
            for replica in removed {
                if replica.site == 0 { replica.site = site; }
            }
        }
    }
}

fn add_site_list_op(op: &mut list::RemoteOp<JsonValue>, site: u32) {
    match *op {
        list::RemoteOp::Insert(ref mut element) => {
            element.0.site = site;
            element.1.add_site_to_all(site);
        }
        list::RemoteOp::Remove(ref mut uid) => {
            if uid.site == 0 { uid.site = site };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let remote_op1 = crdt.object_insert_json("", "foo".to_owned(), r#"{"bar": 3.5}"#).unwrap();
        let remote_op2 = crdt.object_insert("/foo", "baz".to_owned(), true).unwrap();

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
        let result = crdt.object_insert_json("/foo", "bar".to_owned(), r#"{"bar": 3.5}"#);
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_insert_replaces_value() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let _ = crdt.object_insert("", "foo".to_owned(), 19.7).unwrap();
        let remote_op = crdt.object_insert("", "foo".to_owned(), 4.6).unwrap();

        assert!(crdt.replica.counter == 3);
        assert!(*nested_value(&mut crdt, "/foo").unwrap() == JsonValue::Number(4.6));

        assert!(remote_op.pointer.is_empty());
        let (key, element, removed) = map_insert_op_fields(remote_op);
        assert!(key == "foo");
        assert!(element.0 == Replica::new(1,2));
        assert!(element.1 == JsonValue::Number(4.6));
        assert!(removed[0].0 == Replica::new(1,1));
    }

    #[test]
    fn test_object_insert_same_value() {
        let mut crdt = Json::from_str("{}").unwrap();
        assert!(crdt.object_insert("", "foo".to_owned(), 19.7).is_ok());
        assert!(crdt.object_insert("", "foo".to_owned(), 19.7).unwrap_err() == Error::AlreadyExists);
    }

    #[test]
    fn test_object_insert_awaiting_site() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        let result = crdt2.object_insert("", "foo".to_owned(), 19.7);

        assert!(result.unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(*nested_value(&mut crdt2, "/foo").unwrap() == JsonValue::Number(19.7));
    }

    #[test]
    fn test_object_remove() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let remote_op = crdt.object_remove("/abc/2", "def").unwrap();

        assert!(nested_value(&mut crdt, "abc/2/def").is_none());
        assert!(remote_op.pointer.len() == 2);
        assert!(remote_op.pointer[0] == RemoteUID::Object("abc".to_owned(), Replica::new(1,0)));
        assert_matches!(remote_op.pointer[1], RemoteUID::Array(_));

        let (key, removed) = map_remove_op_fields(remote_op);
        assert!(key == "def");
        assert!(removed.len() == 1);
        assert!(removed[0].0 == Replica::new(1,0));
        assert!(removed[0].1 == JsonValue::Bool(false));
    }

    #[test]
    fn test_object_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let result = crdt.object_remove("/uhoh/11", "def");
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_remove_does_not_exist() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let result = crdt.object_remove("/abc/2", "zebra!");
        assert!(result.unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_object_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        assert!(crdt2.object_remove("/abc/2", "def").unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(nested_value(&mut crdt2, "/abc/2/def").is_none());
    }

    #[test]
    fn test_array_insert() {
        let mut crdt = Json::from_str(r#"{"things":[1,[],2,3]}"#).unwrap();
        let remote_op = crdt.array_insert("/things/1", 0, true).unwrap();
        let element = list_insert_op_element(remote_op);
        assert!(*nested_value(&mut crdt, "/things/1/0").unwrap() == JsonValue::Bool(true));
        assert!(crdt.replica.counter == 2);
        assert!(element.1 == JsonValue::Bool(true));
    }

    #[test]
    fn test_array_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        assert!(crdt.array_insert("/others", 1, true).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_array_insert_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        assert!(crdt.array_insert("/things", 4, true).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_array_insert_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        assert!(crdt2.array_insert("/things", 1, true).unwrap_err() == Error::AwaitingSite);
        assert!(crdt2.awaiting_site.len() == 1);
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == JsonValue::Bool(true));
    }

    #[test]
    fn test_array_remove() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let remote_op = crdt.array_remove("/things/1", 2).unwrap();
        let element = list_remove_op_element(remote_op);
        assert!(nested_value(&mut crdt, "/things/1/2").is_none());
        assert!(crdt.replica.counter == 2);
        assert!(local_json(&element.1) == r#""hi""#);
    }

    #[test]
    fn test_array_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        assert!(crdt.array_remove("/things/5", 2).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_array_remove_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        assert!(crdt.array_remove("/things/1", 3).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_array_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        assert!(crdt2.array_remove("/things", 1).unwrap_err() == Error::AwaitingSite);

        let remote_op = crdt2.awaiting_site.pop().unwrap();
        let element = list_remove_op_element(remote_op);
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == JsonValue::Number(2.0));
        assert!(local_json(&element.1) == r#"[true,false,"hi"]"#);
    }

    #[test]
    fn test_string_insert() {
        let mut crdt = Json::from_str(r#"["hello",5.0]"#).unwrap();
        let remote_op = crdt.string_insert("/0", 0, "😗🇺🇸".to_owned()).unwrap();
        let remote_op = text_remote_op(remote_op);
        assert!(local_json(crdt.value()) == r#"["😗🇺🇸hello",5.0]"#);
        assert!(remote_op.removes.is_empty());
        assert!(remote_op.inserts[0].text == "😗🇺🇸");
        assert!(crdt.replica.counter == 2);
    }

    #[test]
    fn test_string_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"["hello",5.0]"#).unwrap();
        assert!(crdt.string_insert("/1", 0, "😗🇺🇸".to_owned()).unwrap_err() == Error::WrongJsonType);
    }

    #[test]
    fn test_string_insert_out_of_bounds() {
        let mut crdt = Json::from_str(r#"["hello",5.0]"#).unwrap();
        assert!(crdt.string_insert("/0", 6, "😗🇺🇸".to_owned()).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_string_insert_awaiting_site() {
        let remote_crdt = Json::from_str(r#"["hello",5.0]"#).unwrap();
        let mut crdt = Json::from_value(remote_crdt.clone_value(), 0);
        assert!(crdt.string_insert("/0", 0, "😗🇺🇸".to_owned()).unwrap_err() == Error::AwaitingSite);
        assert!(local_json(crdt.value()) == r#"["😗🇺🇸hello",5.0]"#);

        let remote_op = text_remote_op(crdt.awaiting_site.pop().unwrap());
        assert!(remote_op.removes.is_empty());
        assert!(remote_op.inserts[0].text == "😗🇺🇸");
    }

    #[test]
    fn test_string_remove() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let remote_op = crdt.string_remove("/1", 1, 2).unwrap();
        let remote_op = text_remote_op(remote_op);

        assert!(local_json(crdt.value()) == r#"[5.0,"hlo"]"#);
        assert!(remote_op.removes[0].text == "hello");
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "lo");
    }

    #[test]
    fn test_string_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.string_remove("/0", 1, 2).unwrap_err() == Error::WrongJsonType);
    }

    #[test]
    fn test_string_remove_out_of_bounds() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.string_remove("/1", 1, 6).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_string_remove_awaiting_site() {
        let remote_crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let mut crdt = Json::from_value(remote_crdt.clone_value(), 0);
        assert!(crdt.string_remove("/1", 1, 2).unwrap_err() == Error::AwaitingSite);
        assert!(local_json(crdt.value()) == r#"[5.0,"hlo"]"#);

        let remote_op = text_remote_op(crdt.awaiting_site.pop().unwrap());
        assert!(remote_op.removes[0].text == "hello");
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "lo");
    }

    #[test]
    fn test_string_replace() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let remote_op = crdt.string_replace("/1", 1, 2, "åⱡ".to_owned()).unwrap();
        let remote_op = text_remote_op(remote_op);
        assert!(local_json(crdt.value()) == r#"[5.0,"håⱡlo"]"#);
        assert!(remote_op.removes[0].text == "hello");
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "åⱡ");
        assert!(remote_op.inserts[2].text == "lo");
    }

    #[test]
    fn test_string_replace_invalid_pointer() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.string_replace("/0", 1, 2, "åⱡ".to_owned()).unwrap_err() == Error::WrongJsonType);
    }

    #[test]
    fn test_string_replace_out_of_bounds() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert!(crdt.string_replace("/1", 1, 6, "åⱡ".to_owned()).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_string_replace_awaiting_site() {
        let remote_crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let mut crdt = Json::from_value(remote_crdt.clone_value(), 0);
        assert!(crdt.string_replace("/1", 1, 2, "åⱡ".to_owned()).unwrap_err() == Error::AwaitingSite);
        assert!(local_json(crdt.value()) == r#"[5.0,"håⱡlo"]"#);

        let remote_op = text_remote_op(crdt.awaiting_site.pop().unwrap());
        assert!(remote_op.removes[0].text == "hello");
        assert!(remote_op.inserts[0].text == "h");
        assert!(remote_op.inserts[1].text == "åⱡ");
        assert!(remote_op.inserts[2].text == "lo");
    }

    #[test]
    fn test_execute_remote_object() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        let remote_op = crdt1.object_insert("", "baz".to_owned(), 54.0).unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(local_op.pointer.is_empty());
    }

    #[test]
    fn test_execute_remote_array() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        let remote_op = crdt1.array_insert("/foo", 0, 54.0).unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(local_op.pointer == [LocalUID::Object("foo".to_owned())]);
    }

    #[test]
    fn test_execute_remote_string() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        let remote_op = crdt1.string_replace("/foo/2", 1, 2, "ab".to_owned()).unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(local_op.pointer == [LocalUID::Object("foo".to_owned()), LocalUID::Array(2)]);
    }

    #[test]
    fn test_execute_remote_missing_pointer() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 2);
        let remote_op = crdt1.object_remove("", "bar").unwrap();
        let _         = crdt2.object_remove("", "bar").unwrap();
        assert!(crdt2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_add_site() {
        // let mut crdt = Json::from_str("{}").unwrap();
        // let _ = crdt.object_insert("", "foo", vec![1.0]).unwrap();
        // let _ = crdt.array_insert("", "foo", 0, "hello").unwrap();
        // let _ = crdt.text_insert("/foo/0", 5, " everybody").unwrap();
        // let mut remote_ops = crdt.add_site(11).unwrap().into_iter();

        // let nested_value = nested_value(&mut crdt, "/foo/0")

        // assert!(crdt.replica.site == 11);
        // assert!(crdt.value.get_nested_local())

        // let (_, element, _) = map_insert_op_fields(remote_ops.next().unwrap());
        // assert!(element.0.site == 11);

        // let element = list_insert_op_element(remote_ops.next().unwrap());
        // assert!(element.0.site == 11);

        // let element = text_remote_op(remote_ops.next().unwrap());
        // assert!(element.site == 11);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut crdt = Json::from_str("{}").unwrap();
        let _ = crdt.object_insert("", "foo", vec![1.0]).unwrap();
        let _ = crdt.array_insert("", "foo", 0, "hello").unwrap();
        let _ = crdt.text_insert("/foo/0", 5, " everybody").unwrap();
        assert!(crdt.add_site(11).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_value(crdt1.clone_value(), 0);
        let remote_op = crdt1.object_remove("", "bar").unwrap();
        assert!(crdt2.execute_remote(&remote_op).is_some());
        assert!(crdt2.execute_remote(&remote_op).is_none());
    }

    fn nested_value<'a>(crdt: &'a mut Json, pointer: &str) -> Option<&'a JsonValue> {
        let (value, _) = try_opt!(crdt.value.get_nested_local(pointer).ok());
        Some(value)
    }

    fn local_json(json_value: &JsonValue) -> String {
        let x = serde_json::to_string(&json_value.local_value()).unwrap();
        println!("{:?}", &x);
        x
    }

    fn map_insert_op_fields(remote_op: RemoteOp) -> (String, map::Element<JsonValue>, Vec<map::Element<JsonValue>>) {
        match remote_op.op {
            RemoteOpInner::Object(map::RemoteOp::Insert{key: k, element: e, removed: r}) => (k, e, r),
            _ => panic!(),
        }
    }

    fn map_remove_op_fields(remote_op: RemoteOp) -> (String, Vec<map::Element<JsonValue>>) {
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

    fn list_remove_op_element(remote_op: RemoteOp) -> list::Element<JsonValue> {
        match remote_op.op {
            RemoteOpInner::Array(list::RemoteOp::Remove(element)) => element,
            _ => panic!(),
        }
    }

    fn text_remote_op(remote_op: RemoteOp) -> text::RemoteOp {
        match remote_op.op {
            RemoteOpInner::String(op) => op,
            _ => panic!(),
        }
    }

}
