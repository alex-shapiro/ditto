use super::element::Element;

#[derive(Clone,PartialEq,PartialOrd,Debug)]
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

#[derive(Debug)]
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
                if char_index == stop && !elt.is_attr_close() {
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
    use sequence::uid::UID;
    use Replica;

    #[test]
    fn test_new() {
        let range = Range::new(43, 0, 53, 3);
        assert!(range.start == Index::Whole{index: 43});
        assert!(range.stop == Index::Part{index: 53, offset: 3});
    }

    #[test]
    fn test_excluding_boundary_attrs_0() {
        let elements = test_elements();
        let range = Range::excluding_boundary_attrs(&elements, 0, 17);
        assert!(range.start == Index::Whole{index: 1});
        assert!(range.stop == Index::Whole{index: 6});
    }

    #[test]
    fn test_excluding_boundary_attrs_1() {
        let elements = test_elements();
        let range = Range::excluding_boundary_attrs(&elements, 6, 11);
        assert!(range.start == Index::Whole{index: 4});
        assert!(range.stop == Index::Whole{index: 6});
    }

    #[test]
    fn test_excluding_boundary_attrs_2() {
        let elements = test_elements();
        let range = Range::excluding_boundary_attrs(&elements, 4, 8);
        assert!(range.start == Index::Part{index: 1, offset: 4});
        assert!(range.stop == Index::Part{index: 6, offset: 3});
    }

    #[test]
    fn test_including_boundary_attrs_0() {
        let elements = test_elements();
        let range = Range::including_boundary_attrs(&elements, 0, 17);
        assert!(range.start == Index::Whole{index: 1});
        assert!(range.stop == Index::Whole{index: 7});
    }

    #[test]
    fn test_including_boundary_attrs_1() {
        let elements = test_elements();
        let range = Range::including_boundary_attrs(&elements, 6, 3);
        assert!(range.start == Index::Whole{index: 2});
        assert!(range.stop == Index::Whole{index: 5});
    }

    #[test]
    fn test_including_boundary_attrs_2() {
        let elements = test_elements();
        let range = Range::including_boundary_attrs(&elements, 4, 12);
        assert!(range.start == Index::Part{index: 1, offset: 4});
        assert!(range.stop == Index::Part{index: 6, offset: 7});
    }

    fn test_elements() -> Vec<Element> {
        build_elements(vec![
            text("hello "),
            attropen("A",""),
            attropen("B",""),
            text("and"),
            attrclose("B"),
            text(" goodbye"),
            attrclose("A"),
        ])
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
        let replica = Replica::new(1,1);

        for value in elt_values {
            let uid = UID::between(&elements.last().unwrap().uid, &end_marker.uid, &replica);
            elements.push(Element::new(value, uid));
        }
        elements.push(end_marker);
        elements
    }
}
