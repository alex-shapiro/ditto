mod attr;
mod element;
mod range;

use self::element::Element;
use self::range::Range;

#[derive(Clone,PartialEq)]
pub struct AttributedString{
    elements: Vec<Element>,
    len: usize,
}

impl AttributedString {
    pub fn new() -> Self {
        AttributedString{
            elements: vec![Element::start_marker(), Element::end_marker()],
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn insert_text(&mut self, index: usize, text: String, site: Site, counter: Counter) {
        let range = EltRange::excluding_boundary_attrs(&self.elements, start, 0);
        match range.start {
            EltRangeValue::Boundary{elt_index: elt_index} -> {
                let ref path1 = self.elements[elt_index-1].path();
                let ref path2 = self.elements[elt_index].path();
                let path = Path::between(path1, path2, site);
                let element = Element::text(text.clone(), path.clone(), counter);
                self.elements.insert(elt_index, element);
                UpdateAttributedString::new(vec![element], vec![])
            },
            EltRangeValue::Middle{elt_index: elt_index, offset: offset} -> {
                let removed_elt = self.elements.remove(elt_index);
                let (elt_pre, elt_post) = removed_elt.split();
                let path = Path::between(&elt_pre.uid.path(), &elt_post.uid.path());
                let element = Element::text(text.clone(), path.clone(), counter);
                self.elements.insert(elt_index, elt_pre.clone());
                self.elements.insert(elt_index+1, element.clone());
                self.elements.insert(elt_index+2, elt_post.clone());
                UpdateAttributedString::new(vec![elt_pre, element, elt_post], vec![removed_elt])
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::element::Element;

    #[test]
    fn test_new() {
        let string = AttributedString::new();
        assert!(string.len() == 0);
        assert!(string.elements[0] == Element::start_marker());
        assert!(string.elements[1] == Element::end_marker());
    }
}
