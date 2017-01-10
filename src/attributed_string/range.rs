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
