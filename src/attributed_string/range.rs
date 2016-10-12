use super::element::Element;

#[derive(Clone,PartialEq,PartialOrd)]
pub enum Index {
    Whole{index: usize},
    Part{index: usize, offset: usize}
}

impl Index {
    fn new(index: usize, offset: usize) -> Self {
        match offset {
            0 => Index::Whole{index: index},
            _ => Index::Part{index: index, offset: offset},
        }
    }
}


pub struct Range {
    pub start: Index,
    pub stop: Index,
}

impl Range {
    /// Create a new Range that excludes all attributes bounding the
    /// in-range text. Used in operations that don't affect
    /// bounding attributes (ie text insert/replace).
    pub fn excluding_boundary_attrs(elements: &[Element], start: usize, len: usize) -> Self {
        let stop = start + len;
        let mut in_range = false;
        let mut char_index:   usize = 0;
        let mut elt_index:    usize = 0;

        let mut start_index:  usize = 0;
        let mut start_offset: usize = 0;
        let mut stop_index:   usize = 0;
        let mut stop_offset:  usize = 0;

        for elt in elements {
            let elt_len = elt.len();
            if !in_range {
                if char_index + elt_len > start {
                    start_index = elt_index;
                    start_offset = start - char_index;
                    in_range = true;
                } else if elt.text().is_some() {
                    start_index = elt_index;
                }
            } else {
                if char_index + elt_len > stop {
                    stop_index = elt_index;
                    stop_offset = stop - char_index;
                    break;
                } else if char_index + elt_len == stop {
                    stop_index = elt_index;
                    break;
                } else if elt.text().is_some() {
                    stop_index = elt_index;
                }
            }
            char_index += elt_len;
            elt_index += 1;
        }
        Range::new(start_index, start_offset, stop_index, stop_offset)
    }

    /// Create a new Range that includes all attributes bounding the
    /// in-range text. Used in operations that affect bounding
    /// attributes (ie text delete, attribute put/delete)
    pub fn including_boundary_attrs(elements: &[Element], start: usize, len: usize) -> Self {
        let stop = start + len;
        let mut in_range = false;
        let mut char_index:   usize = 0;
        let mut elt_index:    usize = 0;

        let mut start_index:  usize = 0;
        let mut start_offset: usize = 0;
        let mut stop_index:   usize = 0;
        let mut stop_offset:  usize = 0;

        for elt in elements {
            let elt_len = elt.len();
            if !in_range {
                if char_index + elt_len > start {
                    start_index = elt_index;
                    start_offset = start - char_index;
                    in_range = true;
                } else if char_index == start && elt.is_attr_open() {
                    start_index = elt_index;
                    in_range = true;
                } else if elt.is_attr_open() || elt.is_text() {
                    start_index = elt_index;
                }
            } else {
                if char_index == stop && !elt.is_marker() && !elt.is_attr_close() {
                    stop_index = elt_index - 1;
                    break;
                } else if char_index + elt_len > stop {
                    stop_index = elt_index;
                    stop_offset = stop - char_index;
                    break;
                } else if elt.is_text() || elt.is_attr_open() {
                    stop_index = elt_index;
                }
            }
            char_index += elt_len;
            elt_index += 1;
        }
        Range::new(start_index, start_offset, stop_index, stop_offset)
    }

    fn new(start_index: usize, start_offset: usize, stop_index: usize, stop_offset: usize) -> Self {
        let start = Index::new(start_index, start_offset);
        let stop = Index::new(stop_index, stop_offset);
        match start <= stop {
            true  => Range{start: start, stop: stop},
            false => Range{start: start.clone(), stop: start},
        }
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
