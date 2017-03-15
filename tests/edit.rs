extern crate ditto;
extern crate regex;
extern crate serde_json;

use ditto::CRDT;

#[test]
fn edit_attrstr() {
    let mut crdt = CRDT::create(r#"{"__TYPE__":"attrstr", "text":""}"#).unwrap();
    let op1 = crdt.insert_text("", 0, "hello ").unwrap();
    let op2 = crdt.insert_text("", 6, "world!").unwrap();

    let dump = crdt.dump();
    let op1_json = serde_json::to_string(&op1).unwrap();
    let op2_json = serde_json::to_string(&op2).unwrap();

    assert_match(dump, r#"[0,[["XXX","hello "],["XXX","world!"]]]"#);
    assert_match(op1_json, r#"[8,"",[["XXX","hello "]],[]]"#);
    assert_match(op2_json, r#"[8,"",[["XXX","world!"]],[]]"#);
}

fn assert_match(dump: String, template: &str) {
    let escaped_template = regex::quote(template).replace("XXX", r"[A-Za-z0-9+/]+");
    let re = regex::Regex::new(&escaped_template).unwrap();
    assert!(re.is_match(&dump))
}
