pub mod element;
mod range;

use self::element::Element;
use self::range::Range;
use self::range::Bound;
use sequence::uid::UID;
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
        if index > self.len { return None }
        let bound = Bound::new(&self.elements, index);
        Some(match bound.offset {
            0 => self.insert_at_index(bound.index, text, replica),
            _ => self.insert_in_index(bound.index, bound.offset, text, replica),
        })
    }

    pub fn delete_text(&mut self, index: usize, len: usize, replica: &Replica) -> Option<UpdateAttributedString> {
        if index >= self.len { return None }
        let range = Range::new(&self.elements, index, len);
        Some(self.delete_in_range(&range, replica))
    }

    // pub fn replace_text(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Option<UpdateAttributedString> {
    //     if index >= self.len { return None }
    //     let op1 = self.delete_text(index, len, replica).unwrap();
    //     let op2 = self.insert_text(index, text, replica).unwrap();
    //     Some(op1.merge(op2));
    // }

    // pub fn execute_remote(&mut self, op: UpdateAttributedString) -> Vec<Box<LocalOp>> {
    //     let delete_ops: Vec<DeleteText> =
    //         op.deletes.into_iter()
    //         .map(|uid| self.delete_remote(uid))
    //         .filter(|op| op.is_some())
    //         .map(|op| op.unwrap())
    //         .collect();

    //     let insert_ops: Vec<InsertItem> =
    //         op.inserts.into_iter()
    //         .map(|elt| self.insert_remote(elt))
    //         .filter(|op| op.is_some())
    //         .map(|op| op.unwrap())
    //         .collect();

    //     let mut local_ops: Vec<Box<LocalOp>> = vec![];
    //     for op in delete_ops { local_ops.push(Box::new(op)); }
    //     for op in insert_ops { local_ops.push(Box::new(op)); }
    //     local_ops

    // }

    fn insert_at_index(&mut self, index: usize, text: String, replica: &Replica) -> UpdateAttributedString {
        let elt_new = {
            let ref elt1 = self.elements[index-1];
            let ref elt2 = self.elements[index];
            Element::between(elt1, elt2, text, replica)
        };

        self.elements.insert(index, elt_new.clone());
        UpdateAttributedString::new(vec![elt_new], vec![])
    }

    fn insert_in_index(&mut self, index: usize, offset: usize, text: String, replica: &Replica) -> UpdateAttributedString {
        let original_elt = self.elements.remove(index);

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

        self.elements.insert(index, elt_post.clone());
        self.elements.insert(index, elt_new.clone());
        self.elements.insert(index, elt_pre.clone());
        UpdateAttributedString::new(
            vec![elt_pre, elt_new, elt_post],
            vec![original_elt.uid]
        )
    }

    fn delete_in_range(&mut self, range: &Range, replica: &Replica) -> UpdateAttributedString {
        let mut deletes: Vec<Element> = vec![];
        let mut inserts: Vec<Element> = vec![];
        let mut lower_index = range.lower.index;
        let mut upper_index = range.upper.index;

        // if only part of the lower-bound element is deleted, replace
        // the element in-place instead of deleting and re-inserting it
        if range.lower.offset > 0 {
            let ref mut element = self.elements[range.lower.index];
            deletes.push(element.clone());
            element.trim_left(range.lower.offset, replica);
            inserts.push(element.clone());
            lower_index += 1;
        }

        // same for the upper-bound element, taking care to not
        // duplicate inserted/deleted elements if the lower bound
        // and upper bound are on the same element.
        if range.upper.offset > 0 {
            let ref mut element = self.elements[range.upper.index];
            if range.lower.index != range.upper.index {
                deletes.push(element.clone());
            }
            element.trim_right(range.lower.offset, replica);
            if range.lower.index == range.upper.index {
                inserts = vec![element.clone()];
            }
            upper_index -= 1;
        }

        for i in lower_index..upper_index {
            deletes.push(self.elements.remove(i));
        }

        let mut deleted_uids: Vec<UID> = deletes.into_iter().map(|e| e.uid).collect();
        deleted_uids.sort();
        UpdateAttributedString::new(inserts, deleted_uids)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use super::element::Element;

//     #[test]
//     fn test_new() {
//         let string = AttributedString::new();
//         assert!(string.len() == 0);
//         assert!(string.elements[0] == Element::start_marker());
//         assert!(string.elements[1] == Element::end_marker());
//     }
// }
