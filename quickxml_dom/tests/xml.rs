extern crate quickxml_dom;

use std::fs;
use std::io::Cursor;
use quickxml_dom::Document;

#[test]
fn test_rdf() {
    test_file("./tests/files/test0.rdf");
}

#[test]
fn test_fdx() {
    test_file("./tests/files/test1.fdx");
}

fn test_file(relative_path: &str) {
    let path = fs::canonicalize(relative_path).unwrap();
    let mut file = fs::File::open(path).unwrap();
    let mut buf: Vec<u8> = vec![];

    let document = Document::from_reader(&mut file).unwrap();
    document.to_writer(&mut buf).unwrap();

    let mut cursor = Cursor::new(buf);
    let document2 = Document::from_reader(&mut cursor).unwrap();

    assert!(document == document2);
}
