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
    declaration: dom::Declaration,
    root: Element,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Element {
    name:       String,
    attributes: MapValue<String, String>,
    children:   ListValue<Child>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Child {
    Text(TextValue),
    Element(Element),
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
    pointer: String,
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
        let local_xml = dom::Document::from_reader(reader)?;
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
        self.after_op(op);
    }

    pub fn remove(&mut self, pointer_str: &str) -> Result<RemoteOp, Error> {
        let op = self.value.remove(pointer_str)?;
        self.after_op(op)
    }

    // pub fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str) -> Result<RemoteOp, Error> {
    //     let op = self.value.replace_text(pointer_str, idx, len, text, &self.replica)?;
    //     self.after_op(op)
    // }
}

impl XmlValue {
    pub fn insert<T: IntoXmlNode>(&mut self, pointer_str: &str, node: T, replica: &Replica) -> Result<RemoteOp, Error> {
        let (pointer, key) = pointer::split_xml_nodes(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let nested_element = child.as_element()?;

        if let Ok(idx) = usize::from_str(key) {
            let op = nested_element.children.insert(idx, node.into_xml_child(), replica)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
        } else {
            let op = nested_element.attributes.insert(key, node.into_xml_attribute_value(), replica)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
        }
    }

    pub fn remove(&mut self, pointer_str: &str) -> Result<RemoteOp, Error> {
        let (pointer, key) = pointer::split_xml_nodes(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let nested_element = child.as_element()?;

        if let Ok(idx) = usize::from_str(key) {
            let op = nested_element.children.remove(idx)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Child(op)})
        } else {
            let op = nested_element.attributes.remove(key)?;
            Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::Attribute(op)})
        }
    }

    pub fn replace_text(&mut self, pointer_str: &str, idx: usize, len: usize, text: &str, replica: &str) -> Result<RemoteOp, Error> {
        let pointer = pointer::split_xml_children(pointer_str)?;
        let (child, remote_pointer) = self.get_nested_local(&pointer)?;
        let nested_text = child.as_text()?;
        let op = nested_text.replace(idx, len, text, replica)?;
        Ok(RemoteOp{pointer: remote_pointer, op: RemoteOpInner::ReplaceText(op)})
    }

    // pub fn execute_remote(&mut self, remote_op: &RemoteOp) -> Option<LocalOp> {
    //     let (child, local_pointer) = try_opt!(self.get_nested_remote(&remote_op.pointer));
    //     match (child, &remote_op.op) {
    //         (&mut Child::Element(ref mut element), &RemoteOpInner::Child(ref op)) => {
    //             let local_op = try_opt!(element.children.execute_remote(op));
    //             Some(LocalOp{pointer: local_pointer, op: LocalOpInner::Child(local_op)})
    //         }
    //         (&mut Child::Element(ref mut element), &RemoteOpInner::Attribute(ref op)) => {
    //             let local_op = try_opt!(element.attributes.execute_remote(op));
    //             Some(LocalOp{pointer: local_pointer, op: LocalOpInner::Attribute(local_op)})
    //         }
    //         (&mut Child::Text(ref mut text), &RemoteOpInner::ReplaceText(ref op)) => {
    //             let local_op = try_opt!(text.execute_remote(op));
    //             Some(LocalOp{pointer: local_pointer, op: LocalOpInner::ReplaceText(local_op)})
    //         }
    //     }
    // }

    pub fn merge(&mut self, other: XmlValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.nested_merge(other, self_tombstones, other_tombstones)
    }

    fn get_nested_local(&mut self, pointer: &[usize]) -> Result<(&mut Child, Vec<SequenceUid>), Error> {
        let mut child = Some(&mut self.root);
        let mut remote_pointer = vec![];

        for idx in pointer.enumerate() {
            match child.unwrap() {
                &mut Child::Text(_) => return Err(Error::InvalidPointer),
                &mut Child::Element(ref mut element) => {
                    let list_elt = element.children.0.get_mut_elt(idx).ok_or(Error::InvalidPointer)?;
                    let uid = list_elt.0.clone();
                    remote_pointer.push(uid);
                    child = Some(&mut list_elt.1);
                }
            }
        }

        Ok((child.unwrap(), remote_pointer))
    }
}

impl CrdtValue for XmlValue {
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;
    type LocalValue = dom::Document;

    fn local_value(&self) -> Self::LocalValue {
        let declaration = self.declaration.clone();
        let root = self.root.local_value();
        dom::Document::new(declaration, root)
    }

    fn add_site(&mut self, op: &RemoteOp, site: u32) {
        unimplemented!()
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn deleted_replicas(&self) -> Vec<Replica> {
        match self.op {
            RemoteOpInner::Attributes(ref op) => op.deleted_replicas(),
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


impl Element {
    fn from_dom(dom_element: dom::Element, replica: &Replica) -> Result<Self, Error> {
        let mut attributes = MapValue::new();
        for (key, value) in dom_element.attributes {
            attributes.insert(key, value, replica);
        }

        let mut children = ListValue::new();
        for (idx, child) in dom_element.children.into_iter().enumerate() {
            match child {
                dom::Child::Element(dom_element) => {
                    let element = dom_element.into_xml_elt(replica)?;
                    children.push(Child::Element(element));
                }
                dom::Child::Text(text) => {
                    let mut text_value = TextValue::new();
                    text_value.replace(0, 0, text, replica)?;
                    children.push(Child::Text(text_value));
                }
            }
        }

        Ok(Element{name: dom_element.name, attributes, children})
    }

    fn into_dom(&self) -> dom::Element {
        let attributes = self.attributes.local_value();
        let children = self.children.iter().map(|child|
            match *child {
                Child::Element(ref element) => dom::Child::Element(element.local_value()),
                Child::Text(ref text) => dom::Child::Text(text),
            }).collect::<Vec<_>>();
        dom::Element::new(self.name.clone(), attributes, children)
    }
}

fn into_xml(dom: dom::Document, replica: &Replica) -> Result<XmlValue, Error> {
    XmlValue{declaration: dom.declaration, root: dom.root.into_xml_element(replica)}
}

trait IntoXmlNode {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error>;

    fn into_xml_attribute_value(self, replica: &Replica) -> Result<String, Error> {
        Err(Error::InvalidXml)
    }
}

impl<'a> IntoXmlNode for &'a str {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        let dom_child = dom::Child::from_str(self)?;
        dom_child.into_xml_child(replica)
    }

    fn into_xml_attribute_value(self, replica: &Replica) -> Result<String, Error> {
        let dom_child = dom::Child::from_str(self)?;
        let text = dom_child.into_text().ok_or(Error::InvalidXml)?;
        Ok(text)
    }
}

impl IntoXmlNode for dom::Child {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        match self {
            dom::Child::Text(text) => Ok(Child::Text(text)),
            dom::Child::Element(dom_element) => dom_element.into_xml_child()
        }
    }
}

impl IntoXmlNode for dom::Element {
    fn into_xml_child(self, replica: &Replica) -> Result<Child, Error> {
        Ok(Child::Element(Element::from_dom(self)?))
    }
}
