//! Provides an `Element` type to represent DOM nodes.

use error::Error;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{Event, BytesStart, BytesEnd, BytesText};
use quick_xml::reader::Reader as XmlReader;
use quick_xml::writer::Writer as XmlWriter;
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::str;

#[derive(Debug, PartialEq)]
pub struct Element {
    name: String,
    attributes: BTreeMap<String, String>,
    children: Vec<Node>,
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> &BTreeMap<String, String> {
        &self.attributes
    }

    pub fn children(&self) -> &[Node] {
        &self.children
    }

    pub fn from_reader<R: BufRead>(reader: &mut XmlReader<R>) -> Result<Element, Error> {
        let mut buf   = vec![];
        let mut stack = vec![];

        while let Ok(event) = reader.read_event(&mut buf) {
            match event {
                Event::Start(ref event) => {
                    let element = build_element(event)?;
                    stack.push(element);
                }
                Event::End(ref event) => {
                    let element = stack.pop().ok_or(Error::InvalidXml)?;
                    let endname = str::from_utf8(event.name())?;

                    if endname != element.name {
                        return Err(Error::InvalidXml)
                    }

                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::Element(element));
                    } else {
                        return Ok(element)
                    }
                }
                Event::Empty(ref event) => {
                    let element = build_element(event)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::Element(element));
                    } else {
                        return Ok(element)
                    }
                }
                Event::Text(ref event) => {
                    let unescaped = event.unescaped()?;
                    let decoded = reader.decode(&*unescaped);
                    let textstr: &str = decoded.borrow();
                    let text = textstr.trim().to_owned();
                    if text.is_empty() { continue }

                    let parent = stack.last_mut().ok_or(Error::InvalidXml)?;
                    parent.children.push(Node::Text(text))
                }
                Event::Eof => return Err(Error::InvalidXml),
                Event::Decl(_) => return Err(Error::InvalidXml),
                Event::Comment(_) => (),
                Event::CData(_) => (),
                Event::PI(_) => (),
                Event::DocType(_) => (),
            }
        }

        Err(Error::InvalidXml)
    }

    pub fn to_writer<W: Write>(&self, writer: &mut XmlWriter<W>) -> Result<usize, Error> {
        let name = self.name.as_bytes();
        let mut element = BytesStart::borrowed(name, name.len());
        for (k, v) in &self.attributes {
            let attribute = Attribute{key: k.as_bytes(), value: v.as_bytes()};
            element.push_attribute(attribute);
        }

        if self.children.is_empty() {
            Ok(writer.write_event(Event::Empty(element))?)
        } else {
            writer.write_event(Event::Start(element))?;
            for child in &self.children {
                child.to_writer(writer)?;
            }
            let element = BytesEnd::borrowed(name);
            Ok(writer.write_event(Event::End(element))?)
        }
    }
}

impl Node {
    pub fn to_writer<W: Write>(&self, writer: &mut XmlWriter<W>) -> Result<usize, Error> {
        match *self {
            Node::Element(ref element) => element.to_writer(writer),
            Node::Text(ref text) => {
                let element = BytesText::borrowed(text.as_bytes());
                Ok(writer.write_event(Event::Text(element))?)
            }
        }
    }
}

fn build_element(event: &BytesStart) -> Result<Element, Error> {
    let name = str::from_utf8(event.name())?.to_owned();
    let attributes = event.attributes().map(|attr_result| {
        let attr  = attr_result?;
        let key   = str::from_utf8(attr.key)?.to_owned();
        let value = str::from_utf8(attr.unescaped_value()?.borrow())?.to_owned();
        Ok((key, value))
    }).collect::<Result<BTreeMap<String, String>, Error>>()?;
    Ok(Element{name, attributes, children: vec![]})
}
