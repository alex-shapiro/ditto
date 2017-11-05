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
pub struct LocalOp {
    pointer: Vec<usize>,
    op: LocalOpInner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum LocalOpInner {
    Attribute(map::LocalOp<String, String>),
    Child(list::LocalOp<Child>),
    ReplaceText(text::LocalOp),
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
        let attribute_value = value.into_xml_attribute_value()?;
        let op = element.attributes.insert(key.into(), attribute_value, replica)?;
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
                let (child, pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let element = try_opt!(child.left());
                let local_op = try_opt!(element.children.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::Child(local_op)})
            }
            RemoteOpInner::Attribute(ref op) => {
                let (child, pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let element = try_opt!(child.left());
                let local_op = try_opt!(element.attributes.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::Attribute(local_op)})
            }
            RemoteOpInner::ReplaceText(ref op) => {
                let (child, pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
                let text_value = try_opt!(child.right());
                let local_op = try_opt!(text_value.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::ReplaceText(local_op)})
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
        self.nested_merge(other, self_tombstones, other_tombstones)
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

    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.root.attributes.merge(other.root.attributes, self_tombstones, other_tombstones);
        self.root.children.merge(other.root.children, self_tombstones, other_tombstones);
    }
}

impl CrdtValue for Child {
    type RemoteOp = <XmlValue as CrdtValue>::RemoteOp;
    type LocalOp = <XmlValue as CrdtValue>::LocalOp;
    type LocalValue = <XmlValue as CrdtValue>::LocalValue;

    fn local_value(&self) -> Self::LocalValue { unreachable!() }

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
        self.nested_merge(other, self_tombstones, other_tombstones)
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

    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        match other {
            Child::Text(other) => {
                let text = some!(self.as_text_mut());
                text.merge(other, self_tombstones, other_tombstones)
            }
            Child::Element(other) => {
                let element = some!(self.as_element_mut());
                element.attributes.merge(other.attributes, self_tombstones, other_tombstones);
                element.children.nested_merge(other.children, self_tombstones, other_tombstones);
            }
        };
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

    fn into_xml_attribute_value(self) -> Result<String, Error>;
}

impl<'a> IntoXmlNode for &'a str {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        let dom_child = dom::Child::from_str(self).map_err(|_| Error::InvalidXml)?;
        dom_child.into_xml_child(replica)
    }

    fn into_xml_attribute_value(self) -> Result<String, Error> {
        let dom_child = dom::Child::from_str(self).map_err(|_| Error::InvalidXml)?;
        let text = dom_child.into_text().ok_or(Error::InvalidXml)?;
        Ok(text)
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

    fn into_xml_attribute_value(self) -> Result<String, Error> {
        Err(Error::InvalidXml)
    }
}

impl IntoXmlNode for dom::Element {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        Ok(Child::Element(Element::from_dom(self, replica)?))
    }

    fn into_xml_attribute_value(self) -> Result<String, Error> {
        Err(Error::InvalidXml)
    }
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

pub fn split_pointer(pointer_str: &str) -> Result<Vec<usize>, Error> {
    if !pointer_str.starts_with("/") { return Err(Error::InvalidPointer) }
    pointer_str.split("/").skip(1).map(|s| Ok(usize::from_str(s)?)).collect()
}

