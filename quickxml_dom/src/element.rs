//! Provides an `Element` type to represent DOM nodes.

use error::Error;
use name::validate as validate_name;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{Event, BytesStart, BytesEnd, BytesText};
use quick_xml::reader::Reader as XmlReader;
use quick_xml::writer::Writer as XmlWriter;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, BufReader, Write, Cursor};
use std::str;

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Child {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn new(name: String, attributes: HashMap<String, String>, children: Vec<Child>) -> Self {
        Element{name, attributes, children}
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> &HashMap<String, String> {
        &self.attributes
    }

    pub fn children(&self) -> &[Child] {
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
                        parent.children.push(Child::Element(element));
                    } else {
                        return Ok(element)
                    }
                }
                Event::Empty(ref event) => {
                    let element = build_element(event)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Child::Element(element));
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
                    parent.children.push(Child::Text(text))
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

impl Child {
    pub fn to_writer<W: Write>(&self, writer: &mut XmlWriter<W>) -> Result<usize, Error> {
        match *self {
            Child::Element(ref element) => element.to_writer(writer),
            Child::Text(ref text) => {
                let element = BytesText::borrowed(text.as_bytes());
                Ok(writer.write_event(Event::Text(element))?)
            }
        }
    }

    pub fn from_str(string: &str) -> Result<Self, Error> {
        if string.starts_with("<") {
            let cursor = Cursor::new(string);
            let buf_reader = BufReader::new(cursor);
            let mut xml_reader = XmlReader::from_reader(buf_reader);
            let element = Element::from_reader(&mut xml_reader)?;
            Ok(Child::Element(element))
        } else {
            let escaped = BytesText::borrowed(string.as_bytes());
            let unescaped = escaped.unescaped()?.into_owned();
            let text = String::from_utf8(unescaped)?;
            Ok(Child::Text(text))
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        if let Child::Text(ref text) = *self { Some(text) } else { None }
    }

    pub fn as_element(&self) -> Option<&Element> {
        if let Child::Element(ref element) = *self { Some(element) } else { None }
    }

    pub fn as_text_mut(&mut self) -> Option<&mut str> {
        if let Child::Text(ref mut text) = *self { Some(text) } else { None }
    }

    pub fn as_element_mut(&mut self) -> Option<&mut Element> {
        if let Child::Element(ref mut element) = *self { Some(element) } else { None }
    }

    pub fn into_text(self) -> Option<String> {
        if let Child::Text(text) = self { Some(text) } else { None }
    }

    pub fn into_element(self) -> Option<Element> {
        if let Child::Element(element) = self { Some(element) } else { None }
    }
}

impl fmt::Display for Child {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut bytes = vec![];
        {
            let mut xml_writer = XmlWriter::new(&mut bytes);
            self.to_writer(&mut xml_writer).unwrap();
        }
        write!(f, "{}", String::from_utf8(bytes).unwrap())
    }
}

fn build_element(event: &BytesStart) -> Result<Element, Error> {
    let name = str::from_utf8(event.name())?.to_owned();
    validate_name(&name)?;

    let attributes = event.attributes().map(|attr_result| {
        let attr  = attr_result?;
        let key   = str::from_utf8(attr.key)?.to_owned();
        let value = str::from_utf8(attr.unescaped_value()?.borrow())?.to_owned();
        validate_name(&key)?;

        Ok((key, value))
    }).collect::<Result<HashMap<String, String>, Error>>()?;
    Ok(Element{name, attributes, children: vec![]})
}
