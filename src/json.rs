//! A CRDT that stores a JSON value.

use Error;
use dot::{Dot, Summary, SiteId};
use list::{self, Inner as ListInner};
use map::{self, Inner as MapInner};
use text::{self, Inner as TextInner};
use sequence;
use traits::*;

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
/// CRDTs. It allows op-based replication via [`execute_op`](#method.execute_op)
/// and state-based replication via [`merge`](#method.merge).
/// State-based replication allows out-of-order delivery but
/// op-based replication does not.
///
/// The root value of a Json CRDT (typically an object or array) cannot
/// be replaced; for example, a Json CRDT whose root is an array will
/// always have an array as its root. This constraint means that any Json
/// CRDT with a numeric, boolean, or null root is immutable.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Json {
    inner:          Inner,
    summary:        Summary,
    site_id:        SiteId,
    outoforder_ops: Vec<Op>,
    cached_ops:     Vec<Op>,
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
    Array(sequence::uid::Uid),
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
        let mut summary = Summary::default();
        let dot = summary.get_dot(site_id);
        let inner = local_value.into_json(dot)?;
        Ok(Json{inner, summary, site_id, outoforder_ops: vec![], cached_ops: vec![]})
    }

    /// Constructs and returns a new `Json` CRDT with site 1 from an
    /// unparsed JSON string.
    pub fn from_str(json_str: &str) -> Result<Self, Error> {
        let local_value: SJValue = serde_json::from_str(json_str)?;
        let crdt = Json::new(local_value)?;
        Ok(crdt)
    }

    /// Returns the number of elements in a container at the given
    /// pointer in the `Json` CRDT. If there is no container at the
    /// given pointer, returns `None`.
    pub fn len(&self, pointer: &str) -> Option<usize> {
        let pointer = Inner::split_pointer(pointer).ok()?;
        match *self.inner.get_nested_local(&pointer)? {
            Inner::Object(ref map) => Some(map.len()),
            Inner::Array(ref list) => Some(list.0.len()),
            Inner::String(ref text) => Some(text.len()),
            _ => None,
        }
    }

    /// Inserts a value into the Json CRDT at the given json pointer.
    /// The enclosing value may be an object or an array and the
    /// inserted value must satisfy the [`IntoJson`](IntoJson.t.html) trait.
    ///
    /// If the CRDT does not have a site id allocated, it caches
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
    /// If the CRDT does not have a site id allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert_str(&mut self, pointer: &str, value: &str) -> Result<Op, Error> {
        let json: SJValue = serde_json::from_str(value)?;
        self.insert(pointer, json)
    }

    /// Removes a value at the given JSON pointer from the Json CRDT.
    /// If the enclosing value is an object, it deletes the key-value
    /// pair. If the enclosing value is an array, it deletes the value
    /// at the array index.
    ///
    /// If the CRDT does not have a site id allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, pointer: &str) -> Result<Op, Error> {
        let op = self.inner.remove(pointer)?;
        self.after_op(op)
    }

    /// Replaces a text range in a text value in the Json CRDT.
    /// If the CRDT does not have a site id allocated, it caches
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
        let (json_value, remote_pointer) = self.mut_nested_local(&pointer)?;
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
        let (json_value, remote_pointer) = self.mut_nested_local(&pointer)?;

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
        let (inner, remote_pointer) = self.mut_nested_local(&pointer)?;
        let text_inner = inner.as_text()?;
        let op = text_inner.replace(index, len, text, dot).ok_or(Error::Noop)?;
        Ok(Op{pointer: remote_pointer, op: OpInner::String(op)})
    }

    pub fn execute_op(&mut self, op: Op) -> Option<LocalOp> {
        let (inner, mut pointer) = self.get_nested_remote(&op.pointer)?;
        match op.op {
            OpInner::Object(op) => {
                let map_inner = inner.as_map().ok()?;
                if !map_inner.0.contains_key(op.key()) && op.inserted_element().is_none() {
                    return None
                }
                match map_inner.execute_op(op) {
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
                if changes.is_empty() { return vec![] };
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
        if !(pointer_str.is_empty() || pointer_str.starts_with('/')) {
            return Err(Error::DoesNotExist)
        }
        Ok(pointer_str.split('/').skip(1).collect())
    }

    fn get_nested_local(&self, pointer: &[&str]) -> Option<&Inner> {
        let mut value = self;

        for key in pointer {
            value = match *value {
                Inner::Object(ref map_inner) =>
                    &map_inner.0.get(*key)?[0].value,
                Inner::Array(ref list_inner) => {
                    let idx = usize::from_str(key).ok()?;
                    let element = list_inner.0.get(idx)?;
                    &element.value
                }
                _ => return None,
            };
        }

        Some(value)
    }

    fn mut_nested_local(&mut self, pointer: &[&str]) -> Result<(&mut Inner, Vec<Uid>), Error> {
        let mut value = Some(self);
        let mut remote_pointer = vec![];

        for key in pointer {
            value = match *value.unwrap() {
                Inner::Object(ref mut map_inner) => {
                    let element = map_inner.get_mut(*key).ok_or(Error::DoesNotExist)?;
                    let uid = Uid::Object(key.to_string(), element.dot);
                    remote_pointer.push(uid);
                    Some(&mut element.value)
                }
                Inner::Array(ref mut list_inner) => {
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
            (&Inner::String(_), &Inner::String(_)) |
            (&Inner::Number(_), &Inner::Number(_)) |
            (&Inner::Bool(_),   &Inner::Bool(_))   |
            (&Inner::Null,      &Inner::Null)      => true,
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

    fn removed_dots(&self) -> Vec<Dot> {
        match self.op {
            OpInner::Object(ref op) => op.removed_dots(),
            OpInner::Array(ref op) => op.removed_dots(),
            OpInner::String(ref op) => op.removed_dots(),
        }
    }
}

impl NestedOp for Op {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        // update site ids in the pointer
        for uid in &mut self.pointer {
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
                let mut map_value = MapInner::with_capacity(map.len());
                for (key, value) in map {
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
        let mut map_value = MapInner::with_capacity(self.len());
        for (key, value) in self {
            let _ = map_value.insert(key.into(), value.into_json(dot)?, dot);
        }
        Ok(Inner::Object(map_value))
    }
}

impl<T: IntoJson> IntoJson for Vec<T> {
    fn into_json(self, dot: Dot) -> Result<Inner, Error> {
        let mut list_inner = ListInner::with_capacity(self.len());
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
        if f64::is_finite(self) { Ok(Inner::Number(self)) } else { Err(Error::InvalidJson) }
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

    #[test]
    fn test_from_str() {
        let crdt = Json::from_str(r#"{"foo":123, "bar":true, "baz": [1.0,2.0,3.0]}"#).unwrap();
        assert_matches!(crdt.inner, Inner::Object(_));
        assert_eq!(crdt.site_id, 1);
        assert_eq!(crdt.summary.get(1), 1);
        assert_eq!(crdt.cached_ops, vec![]);
    }

    #[test]
    fn test_from_str_invalid() {
        let invalid_json_str = r#"{"foo":123, "bar":true, "baz": [1.0,2.0,3.0]"#;
        assert_eq!(Json::from_str(invalid_json_str), Err(Error::InvalidJson));
    }

    #[test]
    fn test_object_insert() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let op1 = crdt.insert_str("/foo", r#"{"bar": 3.5}"#).unwrap();
        let op2 = crdt.insert("/foo/baz", true).unwrap();

        assert_eq!(crdt.summary.get(crdt.site_id), 3);
        assert_eq!(nested_value(&mut crdt, "/foo/bar"), Some(&Inner::Number(3.5)));
        assert_eq!(nested_value(&mut crdt, "/foo/baz"), Some(&Inner::Bool(true)));

        assert_eq!(op1.pointer, vec![]);
        assert_matches!(op1.op, OpInner::Object(_));

        assert_eq!(op2.pointer, vec![Uid::Object("foo".to_owned(), Dot::new(1,2))]);
        assert_matches!(op2.op, OpInner::Object(_));
    }

    #[test]
    fn test_object_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let result = crdt.insert_str("/foo/bar", r#"{"bar": 3.5}"#);
        assert_eq!(result, Err(Error::DoesNotExist));
    }

    #[test]
    fn test_object_insert_replaces_value() {
        let mut crdt = Json::from_str(r#"{}"#).unwrap();
        let _ = crdt.insert("/foo", 19.7).unwrap();
        let op = crdt.insert("/foo", 4.6).unwrap();

        assert_eq!(crdt.summary.get(1), 3);
        assert_eq!(nested_value(&mut crdt, "/foo"), Some(&Inner::Number(4.6)));
        assert_eq!(op.pointer, vec![]);

        let map_op = map_op(op.op);
        assert_eq!(map_op.key(), "foo");
        assert_eq!(map_op.inserted_element().unwrap().dot, Dot::new(1,3));
        assert_eq!(map_op.inserted_element().unwrap().value, Inner::Number(4.6));
        assert_eq!(map_op.removed_dots(), [Dot::new(1,2)]);
    }

    #[test]
    fn test_object_insert_same_value() {
        let mut crdt = Json::from_str("{}").unwrap();
        assert_matches!(crdt.insert("/foo", 19.7), Ok(_));
        assert_matches!(crdt.insert("/foo", 19.7), Ok(_));
    }

    #[test]
    fn test_object_insert_awaiting_site_id() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();

        assert_eq!(crdt2.insert("/foo", 19.7), Err(Error::AwaitingSiteId));
        assert_eq!(crdt2.cached_ops.len(), 1);
        assert_eq!(nested_value(&mut crdt2, "/foo"), Some(&Inner::Number(19.7)));
    }

    #[test]
    fn test_object_remove() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let op = crdt.remove("/abc/2/def").unwrap();

        assert_eq!(nested_value(&mut crdt, "abc/2/def"), None);
        assert_eq!(op.pointer.len(), 2);
        assert_eq!(op.pointer[0], Uid::Object("abc".to_owned(), Dot::new(1,1)));
        assert_matches!(op.pointer[1], Uid::Array(_));

        let map_op = map_op(op.op);
        assert_eq!(map_op.key(), "def");
        assert_eq!(map_op.inserted_element(), None);
        assert_eq!(map_op.removed_dots(), [Dot::new(1,1)]);
    }

    #[test]
    fn test_object_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        assert_eq!(crdt.remove("/uhoh/11/def"), Err(Error::DoesNotExist));
    }

    #[test]
    fn test_object_remove_does_not_exist() {
        let mut crdt = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        assert_eq!(crdt.remove("/abc/2/zebra!"), Err(Error::Noop));
    }

    #[test]
    fn test_object_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"abc":[1.5,true,{"def":false}]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert_eq!(crdt2.remove("/abc/2/def"), Err(Error::AwaitingSiteId));
        assert_eq!(crdt2.cached_ops.len(), 1);
        assert_eq!(nested_value(&mut crdt2, "/abc/2/def"), None);
    }

    #[test]
    fn test_array_insert() {
        let mut crdt = Json::from_str(r#"{"things":[1,[],2,3]}"#).unwrap();
        let op = crdt.insert("/things/1/0", true).unwrap();
        let element = list_insert_op_element(op);
        assert_eq!(nested_value(&mut crdt, "/things/1/0"), Some(&Inner::Bool(true)));
        assert_eq!(crdt.summary.get(1), 2);
        assert_eq!(element.value, Inner::Bool(true));
    }

    #[test]
    fn test_array_insert_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        assert_eq!(crdt.insert("/others/1", true), Err(Error::DoesNotExist));
    }

    #[test]
    #[should_panic]
    fn test_array_insert_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        let _ = crdt.insert("/things/4", true);
    }

    #[test]
    fn test_array_insert_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,2,3]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert_eq!(crdt2.insert("/things/1", true), Err(Error::AwaitingSiteId));
        assert_eq!(crdt2.cached_ops.len(), 1);
        assert_eq!(nested_value(&mut crdt2, "/things/1"), Some(&Inner::Bool(true)));
    }

    #[test]
    fn test_array_remove() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let op = crdt.remove("/things/1/2").unwrap();
        let uid = list_remove_op_uid(op);
        assert_eq!(nested_value(&mut crdt, "/things/1/2"), None);
        assert_eq!(crdt.summary.get(1), 1);
        assert_eq!(uid.site_id, 1);
        assert_eq!(uid.counter, 1);
    }

    #[test]
    fn test_array_remove_invalid_pointer() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        assert_eq!(crdt.remove("/things/5/2"), Err(Error::DoesNotExist));
    }

    #[test]
    #[should_panic]
    fn test_array_remove_out_of_bounds() {
        let mut crdt = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let _ = crdt.remove("/things/1/3");
    }

    #[test]
    fn test_array_remove_awaiting_site() {
        let crdt1 = Json::from_str(r#"{"things":[1,[true,false,"hi"],2,3]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        assert_eq!(crdt2.remove("/things/1"), Err(Error::AwaitingSiteId));

        let op = crdt2.cached_ops.pop().unwrap();
        let uid = list_remove_op_uid(op);
        assert_eq!(nested_value(&mut crdt2, "/things/1"), Some(&Inner::Number(2.0)));
        assert_eq!(uid.site_id, 1);
        assert_eq!(uid.counter, 1);
    }

    #[test]
    fn test_replace_text() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let op = crdt.replace_text("/1", 1, 2, "åⱡ").unwrap();
        let op = text_op(op);
        assert_eq!(local_json(&crdt.inner), r#"[5.0,"håⱡlo"]"#);
        assert_eq!(op.removed_uids().len(), 1);
        assert_eq!(op.inserted_elements()[0].text, "h");
        assert_eq!(op.inserted_elements()[1].text, "åⱡ");
        assert_eq!(op.inserted_elements()[2].text, "lo");
    }

    #[test]
    fn test_replace_text_invalid_pointer() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        assert_eq!(crdt.replace_text("/0", 1, 2, "åⱡ"), Err(Error::WrongJsonType));
    }

    #[test]
    #[should_panic]
    fn test_replace_text_out_of_bounds() {
        let mut crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let _ = crdt.replace_text("/1", 1, 6, "åⱡ");
    }

    #[test]
    fn test_replace_text_awaiting_site() {
        let remote_crdt = Json::from_str(r#"[5.0,"hello"]"#).unwrap();
        let mut crdt = Json::from_state(remote_crdt.clone_state(), None).unwrap();
        assert_eq!(crdt.replace_text("/1", 1, 2, "åⱡ"), Err(Error::AwaitingSiteId));
        assert_eq!(local_json(&crdt.inner), r#"[5.0,"håⱡlo"]"#);

        let op = text_op(crdt.cached_ops.pop().unwrap());
        assert_eq!(op.removed_uids().len(), 1);
        assert_eq!(op.inserted_elements()[0].text, "h");
        assert_eq!(op.inserted_elements()[1].text, "åⱡ");
        assert_eq!(op.inserted_elements()[2].text, "lo");
    }

    #[test]
    fn test_execute_op_object() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op = crdt1.insert("/baz", 54.0).unwrap();
        let local_op = crdt2.execute_op(op).unwrap();

        assert_eq!(crdt1.state(), crdt2.state());
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
        let local_op  = crdt2.execute_op(op).unwrap();


        assert_eq!(crdt1.state(), crdt2.state());
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
        let local_op  = crdt2.execute_op(op).unwrap();

        assert_eq!(crdt1.state(), crdt2.state());
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
        let _  = crdt2.remove("/bar").unwrap();
        assert_eq!(crdt2.execute_op(op), None);
    }

    #[test]
    fn test_merge() {
        let mut crdt1 = Json::from_str(r#"{"x":[{"a": 1},{"b": 2},{"c":true},{"d":false}]}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), Some(2)).unwrap();
        let _ = crdt1.insert("/x/0/e", 222.0).unwrap();
        let _ = crdt1.insert("/x/3/f", 333.0).unwrap();
        let _ = crdt1.remove("/x/2").unwrap();
        let _ = crdt2.insert("/x/1/g", 444.0).unwrap();
        let _ = crdt2.remove("/x/3").unwrap();

        let crdt1_state = crdt1.clone_state();
        assert_matches!(crdt1.merge(crdt2.clone_state()), Ok(_));
        assert_matches!(crdt2.merge(crdt1_state), Ok(_));
        assert_eq!(crdt1.state(), crdt2.state());
        assert_eq!(crdt1.local_value(), json!({"x":[{"a": 1.0, "e": 222.0}, {"b": 2.0, "g": 444.0}]}));
    }

    #[test]
    fn test_add_site_id() {
        let crdt1 = Json::from_str(r#"{"foo":[1,2,3],"bar":"hello"}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let _ = crdt2.insert("/baz", json!({"abc":[true, false, 84.0]}));
        let _ = crdt2.insert("/baz/abc/1", 61.0);
        let _ = crdt2.replace_text("/bar", 5, 0, " everyone!");
        let _ = crdt2.replace_text("/bar", 0, 1, "");
        let _ = crdt2.remove("/baz/abc/2");
        let _ = crdt2.remove("/foo");

        let mut ops = crdt2.add_site_id(11).unwrap().into_iter();

        assert_eq!(crdt2.local_value(), json!({"bar":"ello everyone!", "baz":{"abc":[true, 61.0, 84.0]}}));
        assert_eq!(crdt2.site_id(), 11);

        // check that the CRDT's elements have the correct sites

        {
            let map = as_map(&crdt2.inner);
            assert_eq!(map.0.get("foo"), None);
            assert_eq!(map.0.get("bar").unwrap()[0].dot.site_id, 1);
            assert_eq!(map.0.get("baz").unwrap()[0].dot.site_id, 11);
        }
        {
            let text = as_text(nested_value(&mut crdt2, "/bar").unwrap());
            let mut text_elements = text.0.iter();
            assert_eq!(text_elements.next().unwrap().uid.site_id, 11);
            assert_eq!(text_elements.next().unwrap().uid.site_id, 11);
        }
        {
            let list = as_list(nested_value(&mut crdt2, "/baz/abc").unwrap());
            assert_eq!(list.0[0].uid.site_id, 11);
            assert_eq!(list.0[1].uid.site_id, 11);
            assert_eq!(list.0[2].uid.site_id, 11);
        }

        // check that the remote ops' elements have the correct sites
        let map_op1 = map_op(ops.next().unwrap().op);
        assert_eq!(map_op1.inserted_element().unwrap().dot.site_id, 11);
        assert_eq!(map_op1.inserted_element().unwrap().value.nested_validate_all(11), Ok(()));
        assert_eq!(map_op1.removed_dots(), []);

        let element = list_insert_op_element(ops.next().unwrap());
        assert_eq!(element.uid.site_id, 11);
        assert_eq!(element.value.nested_validate_all(11), Ok(()));

        let element = text_op(ops.next().unwrap());
        assert_eq!(element.removed_uids(), []);
        assert_eq!(element.inserted_elements()[0].uid.site_id, 11);

        let element = text_op(ops.next().unwrap());
        assert_eq!(element.removed_uids()[0].site_id, 1);
        assert_eq!(element.inserted_elements()[0].uid.site_id, 11);

        let uid = list_remove_op_uid(ops.next().unwrap());
        assert_eq!(uid.site_id, 11);

        let map_op2 = map_op(ops.next().unwrap().op);
        assert_eq!(map_op2.removed_dots(), [Dot::new(1, 1)]);
    }

    #[test]
    fn test_add_site_nested() {
        let crdt1 = Json::from_str("{}").unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let _ = crdt2.insert("/foo", json!({
            "a": [[1.0],["hello everyone!"],{"x": 3.0}],
            "b": {"cat": true, "dog": false}
        }));

        let mut ops = crdt2.add_site_id(22).unwrap().into_iter();
        assert_eq!(crdt2.site_id(), 22);

        let object = nested_value(&mut crdt2, "/foo").unwrap();
        assert_eq!(object.nested_validate_all(22), Ok(()));

        let map_op = map_op(ops.next().unwrap().op);
        assert_eq!(map_op.inserted_element().unwrap().dot.site_id, 22);
        assert_eq!(map_op.inserted_element().unwrap().value.nested_validate_all(22), Ok(()));
        assert_eq!(map_op.removed_dots(), []);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut crdt = Json::from_str("{}").unwrap();
        let _ = crdt.insert("/foo", vec![1.0]).unwrap();
        let _ = crdt.insert("/foo/0", "hello").unwrap();
        let _ = crdt.replace_text("/foo/0", 5, 0, " everybody!").unwrap();
        assert_eq!(crdt.add_site_id(33), Err(Error::AlreadyHasSiteId));
    }

    #[test]
    fn test_execute_op_dupe() {
        let mut crdt1 = Json::from_str(r#"{"foo":[1.0,true,"hello"],"bar":null}"#).unwrap();
        let mut crdt2 = Json::from_state(crdt1.clone_state(), None).unwrap();
        let op1 = crdt1.remove("/bar").unwrap();
        let op2 = crdt1.remove("/foo/1").unwrap();
        let op3 = crdt1.replace_text("/foo/1",0,1,"H").unwrap();

        assert_matches!(crdt2.execute_op(op1.clone()), Some(_));
        assert_matches!(crdt2.execute_op(op2.clone()), Some(_));
        assert_matches!(crdt2.execute_op(op3.clone()), Some(_));

        assert_eq!(crdt2.execute_op(op1), None);
        assert_eq!(crdt2.execute_op(op2), None);
        assert_eq!(crdt2.execute_op(op3), None);
    }

    fn nested_value<'a>(crdt: &'a mut Json, pointer: &str) -> Option<&'a Inner> {
        let pointer = Inner::split_pointer(pointer).ok()?;
        let (value, _) = crdt.inner.mut_nested_local(&pointer).ok()?;
        Some(value)
    }

    fn local_json(json_value: &Inner) -> String {
        serde_json::to_string(&json_value.local_value()).unwrap()
    }

    fn map_op(inner_op: OpInner) -> map::Op<String, Inner> {
        match inner_op {
            OpInner::Object(map_op) => map_op,
            _ => panic!()
        }
    }

    fn list_insert_op_element(op: Op) -> list::Element<Inner> {
        match op.op {
            OpInner::Array(list::Op::Insert(element)) => element,
            _ => panic!(),
        }
    }

    fn list_remove_op_uid(op: Op) -> sequence::uid::Uid {
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
