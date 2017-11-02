extern crate quickxml_dom;

use quickxml_dom::Child;

#[test]
fn test_element_from_str() {
    let string = r#"<Hello>You&apos;re looking at an element</Hello>"#;
    let child  = Child::from_str(string).unwrap();
    if let Child::Element(element) = child {
        assert!(element.name() == "Hello");
        let ref grandchild = element.children()[0];
        if let Child::Text(ref text) = *grandchild {
            assert!(text == "You're looking at an element");
        } else {
            panic!("Must be decoded as text!");
        }
    } else {
        panic!("Must be decoded as an element!");
    }
}

#[test]
fn test_text_from_str() {
    let string = "&lt;Hello&gt;You&apos;re looking at text&lt;/Hello&gt;";
    let child  = Child::from_str(string).unwrap();
    if let Child::Text(text) = child {
        assert!(text == "<Hello>You're looking at text</Hello>");
    } else {
        panic!("Must be decoded as text!");
    }
}
