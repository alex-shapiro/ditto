use element::Element;
use element::Child;
use error::Error;
use quick_xml::events::{Event, BytesDecl};
use quick_xml::reader::Reader as XmlReader;
use quick_xml::writer::Writer as XmlWriter;
use std::io::{Read, BufRead, BufReader, Write};
use std::str::from_utf8;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct Document {
    pub declaration: Declaration,
    pub root: Element,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub version:    XmlVersion,
    pub encoding:   Option<String>,
    pub standalone: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XmlVersion {
    Version10,
    Version11,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node<'a> {
    Attribute(&'a str),
    Element(&'a Element),
    Text(&'a str),
}

impl Document {
    pub fn new(declaration: Declaration, root: Element) -> Self {
        Document{declaration, root}
    }

    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let buf_reader = BufReader::new(reader);
        let mut xml_reader = XmlReader::from_reader(buf_reader);
        let declaration = Declaration::from_reader(&mut xml_reader)?;
        let root = Element::from_reader(&mut xml_reader)?;
        Ok(Document{declaration, root})
    }

    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<usize, Error> {
        let mut xml_writer = XmlWriter::new(writer);
        self.declaration.to_writer(&mut xml_writer)?;
        Ok(self.root.to_writer(&mut xml_writer)?)
    }

    pub fn to_string(&self) -> Result<String, Error> {
        let mut bytes = Vec::new();
        let _ = self.to_writer(&mut bytes)?;
        Ok(String::from_utf8(bytes)?)
    }

    pub fn version(&self) -> XmlVersion {
        self.declaration.version
    }

    pub fn encoding(&self) -> Option<&str> {
        self.declaration.encoding.as_ref().and_then(|s| Some(s.as_str()))
    }

    pub fn pointer<'a>(&'a self, pointer: &str) -> Result<Node<'a>, Error> {
        if !pointer.starts_with("/") { return Err(Error::InvalidPointer) }
        let keys = pointer.split("/").skip(1);
        let mut node = Ok(Node::Element(&self.root));

        for key in keys {
            if let Node::Element(ref element) = node.unwrap() {
                if let Some(attr_value) = element.attributes().get(key) {
                    node = Ok(Node::Attribute(attr_value));
                } else {
                    let idx = usize::from_str(key).map_err(|_| Error::InvalidPointer)?;
                    match *element.children().get(idx).ok_or(Error::InvalidPointer)? {
                        Child::Element(ref element) =>
                            node = Ok(Node::Element(element)),
                        Child::Text(ref text) =>
                            node = Ok(Node::Text(text)),
                    };
                }
            } else {
                return Err(Error::InvalidPointer)
            }
        }

        node
    }
}

impl Declaration {
    pub fn from_reader<R: BufRead>(reader: &mut XmlReader<R>) -> Result<Self, Error> {
        let mut buf = vec![];
        while let Ok(event) = reader.read_event(&mut buf) {
            match event {
                Event::Decl(event) => {
                    let version = {
                        let version_bytes = event.version().map_err(|_| Error::InvalidXml)?;
                        bytes_to_version(version_bytes)?
                    };

                    let encoding = if let Some(result) = event.encoding() {
                        let encoding_bytes = result.map_err(|_| Error::InvalidXml)?;
                        Some(bytes_to_encoding(encoding_bytes)?)
                    } else {
                        None
                    };

                    let standalone = if let Some(result) = event.standalone() {
                        let standalone_bytes = result.map_err(|_| Error::InvalidXml)?;
                        Some(bytes_to_standalone(standalone_bytes)?)
                    } else {
                        None
                    };

                    return Ok(Declaration{version, encoding, standalone})
                }
                Event::Text(_) => (),
                _ => return Err(Error::InvalidXml),
            }
        }

        Err(Error::InvalidXml)
    }

    pub fn to_writer<W: Write>(&self, writer: &mut XmlWriter<W>) -> Result<usize, Error> {
        let version    = version_to_bytes(self.version);
        let encoding   = self.encoding.as_ref().and_then(|s| Some(s.as_bytes()));
        let standalone = self.standalone.and_then(|b| Some(standalone_to_bytes(b)));
        let element    = BytesDecl::new(version, encoding, standalone);
        Ok(writer.write_event(Event::Decl(element))?)
    }
}

fn bytes_to_version(bytes: &[u8]) -> Result<XmlVersion, Error> {
    match bytes {
        b"1.0" => Ok(XmlVersion::Version10),
        b"1.1" => Ok(XmlVersion::Version11),
        _ => Err(Error::InvalidXml),
    }
}

fn version_to_bytes(version: XmlVersion) -> &'static [u8] {
    match version {
        XmlVersion::Version10 => b"1.0",
        XmlVersion::Version11 => b"1.1",
    }
}

fn bytes_to_encoding(bytes: &[u8]) -> Result<String, Error> {
    Ok(from_utf8(bytes)?.to_owned())
}

fn bytes_to_standalone(bytes: &[u8]) -> Result<bool, Error> {
    match bytes {
        b"yes" => Ok(true),
        b"no" => Ok(false),
        _ => Err(Error::InvalidXml),
    }
}

fn standalone_to_bytes(standalone: bool) -> &'static [u8] {
    if standalone { b"yes" } else { b"no" }
}
