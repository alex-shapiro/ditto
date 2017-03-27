extern crate ditto;
extern crate regex;

use ditto::{CRDT, Error};

#[test]
fn create_load_dump_null() {
    let crdt = CRDT::create("null").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump).unwrap();
    assert!(crdt.site() == 1);
    assert!(crdt.counter() == 1);
    assert!(loaded.site() == 1);
    assert!(loaded.counter() == 1);
    assert!(dump == r#"{"root_value":null,"replica":[1,1],"awaiting_site":[]}"#);
}

#[test]
fn create_load_dump_bool() {
    let crdt = CRDT::create("false").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump).unwrap();
    assert!(dump == loaded.dump());
    assert!(dump == r#"{"root_value":false,"replica":[1,1],"awaiting_site":[]}"#);
}

#[test]
fn create_load_dump_number() {
    let crdt = CRDT::create("43").unwrap();
    let dump = crdt.dump();
    let loaded = CRDT::load(&dump).unwrap();
    assert!(dump == loaded.dump());
}

#[test]
fn create_load_dump_string() {
    let crdt = CRDT::create(r#""The quick fox ran over the lazy dog.""#).unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(dump == loaded.dump_value());
    assert!(dump == r#""The quick fox ran over the lazy dog.""#);
}

#[test]
fn create_load_dump_empty_attrstr() {
    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":""}"#).unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(dump == loaded.dump_value());
    assert_match(dump, "[0,[]]");
}

#[test]
fn create_load_dump_nonempty_attrstr() {
    let crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":"The quick fox ran over the lazy dog."}"#).unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(dump == loaded.dump_value());
    assert_match(dump, r#"[0,[["XXX","The quick fox ran over the lazy dog."]]]"#);
}

#[test]
fn create_load_dump_empty_array() {
    let crdt = CRDT::create("[]").unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(loaded.dump_value() == dump);
    assert_match(dump, "[1,[]]");
}

#[test]
fn create_load_dump_nonempty_array() {
    let crdt = CRDT::create("[1,2,3]").unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(loaded.dump_value() == dump);
    assert_match(dump, r#"[1,[["XXX",1.0],["XXX",2.0],["XXX",3.0]]]"#);
}

#[test]
fn create_load_dump_empty_object() {
    let crdt = CRDT::create("{}").unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(dump == loaded.dump_value());
    assert_match(dump, "[2,[]]");
}

#[test]
fn create_nonempty_object() {
    let crdt = CRDT::create(r#"{"hello":"goodbye"}"#).unwrap();
    let dump = crdt.dump_value();
    let loaded = CRDT::load_value(&dump).unwrap();
    assert!(dump == loaded.dump_value());
    assert_match(dump, r#"[2,[["XXX,hello","goodbye"]]]"#);
}

#[test]
fn create_load_invalid_json() {
    assert!(CRDT::create("{hello:goodbye}").is_err());
    assert!(CRDT::load_value("{hello:goodbye}").is_err());
}

#[test]
fn update_site() {
    let mut crdt = CRDT::load_value("[2,[]]").unwrap();
    assert!(crdt.put("", "foo", "false") == Err(Error::AwaitingSite));
    assert!(crdt.put("", "bar", "true") == Err(Error::AwaitingSite));

    let ops = crdt.update_site(4).unwrap();
    assert!(crdt.site() == 4);
    assert!(ops[0].op.validate(4));
    assert!(ops[1].op.validate(4));

    assert!(crdt.update_site(5) == Err(Error::AlreadyHasSite));
}

fn assert_match(dump: String, template: &str) {
    let escaped_template = regex::quote(template).replace("XXX", r"[A-Za-z0-9+/]+");
    let re = regex::Regex::new(&escaped_template).unwrap();
    assert!(re.is_match(&dump))
}
