//! An `Xml` CRDT stores an XML document.

use {Error, Replica, Tombstones};
use sequence::uid::UID as SequenceUid;
use list::{self, ListValue};
use map::{self, MapValue};
use pointer;
use text::{self, TextValue};
use traits::*;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Declaration {
    version:    XmlVersion,
    encoding:   Option<String>,
    standalone: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    name:       String,
    attributes: MapValue<String, String>,
    children:   ListValue<Child>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Child {
    Text(TextValue),
    Element(Element),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum XmlVersion {
    Version10,
    Version11,
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

    pub fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str) -> Result<RemoteOp, Error> {
        let op = self.value.replace_text(pointer_str, idx, len, text, &self.replica)?;
        self.after_op(op)
    }
}

impl XmlValue {
    pub fn insert<T: IntoXmlNode>(&mut self, pointer_str: &str, node: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let (pointer, key) = pointer::split_xml_nodes(pointer_str)?;
        let (nested_element, remote_pointer) = self.get_nested_element_local(&pointer)?;

        if let Ok(idx) = usize::from_str(key) {
            let node = node.into_xml_child(replica)?;
            let op = nested_element.children.insert(idx, node, replica)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
        } else {
            let attribute_value = node.into_xml_attribute_value()?;
            let op = nested_element.attributes.insert(key.into(), attribute_value, replica)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
        }
    }

    pub fn remove(&mut self, pointer_str: &str) -> Result<RemoteOp, Error> {
        let (pointer, key) = pointer::split_xml_nodes(pointer_str)?;
        let (nested_element, remote_pointer) = self.get_nested_element_local(&pointer)?;

        if let Ok(idx) = usize::from_str(key) {
            let op = nested_element.children.remove(idx)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
        } else {
            let op = nested_element.attributes.remove(key)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
        }
    }

    pub fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        let pointer = pointer::split_xml_children(pointer_str)?;
        let (nested_text, remote_pointer) = self.get_nested_text_local(&pointer)?;
        let op = nested_text.replace(idx, len, text, replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::ReplaceText(op)})
    }

    pub fn execute_remote(&mut self, remote_op: &RemoteOp) -> Option<LocalOp> {
        match remote_op.op {
            RemoteOpInner::Child(ref op) => {
                let (element, pointer) = try_opt!(self.get_nested_element_remote(&remote_op.pointer));
                let local_op = try_opt!(element.children.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::Child(local_op)})
            }
            RemoteOpInner::Attribute(ref op) => {
                let (element, pointer) = try_opt!(self.get_nested_element_remote(&remote_op.pointer));
                let local_op = try_opt!(element.attributes.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::Attribute(local_op)})
            }
            RemoteOpInner::ReplaceText(ref op) => {
                let (text, pointer) = try_opt!(self.get_nested_text_remote(&remote_op.pointer));
                let local_op = try_opt!(text.execute_remote(op));
                Some(LocalOp{pointer, op: LocalOpInner::ReplaceText(local_op)})
            }
        }
    }

    pub fn merge(&mut self, other: XmlValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.root.nested_merge(other.root, self_tombstones, other_tombstones)
    }

    fn get_nested_element_local(&mut self, pointer: &[usize]) -> Result<(&mut Element, Vec<SequenceUid>), Error> {
        let mut element = Some(&mut self.root);
        let mut remote_pointer = vec![];

        for idx in pointer {
            let (list_elt, _) = element.unwrap().children.0.get_mut_elt(*idx).map_err(|_| Error::InvalidPointer)?;
            let uid = list_elt.0.clone();
            element = Some(list_elt.1.as_element_mut().ok_or(Error::InvalidPointer)?);
            remote_pointer.push(uid);
        }

        Ok((element.unwrap(), remote_pointer))
    }

    fn get_nested_text_local(&mut self, pointer: &[usize]) -> Result<(&mut TextValue, Vec<SequenceUid>), Error> {
        let idx = *pointer.last().ok_or(Error::InvalidPointer)?;
        let pointer = &pointer[0 .. pointer.len() - 1];

        let (element, mut remote_pointer) = self.get_nested_element_local(&pointer)?;
        let (list_elt, _) = element.children.0.get_mut_elt(idx).map_err(|_| Error::InvalidPointer)?;
        let uid = list_elt.0.clone();
        let text = list_elt.1.as_text_mut().ok_or(Error::InvalidPointer)?;

        remote_pointer.push(uid);
        Ok((text, remote_pointer))
    }

    fn get_nested_element_remote(&mut self, pointer: &[SequenceUid]) -> Option<(&mut Element, Vec<usize>)> {
        let mut element = Some(&mut self.root);
        let mut local_pointer = vec![];

        for uid in pointer {
            let unwrapped_element = element.unwrap();
            let list_idx = try_opt!(unwrapped_element.children.0.get_idx(uid));
            let list_elt = try_opt!(unwrapped_element.children.0.lookup_mut(uid));
            element = Some(try_opt!(list_elt.1.as_element_mut()));
            local_pointer.push(list_idx);
        }

        Some((element.unwrap(), local_pointer))
    }

    fn get_nested_text_remote(&mut self, pointer: &[SequenceUid]) -> Option<(&mut TextValue, Vec<usize>)> {
        let uid = try_opt!(pointer.last());
        let pointer = &pointer[0 .. pointer.len() - 1];

        let (element, mut local_pointer) = try_opt!(self.get_nested_element_remote(&pointer));
        let idx = try_opt!(element.children.0.get_idx(&uid));
        let list_elt = try_opt!(element.children.0.lookup_mut(&uid));
        let text = try_opt!(list_elt.1.as_text_mut());

        local_pointer.push(idx);
        Some((text, local_pointer))
    }
}

impl NestedValue for Element {
    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.attributes.merge(other.attributes, self_tombstones, other_tombstones);
        self.children.nested_merge(other.children, self_tombstones, other_tombstones);
    }
}

impl NestedValue for Child {
    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        match other {
            Child::Text(other_text) =>
                some!(self.as_text_mut()).merge(other_text, self_tombstones, other_tombstones),
            Child::Element(other_element) =>
                some!(self.as_element_mut()).nested_merge(other_element, self_tombstones, other_tombstones),
        }
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
        unimplemented!()
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
        unimplemented!()
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        unimplemented!()
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
                    let mut text_value = TextValue::new();
                    text_value.replace(0, 0, &text, replica)?;
                    children.push(Child::Text(text_value), replica)?;
                }
            }
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
