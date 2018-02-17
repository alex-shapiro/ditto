//! A CRDT that stores a JSON value.

use Error;
use dot::{Dot, Summary, SiteId};
use list2::{self as list, Inner as ListInner};
use map2::{self as map, Inner as MapInner};
use text2::{self as text, Inner as TextInner};
use sequence;
use traits2::*;

use serde_json::{self, Value as SJValue};
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;
use std::str::FromStr;

/// Json is a CRDT that stores a JSON value. It can handle
/// any kind of JSON value (object, array, string, number, bool, and null)
/// and allows arbitrarily-nested values. A nested Json value is indexed by a
/// [JSON pointer](https://tools.ietf.org/html/rfc6901).
///
/// Internally, Json is built on Ditto's [`Map`](../map/Map.t.html),
/// [`List`](../list/List.t.html), and [`Text`](../text/Text.t.html)
/// CRDTs. It can be used as a CmRDT or a CvRDT, providing both
/// op-based and state-based replication. This flexibility comes
/// with tradeoffs:
///
///   * Unlike a pure CmRDT, it requires tombstones, which increase size.
///   * Unlike a pure CvRDT, it requires each site to replicate its ops
///     in their order of generation.
///
/// The root value of a Json CRDT (typically an object or array) cannot
/// be replaced; for example, a Json CRDT whose root is an array will
/// always have its root be an array. This constraint means that any Json
/// CRDT with a numeric, boolean, or null root is immutable.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Json {
    inner:      Inner,
    summary:    Summary,
    site_id:    SiteId,
    cached_ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonState<'a> {
    inner: Cow<'a, Inner>,
    summary: Cow<'a, Summary>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Inner {
    Object(MapInner<String, Inner>),
    Array(ListInner<Inner>),
    String(TextInner),
    Number(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    pointer: Vec<Uid>,
    op: OpInner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OpInner {
    Object(map::Op<String, Inner>),
    Array(list::Op<Inner>),
    String(text::Op),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Uid {
    Object(String, Dot),
    Array(sequence::uid::UID),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all="snake_case")]
pub enum LocalOp {
    Insert{pointer: Vec<LocalUid>, value: SJValue},
    Remove{pointer: Vec<LocalUid>},
    ReplaceText{pointer: Vec<LocalUid>, changes: Vec<text::LocalOp>},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocalUid {
    Object(String),
    Array(usize),
}

pub trait IntoJson {
    fn into_json(self, dot: Dot) -> Result<Inner, Error>;
}

impl Json {

    /// Constructs and returns a new `Json` CRDT with site 1 from any
    /// value that satisfies the [`IntoJson`](IntoJson.t.html) trait.
    pub fn new<T: IntoJson>(local_value: T) -> Result<Self, Error> {
        let site_id = 1;
        let mut summary = Summary::new();
        let dot = summary.get_dot(site_id);
        let inner = local_value.into_json(dot)?;
        Ok(Json{inner, summary, site_id, cached_ops: vec![]})
    }

    /// Constructs and returns a new `Json` CRDT with site 1 from an
    /// unparsed JSON string.
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
    pub fn insert<T: IntoJson>(&mut self, pointer: &str, value: T) -> Result<Op, Error> {
        let dot   = self.summary.get_dot(self.site_id);
        let value = value.into_json(dot)?;
        let op    = self.inner.insert(pointer, value, dot)?;
        self.after_op(op)
    }

    /// Inserts a value into the Json CRDT at the given json pointer.
    /// The enclosing value may be an object or an array and the
    /// value being inserted is stringified JSON.
    ///
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert_str(&mut self, pointer: &str, value: &str) -> Result<Op, Error> {
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
    pub fn remove(&mut self, pointer: &str) -> Result<Op, Error> {
        let op = self.inner.remove(pointer)?;
        self.after_op(op)
    }

    /// Replaces a text range in a text value in the Json CRDT.
    /// If the CRDT does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str) -> Result<Op, Error> {
        let dot = self.summary.get_dot(self.site_id);
        let op = self.inner.replace_text(pointer, index, len, text, dot)?;
        self.after_op(op)
    }

    crdt_impl2! {
        Json,
        JsonState,
        JsonState<'static>,
        JsonState,
        Inner,
        Op,
        Option<LocalOp>,
        SJValue,
    }
}

impl Inner {
    pub fn insert<T: IntoJson>(&mut self, pointer: &str, value: T, dot: Dot) -> Result<Op, Error> {
        let mut pointer = Self::split_pointer(pointer)?;
        let key = pointer.pop().ok_or(Error::DoesNotExist)?;
        let (json_value, remote_pointer) = self.get_nested_local(&pointer)?;
        let value = value.into_json(dot)?;

        match *json_value {
            Inner::Object(ref mut map) => {
                let op = map.insert(key.into(), value, dot);
                let op = OpInner::Object(op);
                Ok(Op{pointer: remote_pointer, op})
            }
            Inner::Array(ref mut list) => {
                let idx = usize::from_str(key)?;
                let op = list.insert(idx, value, dot);
                let op = OpInner::Array(op);
                Ok(Op{pointer: remote_pointer, op})
            }
            _ => Err(Error::DoesNotExist),
        }
    }

    pub fn remove(&mut self, pointer: &str) -> Result<Op, Error> {
        let mut pointer = Self::split_pointer(pointer)?;
        let key = pointer.pop().ok_or(Error::DoesNotExist)?;
        let (json_value, remote_pointer) = self.get_nested_local(&pointer)?;

        match *json_value {
            Inner::Object(ref mut map) => {
                let op = map.remove(key).ok_or(Error::Noop)?;
                Ok(Op{pointer: remote_pointer, op: OpInner::Object(op)})
            }
            Inner::Array(ref mut list) => {
                let idx = usize::from_str(key)?;
                let (_, op) = list.remove(idx);
                Ok(Op{pointer: remote_pointer, op: OpInner::Array(op)})
            }
            _ => Err(Error::DoesNotExist),
        }
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str, dot: Dot) -> Result<Op, Error> {
        let pointer = Self::split_pointer(pointer)?;
        let (inner, remote_pointer) = self.get_nested_local(&pointer)?;
        let text_inner = inner.as_text()?;
        let op = text_inner.replace(index, len, text, dot).ok_or(Error::Noop)?;
        Ok(Op{pointer: remote_pointer, op: OpInner::String(op)})
    }

    pub fn execute_op(&mut self, op: Op) -> Option<LocalOp> {
        let (inner, mut pointer) = self.get_nested_remote(&op.pointer)?;
        match op.op {
            OpInner::Object(op) => {
                match inner.as_map().ok()?.execute_op(op) {
                    map::LocalOp::Insert{key, value} => {
                        pointer.push(LocalUid::Object(key));
                        Some(LocalOp::Insert{pointer, value: value.local_value()})
                    }
                    map::LocalOp::Remove{key} => {
                        pointer.push(LocalUid::Object(key));
                        Some(LocalOp::Remove{pointer})
                    }
                }
            }
            OpInner::Array(op) => {
                match inner.as_list().ok()?.execute_op(op)? {
                    list::LocalOp::Insert{idx, value} => {
                        pointer.push(LocalUid::Array(idx));
                        Some(LocalOp::Insert{pointer, value: value.local_value()})
                    }
                    list::LocalOp::Remove{idx} => {
                        pointer.push(LocalUid::Array(idx));
                        Some(LocalOp::Remove{pointer})
                    }
                }
            }
            OpInner::String(op) => {
                let changes = inner.as_text().ok()?.execute_op(op);
                if changes.is_empty() { return None };
                Some(LocalOp::ReplaceText{pointer, changes})
            }
        }
    }

    pub fn merge(&mut self, other: Inner, summary: &Summary, other_summary: &Summary) {
        self.nested_merge(other, summary, other_summary).unwrap()
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        self.nested_add_site_id(site_id)
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        self.nested_validate_no_unassigned_sites()
    }

    pub fn local_value(&self) -> SJValue {
        match *self {
            Inner::Object(ref map_inner) => {
                let mut map = serde_json::Map::with_capacity(map_inner.len());
                for (k, v) in map_inner.iter() {
                    let _ = map.insert(k.clone(), v[0].value.local_value());
                }
                SJValue::Object(map)
            }
            Inner::Array(ref list_inner) =>
                SJValue::Array(list_inner.iter().map(|v| v.value.local_value()).collect()),
            Inner::String(ref text_value) =>
                SJValue::String(text_value.local_value()),
            Inner::Number(float) => {
                let number = serde_json::Number::from_f64(float).unwrap();
                SJValue::Number(number)
            }
            Inner::Bool(bool_value) =>
                SJValue::Bool(bool_value),
            Inner::Null =>
                SJValue::Null,
        }
    }

    fn split_pointer(pointer_str: &str) -> Result<Vec<&str>, Error> {
        if !(pointer_str.is_empty() || pointer_str.starts_with("/")) {
            return Err(Error::DoesNotExist)
        }
        Ok(pointer_str.split("/").skip(1).collect())
    }

    fn get_nested_local(&mut self, pointer: &[&str]) -> Result<(&mut Inner, Vec<Uid>), Error> {
        let mut value = Some(self);
        let mut remote_pointer = vec![];

        for key in pointer {
            value = match value.unwrap() {
                &mut Inner::Object(ref mut map_value) => {
                    let element = map_value.get_mut(*key).ok_or(Error::DoesNotExist)?;
                    let uid = Uid::Object(key.to_string(), element.dot);
                    remote_pointer.push(uid);
                    Some(&mut element.value)
                }
                &mut Inner::Array(ref mut list_inner) => {
                    let idx = usize::from_str(key)?;
                    let element = list_inner.0.get_mut(idx).ok_or(Error::DoesNotExist)?;
                    let uid = Uid::Array(element.uid.clone());
                    remote_pointer.push(uid);
                    Some(&mut element.value)
                }
                _ => return Err(Error::DoesNotExist),
            }
        }

        Ok((value.unwrap(), remote_pointer))
    }

    fn get_nested_remote(&mut self, pointer: &[Uid]) -> Option<(&mut Inner, Vec<LocalUid>)> {
        let mut value = Some(self);
        let mut local_pointer = vec![];

        for uid in pointer {
            value = match (value.unwrap(), uid) {
                (&mut Inner::Object(ref mut map), &Uid::Object(ref key, dot)) => {
                    let element = map.get_mut_element(key, dot)?;
                    local_pointer.push(LocalUid::Object(key.clone()));
                    Some(&mut element.value)
                }
                (&mut Inner::Array(ref mut list), &Uid::Array(ref uid)) => {
                    let idx = list.get_idx(uid)?;
                    let element = list.get_mut(idx).unwrap();
                    local_pointer.push(LocalUid::Array(idx));
                    Some(&mut element.value)
                }
                _ => return None
            }
        }

        Some((value.unwrap(), local_pointer))
    }

    fn as_map(&mut self) -> Result<&mut MapInner<String, Inner>, Error> {
        match *self {
            Inner::Object(ref mut map_value) => Ok(map_value),
            _ => Err(Error::WrongJsonType)
        }
    }

    fn as_list(&mut self) -> Result<&mut ListInner<Inner>, Error> {
        match *self {
            Inner::Array(ref mut list_inner) => Ok(list_inner),
            _ => Err(Error::WrongJsonType)
        }
    }

    fn as_text(&mut self) -> Result<&mut TextInner, Error> {
        match *self {
            Inner::String(ref mut text_value) => Ok(text_value),
            _ => Err(Error::WrongJsonType)
        }
    }
}

impl NestedInner for Inner {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        match *self {
            Inner::Object(ref mut map) => map.nested_add_site_id(site_id),
            Inner::Array(ref mut list) => list.nested_add_site_id(site_id),
            Inner::String(ref mut text) => text.add_site_id(site_id),
            _ => (),
        }
    }

    fn nested_validate_no_unassigned_sites(&self) -> Result<(), Error> {
        match *self {
            Inner::Object(ref map) => map.nested_validate_no_unassigned_sites(),
            Inner::Array(ref list) => list.nested_validate_no_unassigned_sites(),
            Inner::String(ref text) => text.validate_no_unassigned_sites(),
            _ => Ok(())
        }
    }

    fn nested_validate_all(&self, site_id: SiteId) -> Result<(), Error> {
        match *self {
            Inner::Object(ref map) => map.nested_validate_all(site_id),
            Inner::Array(ref list) => list.nested_validate_all(site_id),
            Inner::String(ref text) => text.validate_all(site_id),
            _ => Ok(())
        }
    }

    fn nested_can_merge(&self, other: &Inner) -> bool {
        match (self, other) {
            (&Inner::Object(ref v1), &Inner::Object(ref v2)) => v1.nested_can_merge(v2),
            (&Inner::Array(ref v1), &Inner::Array(ref v2)) => v1.nested_can_merge(v2),
            (&Inner::String(_), &Inner::String(_)) => true,
            _ => false,
        }
    }

    fn nested_force_merge(&mut self, other: Inner, summary: &Summary, other_summary: &Summary) {
        match other {
            Inner::Object(other_map) => {
                self.as_map().unwrap().nested_force_merge(other_map, summary, other_summary);
            }
            Inner::Array(other_list) => {
                self.as_list().unwrap().nested_force_merge(other_list, summary, other_summary);
            }
            Inner::String(other_text) =>
                self.as_text().unwrap().merge(other_text, summary, other_summary),
            _ => (),
        }
    }
}


impl Op {
    fn add_site_id(&mut self, site_id: SiteId) {
        self.nested_add_site_id(site_id)
    }

    fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        self.nested_validate(site_id)
    }

    fn inserted_dots(&self) -> Vec<Dot> {
        match self.op {
            OpInner::Object(ref op) => op.inserted_dots(),
            OpInner::Array(ref op) => op.inserted_dots(),
            OpInner::String(ref op) => op.inserted_dots(),
        }
    }
}

impl NestedOp for Op {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        // update site ids in the pointer
        for uid in self.pointer.iter_mut() {
            match *uid {
                Uid::Object(_, ref mut dot) => {
                    if dot.site_id == 0 { dot.site_id = site_id; }
                }
                Uid::Array(ref mut uid) => {
                    if uid.site_id == 0 { uid.site_id = site_id; }
                }
            }
        }

        // update sites in the op
        match self.op {
            OpInner::Object(ref mut op) => op.nested_add_site_id(site_id),
            OpInner::Array(ref mut op) => op.nested_add_site_id(site_id),
            OpInner::String(ref mut op) => op.add_site_id(site_id),
        }
    }

    fn nested_validate(&self, site_id: SiteId) -> Result<(), Error> {
        match self.op {
            OpInner::Object(ref op) => op.nested_validate(site_id),
            OpInner::Array(ref op) => op.nested_validate(site_id),
            OpInner::String(ref op) => op.validate(site_id),
        }
    }
}


impl IntoJson for Inner {
    #[inline]
    fn into_json(self, _: Dot) -> Result<Inner, Error> {
        Ok(self)
    }
}

impl IntoJson for SJValue {
    fn into_json(self, dot: Dot) -> Result<Inner, Error> {
        match self {
            SJValue::Object(map) => {
                let mut map_value = MapInner::new();
                for (key, value) in map.into_iter() {
                    let _ = map_value.insert(key, value.into_json(dot)?, dot);
                }
                Ok(Inner::Object(map_value))
            }
            SJValue::Array(vec) =>
                vec.into_json(dot),
            SJValue::String(string) =>
                string.into_json(dot),
            SJValue::Number(number) =>
                number.as_f64().ok_or(Error::InvalidJson)?.into_json(dot),
            SJValue::Bool(bool_value) =>
                Ok(Inner::Bool(bool_value)),
            SJValue::Null =>
                Ok(Inner::Null),
        }
    }
}

impl<S: Into<String> + Hash + Eq, T: IntoJson> IntoJson for HashMap<S, T> {
    fn into_json(self, dot: Dot) -> Result<Inner, Error> {
        let mut map_value = MapInner::new();
        for (key, value) in self.into_iter() {
            let _ = map_value.insert(key.into(), value.into_json(dot)?, dot);
        }
        Ok(Inner::Object(map_value))
    }
}

impl<T: IntoJson> IntoJson for Vec<T> {
    fn into_json(self, dot: Dot) -> Result<Inner, Error> {
        let mut list_inner = ListInner::new();
        for (idx, elt) in self.into_iter().enumerate() {
            let _ = list_inner.insert(idx, elt.into_json(dot)?, dot);
        }
        Ok(Inner::Array(list_inner))
    }
}

impl<'a> IntoJson for &'a str {
    fn into_json(self, dot: Dot) -> Result<Inner, Error> {
        let mut text = TextInner::new();

        if !self.is_empty() {
            let _ = text.replace(0, 0, self, dot);
        }

        Ok(Inner::String(text))
    }
}

impl IntoJson for f64 {
    fn into_json(self, _: Dot) -> Result<Inner, Error> {
        match f64::is_finite(self) {
            true => Ok(Inner::Number(self)),
            false => Err(Error::InvalidJson),
        }
    }
}

impl IntoJson for i64 {
    fn into_json(self, _: Dot) -> Result<Inner, Error> {
        Ok(Inner::Number(self as f64))
    }
}

impl IntoJson for bool {
    fn into_json(self, _: Dot) -> Result<Inner, Error> {
        Ok(Inner::Bool(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmp_serde;

    #[test]
    fn test_from_str() {
        let crdt = Json::from_str(r#"{"foo":123, "bar":true, "baz": [1.0,2.0,3.0]}"#).unwrap();
        assert_matches!(crdt.value, Inner::Object(_));
        assert!(crdt.replica.site_id == 1);
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
        let op1 = crdt.insert_str("/foo", r#"{"bar": 3.5}"#).unwrap();
        let op2 = crdt.insert("/foo/baz", true).unwrap();

        assert!(crdt.replica.counter == 3);
        assert!(*nested_value(&mut crdt, "/foo/bar").unwrap() == Inner::Number(3.5));
        assert!(*nested_value(&mut crdt, "/foo/baz").unwrap() == Inner::Bool(true));

        assert!(op1.pointer.is_empty());
        assert_matches!(op1.op, OpInner::Object(map::Op::Insert{key: _, element: _, removed: _}));

        assert!(op2.pointer.len() == 1);
        assert!(op2.pointer[0] == Uid::Object("foo".to_owned(), Replica::new(1,1)));
        assert_matches!(op2.op, OpInner::Object(map::Op::Insert{key: _, element: _, removed: _}));
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
        let op = crdt.insert("/foo", 4.6).unwrap();

        assert!(crdt.replica.counter == 3);
        assert!(*nested_value(&mut crdt, "/foo").unwrap() == Inner::Number(4.6));

        assert!(op.pointer.is_empty());
        let (key, element, removed) = map_insert_op_fields(op);
        assert!(key == "foo");
        assert!(element.0 == Replica::new(1,2));
        assert!(element.1 == Inner::Number(4.6));
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
        assert!(*nested_value(&mut crdt2, "/foo").unwrap() == Inner::Number(19.7));
    }

    #[test]
    fn test_object_remove() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let op = crdt.remove("/abc/2/def").unwrap();

        assert!(nested_value(&mut crdt, "abc/2/def").is_none());
        assert!(op.pointer.len() == 2);
        assert!(op.pointer[0] == Uid::Object("abc".to_owned(), Replica::new(1,0)));
        assert_matches!(op.pointer[1], Uid::Array(_));

        let (key, removed) = map_remove_op_fields(op);
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
        let op = crdt.insert("/things/1/0", true).unwrap();
        let element = list_insert_op_element(op);
        assert!(*nested_value(&mut crdt, "/things/1/0").unwrap() == Inner::Bool(true));
        assert!(crdt.replica.counter == 2);
        assert!(element.1 == Inner::Bool(true));
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
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == Inner::Bool(true));
    }

    #[test]
    fn test_array_remove() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let op = crdt.remove("/things/1/2").unwrap();
        let uid = list_remove_op_uid(op);
        assert!(nested_value(&mut crdt, "/things/1/2").is_none());
        assert!(crdt.replica.counter == 2);
        assert!(uid.site_id == 1 && uid.counter == 0);
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

        let op = crdt2.awaiting_site.pop().unwrap();
        let uid = list_remove_op_uid(op);
        assert!(*nested_value(&mut crdt2, "/things/1").unwrap() == Inner::Number(2.0));
        assert!(uid.site_id == 1 && uid.counter == 0);
    }

    #[test]
    fn test_replace_text() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let op = crdt.replace_text("/1", 1, 2, "åⱡ").unwrap();
        let op = text_op(op);
        assert!(local_json(crdt.value()) == r#"[5.0,"håⱡlo"]"#);
        assert!(op.removes.len() == 1);
        assert!(op.inserts[0].text == "h");
        assert!(op.inserts[1].text == "åⱡ");
        assert!(op.inserts[2].text == "lo");
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

        let op = text_op(crdt.awaiting_site.pop().unwrap());
        assert!(op.removes.len() == 1);
        assert!(op.inserts[0].text == "h");
        assert!(op.inserts[1].text == "åⱡ");
        assert!(op.inserts[2].text == "lo");
    }

    #[test]
    fn test_execute_op_object() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op = crdt1.insert("/baz", 54.0).unwrap();
        let local_op  = crdt2.execute_op(&op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::Insert{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUid::Object("baz".to_owned())]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_op_array() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op = crdt1.insert("/foo/0", 54.0).unwrap();
        let local_op  = crdt2.execute_op(&op).unwrap();


        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::Insert{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUid::Object("foo".to_owned()),LocalUid::Array(0)]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_op_string() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op = crdt1.replace_text("/foo/2", 1, 2, "ab").unwrap();
        let local_op  = crdt2.execute_op(&op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        if let LocalOp::ReplaceText{pointer, ..} = local_op {
            assert_eq!(pointer, [LocalUid::Object("foo".to_owned()), LocalUid::Array(2)]);
        } else {
            panic!("expected an insert op");
        }
    }

    #[test]
    fn test_execute_op_missing_pointer() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let op = crdt1.remove("/bar").unwrap();
        let _         = crdt2.remove("/bar").unwrap();
        assert!(crdt2.execute_op(&op).is_none());
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
        let _ = crdt2.replace_text("/bar", 5, 0, " everyone!");
        let _ = crdt2.replace_text("/bar", 0, 1, "");
        let _ = crdt2.remove("/baz/abc/2");
        let _ = crdt2.remove("/foo");

        let mut ops = crdt2.add_site(11).unwrap().into_iter();

        assert!(crdt2.local_value() == json!({"bar":"ello everyone!", "baz":{"abc":[true, 61.0, 84.0]}}));
        assert!(crdt2.site_id() == 11);

        // check that the CRDT's elements have the correct sites

        {
            let map = as_map(&crdt2.value);
            assert!(map.0.get("foo").is_none());
            assert!(map.0.get("bar").unwrap()[0].0.site_id == 1);
            assert!(map.0.get("baz").unwrap()[0].0.site_id == 11);
        }
        {
            let text = as_text(nested_value(&mut crdt2, "/bar").unwrap());
            let mut text_elements = text.0.iter();
            assert!(text_elements.next().unwrap().uid.site_id == 11);
            assert!(text_elements.next().unwrap().uid.site_id == 11);
        }
        {
            let list = as_list(nested_value(&mut crdt2, "/baz/abc").unwrap());
            assert!((list.0.get_elt(0).unwrap().0).0.site_id == 11);
            assert!((list.0.get_elt(1).unwrap().0).0.site_id == 11);
            assert!((list.0.get_elt(2).unwrap().0).0.site_id == 11);
        }

        // check that the remote ops' elements have the correct sites
        let (_, element, replicas) = map_insert_op_fields(ops.next().unwrap());
        assert!(element.0.site_id == 11);
        assert!(element.1.validate_site(11).is_ok());
        assert!(replicas.is_empty());

        let element = list_insert_op_element(ops.next().unwrap());
        assert!(element.0.site_id == 11);
        assert!(element.1.validate_site(11).is_ok());

        let element = text_op(ops.next().unwrap());
        assert!(element.removes.is_empty());
        assert!(element.inserts[0].uid.site_id == 11);

        let element = text_op(ops.next().unwrap());
        assert!(element.removes[0].site_id == 1);
        assert!(element.inserts[0].uid.site_id == 11);

        let uid = list_remove_op_uid(ops.next().unwrap());
        assert!(uid.site_id == 11);

        let (_, replicas) = map_remove_op_fields(ops.next().unwrap());
        assert!(replicas[0].site_id == 1);
    }

    #[test]
    fn test_add_site_nested() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let _ = crdt2.insert("/foo", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        }));

        let mut ops = crdt2.add_site(22).unwrap().into_iter();
        assert!(crdt2.site_id() == 22);

        let object = nested_value(&mut crdt2, "/foo").unwrap();
        assert!(object.validate_site(22).is_ok());

        let (_, element, replicas) = map_insert_op_fields(ops.next().unwrap());
        assert!(element.0.site_id == 22);
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
    fn test_execute_op_dupe() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op = crdt1.remove("/bar").unwrap();
        assert!(crdt2.execute_op(&op).is_some());
        assert!(crdt2.execute_op(&op).is_none());
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
        let value2: Inner = serde_json::from_str(&s_json).unwrap();
        let value3: Inner = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*crdt.value() == value2);
        assert!(*crdt.value() == value3);
    }

    #[test]
    fn test_serialize_op() {
        let mut crdt = Json::from_str(r#"{"foo":{}}"#).unwrap();
        let op1 = crdt.insert("/foo/bar", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        })).unwrap();

        let s_json = serde_json::to_string(&op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&op1).unwrap();
        let op2: Op = serde_json::from_str(&s_json).unwrap();
        let op3: Op = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(op1 == op2);
        assert!(op1 == op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let mut crdt1 = Json::from_str(r#"{"foo":{}}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let op = crdt1.insert("/foo/bar", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        })).unwrap();
        let local_op1 = crdt2.execute_op(&op).unwrap();

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert_eq!(s_json, r#"{"op":"insert","pointer":["foo","bar"],"value":{"a":[[1.0],["hello everyone!"],{"x":3.0}],"b":{"cat":true,"dog":false}}}"#);
        assert_eq!(local_op1, local_op2);
        assert_eq!(local_op1, local_op3);
    }

    fn nested_value<'a>(crdt: &'a mut Json, pointer: &str) -> Option<&'a Inner> {
        let pointer = try_opt!(Inner::split_pointer(pointer).ok());
        let (value, _) = try_opt!(crdt.value.get_nested_local(&pointer).ok());
        Some(value)
    }

    fn local_json(json_value: &Inner) -> String {
        serde_json::to_string(&json_value.local_value()).unwrap()
    }

    fn map_insert_op_fields(op: Op) -> (String, map::Element<Inner>, Vec<Replica>) {
        match op.op {
            OpInner::Object(map::Op::Insert{key: k, element: e, removed: r}) => (k, e, r),
            _ => panic!(),
        }
    }

    fn map_remove_op_fields(op: Op) -> (String, Vec<Replica>) {
        match op.op {
            OpInner::Object(map::Op::Remove{key: k, removed: r}) => (k, r),
            _ => panic!(),
        }
    }

    fn list_insert_op_element(op: Op) -> list::Element<Inner> {
        match op.op {
            OpInner::Array(list::Op::Insert(element)) => element,
            _ => panic!(),
        }
    }

    fn list_remove_op_uid(op: Op) -> sequence::uid::UID {
        match op.op {
            OpInner::Array(list::Op::Remove(uid)) => uid,
            _ => panic!(),
        }
    }

    fn text_op(op: Op) -> text::Op {
        match op.op {
            OpInner::String(op) => op,
            _ => panic!(),
        }
    }

    fn as_map(json_value: &Inner) -> &MapInner<String, Inner> {
        match *json_value {
            Inner::Object(ref map_value) => map_value,
            _ => panic!(),
        }
    }

    fn as_list(json_value: &Inner) -> &ListInner<Inner> {
        match *json_value {
            Inner::Array(ref list_inner) => list_inner,
            _ => panic!(),
        }
    }

    fn as_text(json_value: &Inner) -> &TextInner {
        match *json_value {
            Inner::String(ref text_value) => text_value,
            _ => panic!(),
        }
    }
}
