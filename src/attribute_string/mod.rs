mod attr;
mod element;

use self::element::Element;

pub struct AttributeString{
    elements: Vec<Element>,
    len: usize,
}

impl AttributeString {
    pub fn new() -> Self {
        AttributeString{
            elements: vec![Element::start_marker(), Element::end_marker()],
            len: 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::element::Element;

    #[test]
    fn test_new() {
        let string = AttributeString::new();
        assert!(string.len == 0);
        assert!(string.elements[0] == Element::start_marker());
        assert!(string.elements[1] == Element::end_marker());
    }
}
