extern crate quickxml_dom;

use quickxml_dom::Child;

#[test]
fn test_element_from_str() {
    let string = r#"<Hello>You&apos;re looking at an element</Hello>"#;
    let child = Child::from_str(string).unwrap().into_element().unwrap();
    assert!(child.name() == "Hello");

    let grandchild = child.children()[0].as_text().unwrap();
    assert!(grandchild == "You're looking at an element");
}

#[test]
fn test_text_from_str() {
    let string = "&lt;Hello&gt;You&apos;re looking at text&lt;/Hello&gt;";
    let child = Child::from_str(string).unwrap().into_text().unwrap();
    assert!(child == "<Hello>You're looking at text</Hello>");
}

#[test]
fn test_to_string() {
    let string1 = r#"<Hello>You&apos;re looking at an element</Hello>"#;
    let child   = Child::from_str(string1).unwrap();
    let string2 = child.to_string();
    assert_eq!(string1, string2);
}

#[test]
fn test_odd_name() {
    let string = "<:Hiya>This Thing</:Hiya>";
    let child  = Child::from_str(string).unwrap().into_element().unwrap();
    assert!(child.name() == ":Hiya");
}
