//! An `Xml` CRDT stores an XML document.

use {Error, Replica, Tombstones};
use list::{self, ListValue};
use map::{self, MapValue};
use sequence::uid::UID as SequenceUid;
use text::{self, TextValue};
use traits::*;

use either::Either;
use quickxml_dom as dom;
use std::borrow::Cow;
use std::io::{Read, Cursor};
use std::str::FromStr;
use std::string::ToString;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Xml {
    value: XmlValue,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XmlState<'a> {
    value: Cow<'a, XmlValue>,
    tombstones: Cow<'a, Tombstones>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XmlValue {
    declaration: Declaration,
    root: Element,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum XmlVersion {
    Version10,
    Version11,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Child {
    Text(TextValue),
    Element(Element),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    name:       String,
    attributes: MapValue<String, String>,
    children:   ListValue<Child>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Declaration {
    version:    XmlVersion,
    encoding:   Option<String>,
    standalone: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp {
    pointer: Vec<SequenceUid>,
    op: RemoteOpInner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum RemoteOpInner {
    Attribute(map::RemoteOp<String, String>),
    Child(list::RemoteOp<Child>),
    ReplaceText(text::RemoteOp),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LocalOp {
    Insert{pointer: Vec<usize>, value: String},
    Remove{pointer: Vec<usize>},
    InsertAttribute{pointer: Vec<usize>, key: String, value: String},
    RemoveAttribute{pointer: Vec<usize>, key: String},
    ReplaceText{pointer: Vec<usize>, changes: Vec<text::LocalChange>},
}

impl Xml {
    crdt_impl!(Xml, XmlState, XmlState, XmlState<'static>, XmlValue);

    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self, Error> {
        let mut replica = Replica::new(1, 0);
        let local_xml = dom::Document::from_reader(&mut reader).map_err(|_| Error::InvalidXml)?;
        let value = into_xml(local_xml, &replica)?;
        let tombstones = Tombstones::new();
        replica.counter += 1;
        Ok(Xml{value, replica, tombstones, awaiting_site: vec![]})
    }

    pub fn from_str(xml_str: &str) -> Result<Self, Error> {
        Self::from_reader(Cursor::new(xml_str.as_bytes()))
    }

    pub fn insert<T: IntoXmlNode>(&mut self, pointer_str: &str, node: T) -> Result<RemoteOp, Error> {
        let op = self.value.insert(pointer_str, node, &self.replica)?;
        self.after_op(op)
    }

    pub fn remove(&mut self, pointer_str: &str) -> Result<RemoteOp, Error> {
        let op = self.value.remove(pointer_str)?;
        self.after_op(op)
    }

    pub fn insert_attribute(&mut self, pointer_str: &str, key: &str, value: &str) -> Result<RemoteOp, Error> {
        let op = self.value.insert_attribute(pointer_str, key, value, &self.replica)?;
        self.after_op(op)
    }

    pub fn remove_attribute(&mut self, pointer_str: &str, key: &str) -> Result<RemoteOp, Error> {
        let op = self.value.remove_attribute(pointer_str, key)?;
        self.after_op(op)
    }

    pub fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str) -> Result<RemoteOp, Error> {
        let op = self.value.replace_text(pointer_str, idx, len, text, &self.replica)?;
        self.after_op(op)
    }
}

impl XmlValue {
    fn insert<T: IntoXmlNode>(&mut self, pointer_str: &str, node: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let mut pointer = split_pointer(pointer_str)?;
        let idx = pointer.pop().ok_or(Error::InvalidPointer)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let element = child.left().ok_or(Error::InvalidPointer)?;
        let node = node.into_xml_child(replica)?;
        let op = element.children.insert(idx, node, replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
    }

    fn remove(&mut self, pointer_str: &str) -> Result<RemoteOp, Error> {
        let mut pointer = split_pointer(pointer_str)?;
        let idx = pointer.pop().ok_or(Error::InvalidPointer)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let element = child.left().ok_or(Error::InvalidPointer)?;
        let op = element.children.remove(idx)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
    }

    fn insert_attribute(&mut self, pointer_str: &str, key: &str, value: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        let pointer = split_pointer(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let element = child.left().ok_or(Error::InvalidPointer)?;
        let (key, value) = into_xml_attribute(key, value)?;
        let op = element.attributes.insert(key, value, replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
    }

    fn remove_attribute(&mut self, pointer_str: &str, key: &str) -> Result<RemoteOp, Error> {
        let pointer = split_pointer(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let element = child.left().ok_or(Error::InvalidPointer)?;
        let op = element.attributes.remove(key)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
    }

    fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        let pointer = split_pointer(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let text_value = child.right().ok_or(Error::InvalidPointer)?;
        let op = text_value.replace(idx, len, text, replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::ReplaceText(op)})
    }

    fn execute_remote(&mut self, remote_op: &RemoteOp) -> Option<LocalOp> {
        match remote_op.op {
            RemoteOpInner::Child(ref op) => {
                let (child, mut pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let element = try_opt!(child.left());
                match try_opt!(element.children.execute_remote(op)) {
                    list::LocalOp::Insert{index, value} => {
                        pointer.push(index);
                        let x: dom::Child = value.local_value();
                        let y = x.to_string();
                        Some(LocalOp::Insert{pointer, value: y})
                    }
                    list::LocalOp::Remove{index} => {
                        pointer.push(index);
                        Some(LocalOp::Remove{pointer})
                    }
                }
            }
            RemoteOpInner::Attribute(ref op) => {
                let (child, pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let element = try_opt!(child.left());
                match try_opt!(element.attributes.execute_remote(op)) {
                    map::LocalOp::Insert{key, value} =>
                        Some(LocalOp::InsertAttribute{pointer, key, value}),
                    map::LocalOp::Remove{key} =>
                        Some(LocalOp::RemoveAttribute{pointer, key}),
                }
            }
            RemoteOpInner::ReplaceText(ref op) => {
                let (child, pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let text_value = try_opt!(child.right());
                let text_op = try_opt!(text_value.execute_remote(op));
                Some(LocalOp::ReplaceText{pointer, changes: text_op.0})
            }
        }
    }

    fn get_nested_local(&mut self, pointer: &[usize]) -> Result<(Either<&mut Element, &mut TextValue>, Vec<SequenceUid>), Error> {
        let mut element = Some(&mut self.root);
        let mut remote_pointer = vec![];

        for (pointer_idx, child_idx) in pointer.iter().enumerate() {
            let (list_elt, _) = element.unwrap().children.0.get_mut_elt(*child_idx).map_err(|_| Error::InvalidPointer)?;
            let uid = list_elt.0.clone();
            remote_pointer.push(uid);
            match list_elt.1 {
                Child::Element(ref mut elt) =>
                    element = Some(elt),
                Child::Text(ref mut text) => {
                    if pointer_idx + 1 != pointer.len() { return Err(Error::InvalidPointer) };
                    return Ok((Either::Right(text), remote_pointer))
                }
            };
        }

        Ok((Either::Left(element.unwrap()), remote_pointer))
    }

    fn get_nested_remote(&mut self, pointer: &[SequenceUid]) -> Option<(Either<&mut Element, &mut TextValue>, Vec<usize>)> {
        let mut element = Some(&mut self.root);
        let mut local_pointer = vec![];

        for (pointer_idx, uid) in pointer.iter().enumerate() {
            let unwrapped_element = element.unwrap();
            let list_idx = try_opt!(unwrapped_element.children.0.get_idx(uid));
            let list_elt = try_opt!(unwrapped_element.children.0.lookup_mut(uid));
            local_pointer.push(list_idx);
            match list_elt.1 {
                Child::Element(ref mut elt) =>
                    element = Some(elt),
                Child::Text(ref mut text) => {
                    if pointer_idx + 1 != pointer.len() { return None }
                    return Some((Either::Right(text), local_pointer))
                }
            };
        }

        Some((Either::Left(element.unwrap()), local_pointer))
    }
}

impl CrdtValue for XmlValue {
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;
    type LocalValue = dom::Document;

    fn local_value(&self) -> Self::LocalValue {
        let declaration = self.declaration.clone();
        let root = self.root.dom();
        dom::Document::new(declaration.into(), root)
    }

    fn add_site(&mut self, op: &RemoteOp, site: u32) {
        self.nested_add_site(op, site);
    }

    fn add_site_to_all(&mut self, site: u32) {
        self.nested_add_site_to_all(site)
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        self.nested_validate_site(site)
    }

    fn merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.nested_merge(other, self_tombstones, other_tombstones).unwrap()
    }
}

impl NestedCrdtValue for XmlValue {
    fn nested_add_site(&mut self, op: &RemoteOp, site: u32) {
        match op.op {
            RemoteOpInner::Child(ref op_inner) => {
                let (child, _) = some!(self.get_nested_remote(&op.pointer));
                let element = some!(child.left());
                element.children.nested_add_site(op_inner, site);
            }
            RemoteOpInner::Attribute(ref op_inner) => {
                let (child, _) = some!(self.get_nested_remote(&op.pointer));
                let element = some!(child.left());
                element.attributes.add_site(op_inner, site);
            }
            RemoteOpInner::ReplaceText(ref op_inner) => {
                let (child, _) = some!(self.get_nested_remote(&op.pointer));
                let text = some!(child.right());
                text.add_site(op_inner, site);
            }
        };
    }

    fn nested_add_site_to_all(&mut self, site: u32) {
        self.root.attributes.add_site_to_all(site);
        self.root.children.nested_add_site_to_all(site);
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        self.root.attributes.validate_site(site)?;
        self.root.children.nested_validate_site(site)
    }

    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) -> Result<(), Error> {
        if self.declaration != other.declaration || self.root.name != other.root.name {
            return Err(Error::CannotMerge)
        }
        self.root.attributes.merge(other.root.attributes, self_tombstones, other_tombstones);
        self.root.children.nested_merge(other.root.children, self_tombstones, other_tombstones)
    }
}

impl CrdtValue for Child {
    type RemoteOp = <XmlValue as CrdtValue>::RemoteOp;
    type LocalOp = <XmlValue as CrdtValue>::LocalOp;
    type LocalValue = dom::Child;

    fn local_value(&self) -> Self::LocalValue {
        match *self {
            Child::Element(ref e) => dom::Child::Element(e.dom()),
            Child::Text(ref t) => dom::Child::Text(t.local_value()),
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

    fn merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.nested_merge(other, self_tombstones, other_tombstones).unwrap()
    }
}

impl NestedCrdtValue for Child {
    fn nested_add_site(&mut self, _: &RemoteOp, _: u32) {
        unimplemented!()
    }

    fn nested_add_site_to_all(&mut self, site: u32) {
        match *self {
            Child::Text(ref mut text) => text.add_site_to_all(site),
            Child::Element(ref mut element) => {
                element.attributes.add_site_to_all(site);
                element.children.nested_add_site_to_all(site);
            }
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        match *self {
            Child::Text(ref text) => text.validate_site(site),
            Child::Element(ref element) => {
                element.attributes.validate_site(site)?;
                element.children.nested_validate_site(site)
            }
        }
    }

    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) -> Result<(), Error> {
        match other {
            Child::Text(other) => {
                let text = self.as_text_mut().ok_or(Error::CannotMerge)?;
                text.merge(other, self_tombstones, other_tombstones);
                Ok(())
            }
            Child::Element(other) => {
                let element = self.as_element_mut().ok_or(Error::CannotMerge)?;
                element.attributes.merge(other.attributes, self_tombstones, other_tombstones);
                element.children.nested_merge(other.children, self_tombstones, other_tombstones)
            }
        }
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn deleted_replicas(&self) -> Vec<Replica> {
        match self.op {
            RemoteOpInner::Attribute(ref op) => op.deleted_replicas(),
            RemoteOpInner::Child(ref op) => op.deleted_replicas(),
            RemoteOpInner::ReplaceText(ref op) => op.deleted_replicas(),
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
        // updates sites in the pointer
        for uid in self.pointer.iter_mut() {
            if uid.site == 0 { uid.site = site; }
        };

        // update sites in the op
        match self.op {
            RemoteOpInner::Child(ref mut op) => op.nested_add_site(site),
            RemoteOpInner::Attribute(ref mut op) => op.add_site(site),
            RemoteOpInner::ReplaceText(ref mut op) => op.add_site(site),
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        match self.op {
            RemoteOpInner::Child(ref op) => op.nested_validate_site(site),
            RemoteOpInner::Attribute(ref op) => op.validate_site(site),
            RemoteOpInner::ReplaceText(ref op) => op.validate_site(site),
        }
    }
}

impl Child {
    fn as_element_mut(&mut self) -> Option<&mut Element> {
        if let Child::Element(ref mut element) = *self { Some(element) } else { None }
    }

    fn as_text_mut(&mut self) -> Option<&mut TextValue> {
        if let Child::Text(ref mut text) = *self { Some(text) } else { None }
    }
}

impl Element {
    fn from_dom(dom_element: dom::Element, replica: &Replica) -> Result<Self, Error> {
        let dom::Element{name, attributes: dom_attributes, children: dom_children} = dom_element;

        let mut attributes = MapValue::new();
        for (key, value) in dom_attributes {
            let _ = attributes.insert(key, value, replica);
        }

        let mut children = ListValue::new();
        for child in dom_children.into_iter() {
            match child {
                dom::Child::Element(dom_element) => {
                    let element = Element::from_dom(dom_element, replica)?;
                    children.push(Child::Element(element), replica)?;
                }
                dom::Child::Text(text) => {
                    let text_value = TextValue::from_str(&text, replica);
                    children.push(Child::Text(text_value), replica)?;
                }
            };
        }

        Ok(Element{name, attributes, children})
    }

    fn dom(&self) -> dom::Element {
        let attributes = self.attributes.local_value();
        let children = self.children.iter().map(|child|
            match child.1 {
                Child::Element(ref element) => dom::Child::Element(element.dom()),
                Child::Text(ref text) => dom::Child::Text(text.local_value()),
            }).collect::<Vec<_>>();
        dom::Element::new(self.name.clone(), attributes, children)
    }
}

fn into_xml(dom: dom::Document, replica: &Replica) -> Result<XmlValue, Error> {
    let dom::Document{declaration, root} = dom;
    let root = Element::from_dom(root, replica)?;
    Ok(XmlValue{declaration: declaration.into(), root})
}

pub trait IntoXmlNode {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error>;
}

impl<'a> IntoXmlNode for &'a str {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        let dom_child = dom::Child::from_str(self).map_err(|_| Error::InvalidXml)?;
        dom_child.into_xml_child(replica)
    }
}

impl IntoXmlNode for dom::Child {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        match self {
            dom::Child::Text(text) =>
                Ok(Child::Text(TextValue::from_str(&text, replica))),
            dom::Child::Element(dom_element) =>
                Ok(Child::Element(Element::from_dom(dom_element, replica)?)),
        }
    }
}

impl IntoXmlNode for dom::Element {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        Ok(Child::Element(Element::from_dom(self, replica)?))
    }
}

fn into_xml_attribute(key: &str, value: &str) -> Result<(String, String), Error> {
    dom::name::validate(key).map_err(|_| Error::InvalidXml)?;
    let dom_child = dom::Child::from_str(value).map_err(|_| Error::InvalidXml)?;
    let value = dom_child.into_text().ok_or(Error::InvalidXml)?;
    Ok((key.into(), value))
}

impl From<Declaration> for dom::Declaration {
    fn from(declaration: Declaration) -> Self {
        dom::Declaration{
            version:    declaration.version.into(),
            encoding:   declaration.encoding,
            standalone: declaration.standalone,
        }
    }
}

impl From<dom::Declaration> for Declaration {
    fn from(declaration: dom::Declaration) -> Self {
        Declaration{
            version:    declaration.version.into(),
            encoding:   declaration.encoding,
            standalone: declaration.standalone,
        }
    }
}

impl From<dom::XmlVersion> for XmlVersion {
    fn from(xml_version: dom::XmlVersion) -> Self {
        match xml_version {
            dom::XmlVersion::Version10 => XmlVersion::Version10,
            dom::XmlVersion::Version11 => XmlVersion::Version11,
        }
    }
}

impl From<XmlVersion> for dom::XmlVersion {
    fn from(xml_version: XmlVersion) -> Self {
        match xml_version {
            XmlVersion::Version10 => dom::XmlVersion::Version10,
            XmlVersion::Version11 => dom::XmlVersion::Version11,
        }
    }
}

fn split_pointer(pointer_str: &str) -> Result<Vec<usize>, Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    if pointer_str == "/" { return Ok(vec![]) }
    pointer_str.split("/").skip(1).map(|s| Ok(usize::from_str(s)?)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use rmp_serde;

    #[test]
    fn test_from_reader() {
        let string = r#"<?xml version="1.0" encoding="UTF-8"?><A>Something Texty</A>"#;
        let cursor = Cursor::new(string.as_bytes());
        let crdt = Xml::from_reader(cursor).unwrap();
        let local_value = crdt.local_value().to_string().unwrap();
        assert!(local_value == string);
    }

    #[test]
    fn test_from_str() {
        let string = r#"<?xml version="1.0" encoding="UTF-8"?><A>Something Texty</A>"#;
        let crdt = Xml::from_str(string).unwrap();
        let local_value = crdt.local_value().to_string().unwrap();
        assert!(local_value == string);
    }

    #[test]
    fn test_invalid_from_str() {
        let string1 = r#"<A>Something Texty</A>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
        let string3 = r#"<?xml version="1.0" encoding="UTF-8"?>Hello Everybody!"#;
        assert!(Xml::from_str(string1).is_err());
        assert!(Xml::from_str(string2).is_err());
        assert!(Xml::from_str(string3).is_err());
    }

    #[test]
    fn test_insert() {
        let string1 = r#"<?xml version="1.0" encoding="UTF-8"?><A></A>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?><A>Something Texty</A>"#;

        let mut crdt = Xml::from_str(string1).unwrap();
        let op = crdt.insert("/0", "Something Texty").unwrap();
        assert!(crdt.local_value().to_string().unwrap() == string2);
        assert_matches!(op.op, RemoteOpInner::Child(list::RemoteOp::Insert(_)));
    }

    #[test]
    fn test_remove() {
        let string1 = r#"<?xml version="1.0" encoding="UTF-8"?><ul><li>Thing 1</li><li>Thing 2</li>Random Text</ul>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?><ul><li>Thing 2</li></ul>"#;

        let mut crdt = Xml::from_str(string1).unwrap();
        let op1 = crdt.remove("/0").unwrap();
        let op2 = crdt.remove("/1").unwrap();

        assert!(crdt.local_value().to_string().unwrap() == string2);
        assert_matches!(op1.op, RemoteOpInner::Child(list::RemoteOp::Remove(_)));
        assert_matches!(op2.op, RemoteOpInner::Child(list::RemoteOp::Remove(_)));
    }

    #[test]
    fn test_insert_attribute() {
        let string1 = r#"<?xml version="1.0" encoding="UTF-8"?><A/>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?><A class="zebra"/>"#;

        let mut crdt = Xml::from_str(string1).unwrap();
        let op = crdt.insert_attribute("/", "class", "zebra").unwrap();
        println!("{}", crdt.local_value().to_string().unwrap());
        assert!(crdt.local_value().to_string().unwrap() == string2);
        assert_matches!(op.op, RemoteOpInner::Attribute(map::RemoteOp::Insert{..}));
    }

    #[test]
    fn test_remove_attribute() {
        let string1 = r#"<?xml version="1.0" encoding="UTF-8"?><A class="zebra">Hiya</A>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?><A>Hiya</A>"#;

        let mut crdt = Xml::from_str(string1).unwrap();
        let op = crdt.remove_attribute("/", "class").unwrap();
        println!("{}", crdt.local_value().to_string().unwrap());
        assert!(crdt.local_value().to_string().unwrap() == string2);
        assert_matches!(op.op, RemoteOpInner::Attribute(map::RemoteOp::Remove{..}));
    }

    #[test]
    fn test_insert_attribute_invalid() {
        let mut crdt = Xml::from_str(r#"<?xml version="1.0" encoding="UTF-8"?><A/>"#).unwrap();
        assert!(crdt.insert_attribute("/", "Hello There!", "zebra").is_err())
    }

    #[test]
    fn test_remove_attribute_invalid() {
        let mut crdt = Xml::from_str(r#"<?xml version="1.0" encoding="UTF-8"?><A/>"#).unwrap();
        assert!(crdt.remove_attribute("/", "zebra!").is_err())
    }

    #[test]
    fn test_replace_text() {
        let string1 = r#"<?xml version="1.0" encoding="UTF-8"?><ul><li>Thing 1</li></ul>"#;
        let string2 = r#"<?xml version="1.0" encoding="UTF-8"?><ul><li>Thing 9000</li></ul>"#;

        let mut crdt = Xml::from_str(string1).unwrap();
        let op = crdt.replace_text("/0/0", 6, 1, "9000").unwrap();
        assert!(crdt.local_value().to_string().unwrap() == string2);
        assert_matches!(op.op, RemoteOpInner::ReplaceText(text::RemoteOp{..}));
    }

    #[test]
    fn test_awaiting_site() {
        let string1 = r#"<?xml version="1.0"?><A>Hello</A>"#;
        let string2 = r#"<?xml version="1.0"?><A>Hello And Goodbye</A>"#;

        let remote_crdt = Xml::from_str(string1).unwrap();
        let mut crdt = Xml::from_state(remote_crdt.clone_state(), 0);
        assert!(crdt.replace_text("/0", 5, 0, " And Goodbye").unwrap_err() == Error::AwaitingSite);
        assert!(crdt.local_value().to_string().unwrap() == string2);

        let op = crdt.awaiting_site.pop().unwrap();
        assert_matches!(op.op, RemoteOpInner::ReplaceText(text::RemoteOp{..}));
    }

    #[test]
    fn test_execute_remote_child() {
        let string1 = r#"<?xml version="1.0"?><A>Hello</A>"#;
        let string2 = r#"<?xml version="1.0"?><A>Hello<b>GoodBye</b></A>"#;

        let mut crdt1 = Xml::from_str(string1).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 0);
        let remote_op = crdt1.insert("/1", "<b>GoodBye</b>").unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(crdt1.local_value().to_string().unwrap() == string2);
        if let LocalOp::Insert{pointer, ..} = local_op {
            assert_eq!(pointer, [1]);
        } else {
            panic!("Expected an Insert op");
        }
    }

    #[test]
    fn test_execute_remote_attribute() {
        let string1 = r#"<?xml version="1.0"?><A></A>"#;
        let string2 = r#"<?xml version="1.0"?><A name="foo"/>"#;

        let mut crdt1 = Xml::from_str(string1).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 0);
        let remote_op = crdt1.insert_attribute("/", "name", "foo").unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(crdt1.local_value().to_string().unwrap() == string2);
        if let LocalOp::InsertAttribute{pointer, key, value} = local_op {
            assert_eq!(pointer, Vec::<usize>::new());
            assert_eq!(key, "name");
            assert_eq!(value, "foo");
        } else {
            panic!("Expected an InsertAttribute op");
        }
    }

    #[test]
    fn test_execute_remote_replace_text() {
        let string1 = r#"<?xml version="1.0"?><A>Hiya!</A>"#;
        let string2 = r#"<?xml version="1.0"?><A>Hi There!</A>"#;

        let mut crdt1 = Xml::from_str(string1).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 0);
        let remote_op = crdt1.replace_text("/0", 2, 2, " There").unwrap();
        let local_op  = crdt2.execute_remote(&remote_op).unwrap();

        assert!(crdt1.value() == crdt2.value());
        assert!(crdt1.local_value().to_string().unwrap() == string2);
        if let LocalOp::ReplaceText{pointer, changes} = local_op {
            assert_eq!(pointer, [0]);
            assert_eq!(changes, [
                text::LocalChange{idx: 0, len: 5, text: "".into()},
                text::LocalChange{idx: 0, len: 0, text: "Hi".into()},
                text::LocalChange{idx: 2, len: 0, text: " There".into()},
                text::LocalChange{idx: 8, len: 0, text: "!".into()},
            ]);
        } else {
            panic!("Expected a ReplaceText op");
        }
    }

    #[test]
    fn test_execute_remote_missing_pointer() {
        let mut crdt1 = Xml::from_str(r#"<?xml version="1.0"?><A>Hiya!</A>"#).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 2);
        let remote_op = crdt1.remove("/0").unwrap();
        let _         = crdt2.remove("/0").unwrap();
        assert!(crdt2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_merge() {
        let string1 = r#"<?xml version="1.0"?><list><metadata/><items></items></list>"#;

        let mut crdt1 = Xml::from_str(string1).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 2);
        crdt1.insert_attribute("/0", "category", "letters").unwrap();
        crdt1.insert("/1/0", "<li>A</li>").unwrap();
        crdt1.insert("/1/1", "<li>B</li>").unwrap();

        crdt1.insert_attribute("/0", "category", "error codes").unwrap();
        crdt2.insert("/1/0", "<li>404</li>").unwrap();
        crdt2.insert("/1/1", "<li>503</li>").unwrap();

        let crdt1_state = crdt1.clone_state();
        crdt1.merge(crdt2.clone_state());
        crdt2.merge(crdt1_state);

        assert!(crdt1.value == crdt2.value);
        assert!(crdt1.tombstones == crdt2.tombstones);
    }

    #[test]
    fn test_add_site() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items></items></list>"#;
        let crdt1 = Xml::from_str(string).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 0);
        assert_matches!(crdt2.insert("/1/0", "<li>A</li>"), Err(Error::AwaitingSite));
        assert_matches!(crdt2.replace_text("/1/0/0", 0, 1, "B"), Err(Error::AwaitingSite));

        let ops = crdt2.add_site(2).unwrap();
        assert_matches!(ops[0].op, RemoteOpInner::Child(_));
        assert_matches!(ops[1].op, RemoteOpInner::ReplaceText(_));
    }

    #[test]
    fn test_add_site_already_has_site() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items></items></list>"#;
        let mut crdt = Xml::from_str(string).unwrap();
        assert!(crdt.add_site(33).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items><li>A</li><li>B</li></items></list>"#;
        let crdt1 = Xml::from_str(string).unwrap();

        let s_json = serde_json::to_string(&crdt1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&crdt1).unwrap();
        let crdt2: Xml = serde_json::from_str(&s_json).unwrap();
        let crdt3: Xml = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(crdt1 == crdt2);
        assert!(crdt1 == crdt3);
    }

    #[test]
    fn test_serialize_state() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items><li>A</li><li>B</li></items></list>"#;
        let crdt   = Xml::from_str(string).unwrap();
        let state1 = crdt.clone_state();

        let s_json = serde_json::to_string(&state1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&state1).unwrap();
        let state2: XmlState = serde_json::from_str(&s_json).unwrap();
        let state3: XmlState = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(state1 == state2);
        assert!(state1 == state3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items><li>A</li><li>B</li></items></list>"#;
        let mut crdt = Xml::from_str(string).unwrap();
        let remote_op1 = crdt.insert("/1/2", "<li>C</li>").unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let string = r#"<?xml version="1.0"?><list><metadata/><items><li>A</li><li>B</li></items></list>"#;
        let mut crdt1 = Xml::from_str(string).unwrap();
        let mut crdt2 = Xml::from_state(crdt1.clone_state(), 2);
        let remote_op = crdt1.insert("/1/2", "<li>C</li>").unwrap();
        let local_op1 = crdt2.execute_remote(&remote_op).unwrap();

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert_eq!(s_json, r#"{"op":"insert","pointer":[1,2],"value":"<li>C</li>"}"#);
        assert_eq!(local_op1, local_op2);
        assert_eq!(local_op1, local_op3);
    }

    #[test]
    fn test_split_pointer() {
        assert!(split_pointer("/").unwrap() == Vec::<usize>::new());
        assert!(split_pointer("/1/5/102").unwrap() == [1, 5, 102]);
        assert!(split_pointer("/1/5/blob").is_err());
        assert!(split_pointer("1/5/102").is_err());
        assert!(split_pointer("hello!!!").is_err());
    }
}
