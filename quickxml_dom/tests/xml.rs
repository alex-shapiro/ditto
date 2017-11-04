#[macro_use]
extern crate assert_matches;
extern crate quickxml_dom;

use std::fs;
use std::io::Cursor;
use quickxml_dom::{Document, Node, Error};

#[test]
fn test_rdf() {
    test_file("test0.rdf");
}

#[test]
fn test_fdx() {
    test_file("test1.fdx");
}

#[test]
fn test_entities() {
    let document = test_file("test2.xml");

    assert!(document.pointer("/0/0").unwrap() == Node::Text("\""));
    assert!(document.pointer("/1/0").unwrap() == Node::Text("&"));
    assert!(document.pointer("/2").is_err());

    assert!(document.pointer("/0/first-name").unwrap() == Node::Attribute("<Bob>"));
    assert!(document.pointer("/1/last-name").unwrap() == Node::Attribute("O'Malley"));
    assert!(document.pointer("/1/other-attr").is_err());
}

#[test]
fn test_invalid_xml() {
    assert_matches!(try_load("test3.xml").unwrap_err(), Error::InvalidXml);
}

fn test_file(filename: &str) -> Document {
    let document1 = load(filename);
    let document2 = dump_and_reload(&document1);
    assert!(document1 == document2);
    document1
}

fn load(filename: &str) -> Document {
    try_load(filename).unwrap()
}

fn try_load(filename: &str) -> Result<Document, Error> {
    let relative_path = format!("./tests/files/{}", filename);
    let absolute_path = fs::canonicalize(&relative_path).unwrap();
    let mut file = fs::File::open(absolute_path).unwrap();
    Document::from_reader(&mut file)
}

fn dump_and_reload(document: &Document) -> Document {
    let mut buf: Vec<u8> = vec![];
    document.to_writer(&mut buf).unwrap();
    let mut cursor = Cursor::new(buf);
    Document::from_reader(&mut cursor).unwrap()
}
