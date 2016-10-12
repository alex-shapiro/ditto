mod attr;
pub mod element;
mod range;

use self::element::Element;
use self::range::Range;
use self::range::Index;
use op::remote::UpdateAttributedString;
use Replica;

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

    pub fn insert_text(&mut self, index: usize, text: String, replica: &Replica) -> Option<UpdateAttributedString> {
        if index >= self.len { return None }
        let range = Range::excluding_boundary_attrs(&self.elements, index, 0);

        Some(match range.start {
            Index::Whole{index: elt_index} =>
                self.insert_between_elements(elt_index, text, replica),
            Index::Part{index: elt_index, offset} =>
                self.insert_in_element(elt_index, offset, text, replica),
        })
    }

    fn insert_between_elements(&mut self, index: usize, text: String, replica: &Replica)-> UpdateAttributedString {
        let elt_new = {
            let ref elt_pre = self.elements[index-1];
            let ref elt_post = self.elements[index];
            Element::between(elt_pre, elt_post, text, replica)
        };

        self.elements.insert(index, elt_new.clone());
        UpdateAttributedString::new(vec![elt_new], vec![])
    }

    fn insert_in_element(&mut self, index: usize, offset: usize, text: String, replica: &Replica) -> UpdateAttributedString {
        let original_elt  = self.elements.remove(index);

        let (elt_pre, elt_new, elt_post) = {
            let original_text = original_elt.text().unwrap();
            let (text_pre, text_post) = original_text.split_at(offset);
            let ref elt_ppre = self.elements[index-1];
            let ref elt_ppost = self.elements[index];
            let elt_new  = Element::between(elt_ppre, elt_ppost, text, &replica);
            let elt_pre  = Element::between(elt_ppre, &elt_new, text_pre.to_string(), replica);
            let elt_post = Element::between(&elt_new, elt_ppost, text_post.to_string(), replica);
            (elt_pre, elt_new, elt_post)
        };

        self.elements.insert(index, elt_pre.clone());
        self.elements.insert(index+1, elt_new.clone());
        self.elements.insert(index+2, elt_post.clone());
        UpdateAttributedString::new(
            vec![elt_pre, elt_new, elt_post],
            vec![original_elt.uid]
        )
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
