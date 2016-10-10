use super::element::Element;

#[derive(PartialEq)]
pub enum Index {
    Whole{index: usize},
    Part{index: usize, offset: usize}
}

pub struct Range {
    start: Index,
    end: Index,
}

impl Range {
    pub fn new(vec_len: usize) -> Self {
        let start = Index::Whole{index: 1};
        let end = Index::Whole{index: vec_len - 2};
        Range{start: start, end: end}
    }

    pub fn excluding_boundary_attrs(elements: &[Element], start: usize, len: usize) -> Self {
        let mut range = Range::new(elements.len());
        let mut started = false;
        let mut char_index: usize = 0;
        let mut elt_index:  usize = 0;
        let mut elements = elements.iter();
        let stop = start + len;

        while let Some(elt) = elements.next() {
            let elt_len = elt.len();

            if !started {
                if char_index == start && elt.text().is_some() {
                    range.start = Index::Whole{index: elt_index};
                    started = true;
                } else if char_index + elt_len > start {
                    range.start = Index::Part{index: elt_index, offset: start-char_index};
                    started = true;
                }
            } else {
                if char_index == stop {
                    range.end = Index::Whole{index: elt_index - 1};
                    break;
                } else if char_index + elt_len > stop {
                    range.end = Index::Part{index: elt_index, offset: stop-char_index};
                    break;
                }
            }

            char_index += elt_len;
            elt_index += 1;
        }
        range
    }

    pub fn including_boundary_attrs(elements: &[Element], start: usize, len: usize) -> Self {
        let mut range = Range::new(elements.len());
        let mut started = false;
        let mut char_index: usize = 0;
        let mut elt_index: usize = 0;
        let mut elements = elements.iter();
        let stop = start + len;

        while let Some(elt) = elements.next() {
            let elt_len = elt.len();

            if !started {
                if char_index == start && (elt.attr_open().is_some() || elt.text().is_some()) {
                    range.start = Index::Whole{index: elt_index};
                    started = true;
                } else if char_index < start && char_index + elt_len > start {
                    range.start = Index::Part{index: elt_index, offset: start-char_index};
                    started = true;
                }
            } else {
                if char_index == stop && (elt.attr_close().is_none()) {
                    range.end = Index::Whole{index: elt_index - 1};
                    break;
                } else if char_index < stop && char_index + elt_len > stop {
                    range.end = Index::Part{index: elt_index, offset: stop-char_index};
                    break;
                }
            }
            char_index += elt_len;
            elt_index += 1;
        }
        range
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::element::*;
    use super::super::attr::*;
    use sequence::path;

    #[test]
    fn test_new() {
        let range = Range::new(43);
        assert!(range.start == Index::Whole{index: 1});
        assert!(range.end == Index::Whole{index: 41});
    }

    #[test]
    fn test_excluding_boundary_attrs() {
        let elements = build_elements(vec![
            text("hello "),
            attropen("A",""),
            attropen("B",""),
            text("and"),
            attrclose("B"),
            text(" goodbye"),
            attrclose("A"),
        ]);

        let range0 = Range::excluding_boundary_attrs(&elements, 0, 17);
        assert!(range0.start == Index::Whole{index: 1});
        assert!(range0.end == Index::Whole{index: 6});

        let range1 = Range::excluding_boundary_attrs(&elements, 6, 11);
        assert!(range1.start == Index::Whole{index: 4});
        assert!(range1.end == Index::Whole{index: 6});

        let range2 = Range::excluding_boundary_attrs(&elements, 4, 8);
        assert!(range2.start == Index::Part{index: 1, offset: 4});
        assert!(range2.end == Index::Part{index: 6, offset: 3});
    }

    #[test]
    fn test_including_boundary_attrs() {
        let elements = build_elements(vec![
            text("hello "),
            attropen("A",""),
            attropen("B",""),
            text("and"),
            attrclose("B"),
            text(" goodbye"),
            attrclose("A"),
        ]);

        let range0 = Range::including_boundary_attrs(&elements, 0, 17);
        assert!(range0.start == Index::Whole{index: 1});
        assert!(range0.end == Index::Whole{index: 7});

        let range1 = Range::including_boundary_attrs(&elements, 6, 3);
        assert!(range1.start == Index::Whole{index: 2});
        assert!(range1.end == Index::Whole{index: 5});

        let range2 = Range::including_boundary_attrs(&elements, 4, 12);
        assert!(range2.start == Index::Part{index: 1, offset: 4});
        assert!(range2.end == Index::Part{index: 6, offset: 7});
    }

    fn text(value: &str) -> EltValue {
        EltValue::Text(value.to_string())
    }

    fn attropen(key: &str, value: &str) -> EltValue {
        EltValue::AttrOpen(AttrOpen::new(key.to_string(), value.to_string()))
    }

    fn attrclose(key: &str) -> EltValue {
        EltValue::AttrClose(AttrClose::new(key.to_string()))
    }

    fn build_elements(elt_values: Vec<EltValue>) -> Vec<Element> {
        let mut elements: Vec<Element> = vec![Element::start_marker()];
        let end_marker = Element::end_marker();

        for value in elt_values {
            let path = path::between(elements.last().unwrap().path(), end_marker.path(), 1);
            let element = Element::new(value, path, 1);
            elements.push(element);
        }
        elements.push(end_marker);
        elements
    }
}
