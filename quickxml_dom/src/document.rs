use element::Element;
use error::Error;
use quick_xml::events::{Event, BytesDecl};
use quick_xml::reader::Reader as XmlReader;
use quick_xml::writer::Writer as XmlWriter;
use std::io::{Read, BufRead, BufReader, Write};
use std::str::from_utf8;

#[derive(Debug, PartialEq)]
pub struct Document {
    declaration: Declaration,
    root: Element,
}

#[derive(Debug, PartialEq)]
pub struct Declaration {
    version:    XmlVersion,
    encoding:   Option<String>,
    standalone: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XmlVersion {
    Version10,
    Version11,
}

impl Document {
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

    pub fn version(&self) -> XmlVersion {
        self.declaration.version
    }

    pub fn encoding(&self) -> Option<&str> {
        self.declaration.encoding.as_ref().and_then(|s| Some(s.as_str()))
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
