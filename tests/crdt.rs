extern crate ditto;
extern crate regex;

use ditto::CRDT;

#[test]
fn create_load_dump_null() {
    let crdt = CRDT::create("null").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 5, 3).unwrap();
    assert!(crdt.site() == 1);
    assert!(crdt.counter() == 0);
    assert!(loaded.site() == 5);
    assert!(loaded.counter() == 3);
    assert!(dump == loaded.dump());
    assert!(dump == "null");
}

#[test]
fn create_load_dump_bool() {
    let crdt = CRDT::create("false").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert!(dump == "false");
}

#[test]
fn create_load_dump_number() {
    let crdt = CRDT::create("43").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert!(dump == "43.0");
}

#[test]
fn create_load_dump_string() {
    let crdt = CRDT::create(r#""The quick fox ran over the lazy dog.""#).unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert!(dump == r#""The quick fox ran over the lazy dog.""#);
}

#[test]
fn create_load_dump_empty_attrstr() {
    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":""}"#).unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert_match(dump, "[0,[]]");
}

#[test]
fn create_load_dump_nonempty_attrstr() {
    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":"The quick fox ran over the lazy dog."}"#).unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert_match(dump, r#"[0,[["XXX","The quick fox ran over the lazy dog."]]]"#);
}

#[test]
fn create_load_dump_empty_array() {
    let crdt = CRDT::create("[]").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(loaded.dump() == dump);
    assert_match(dump, "[1,[]]");
}

#[test]
fn create_load_dump_nonempty_array() {
    let crdt = CRDT::create("[1,2,3]").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(loaded.dump() == dump);
    assert_match(dump, r#"[1,[["XXX",1.0],["XXX",2.0],["XXX",3.0]]]"#);
}

#[test]
fn create_load_dump_empty_object() {
    let crdt = CRDT::create("{}").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert_match(dump, "[2,[]]");
}

#[test]
fn create_nonempty_object() {
    let crdt = CRDT::create(r#"{"hello":"goodbye"}"#).unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump, 2, 0).unwrap();
    assert!(dump == loaded.dump());
    assert_match(dump, r#"[2,[["XXX,hello","goodbye"]]]"#);
}

#[test]
fn create_load_invalid_json() {
    assert!(CRDT::create("{hello:goodbye}").is_err());
    assert!(CRDT::load("{hello:goodbye}", 1, 1).is_err());
}

fn assert_match(dump: String, template: &str) {
    let escaped_template = regex::quote(template).replace("XXX", r"[A-Za-z0-9+/]+");
    let re = regex::Regex::new(&escaped_template).unwrap();
    assert!(re.is_match(&dump))
}
