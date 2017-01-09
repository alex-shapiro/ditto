//! A half-open interval over a sequence of AttributedString
//! elements, from a lower bound up to, but not including, an
//! upper bound. A complication is that a bound may be split,
//! indicating that the range includes *part* but not all of
//! an element.

use super::element::Element;

#[derive(Debug,PartialEq)]
pub struct Bound {
    pub index: usize,
    pub offset: usize,
}

impl Bound {
    pub fn new(elements: &[Element], char_index: usize) -> Self {
        let mut current_char_index = 0;
        for (index, elt) in elements.iter().enumerate() {
            if current_char_index + elt.len > char_index {
                let offset = char_index - current_char_index;
                return Bound{index:index, offset: offset};
            } else if elt.is_end_marker() {
                return Bound{index: index, offset: 0};
            }
            current_char_index += elt.len;
        }
        Bound{index: elements.len() - 1, offset: 0}
    }
}

#[derive(Debug)]
pub struct Range {
    pub lower: Bound,
    pub upper: Bound,
}

impl Range {
    pub fn new(elements: &[Element], start: usize, len: usize) -> Self {
        let lower = Bound::new(elements, start);
        let elements_rest = &elements[lower.index..elements.len()];
        let mut upper = Bound::new(elements_rest, len + lower.offset);
        upper.index += lower.index;
        Range{lower: lower, upper: upper}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::element::*;
    use sequence::uid::UID;
    use Replica;

    #[test]
    fn test_new_all_elements() {
        let elements = test_elements();
        let range = Range::new(&elements, 0, 44);
        assert!(range.lower == Bound{index: 1, offset: 0});
        assert!(range.upper == Bound{index: 10, offset: 0});
    }

    #[test]
    fn test_new_no_split_bounds() {
        let elements = test_elements();
        let range = Range::new(&elements, 10, 16);
        assert!(range.lower == Bound{index: 3, offset: 0});
        assert!(range.upper == Bound{index: 6, offset: 0});
    }

    #[test]
    fn test_new_split_lower_bound() {
        let elements = test_elements();
        let range = Range::new(&elements, 11, 20);
        assert!(range.lower == Bound{index: 3, offset: 1});
        assert!(range.upper == Bound{index: 7, offset: 0});
    }

    #[test]
    fn test_new_split_upper_bound() {
        let elements = test_elements();
        let range = Range::new(&elements, 4, 10);
        assert!(range.lower == Bound{index: 2, offset: 0});
        assert!(range.upper == Bound{index: 3, offset: 4});
    }

    #[test]
    fn test_new_single_element_split() {
        let elements = test_elements();
        let range = Range::new(&elements, 21, 2);
        assert!(range.lower == Bound{index: 5, offset: 1});
        assert!(range.upper == Bound{index: 5, offset: 3});
    }

    #[test]
    fn test_new_zero_length_range() {
        let elements = test_elements();
        let range = Range::new(&elements, 21, 0);
        assert!(range.lower == Bound{index: 5, offset: 1});
        assert!(range.upper == Bound{index: 5, offset: 1});
    }

    fn test_elements() -> Vec<Element> {
        build_elements(vec![
            text("The "),
            text("quick "),
            text("brown "),
            text("fox "),
            text("jumps "),
            text("over "),
            text("the "),
            text("lazy "),
            text("dog."),
        ])
    }

    fn text(value: &str) -> EltValue {
        EltValue::Text(value.to_string())
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
