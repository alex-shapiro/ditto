pub mod element;
mod index;
mod range;

use self::element::Element;
use self::range::{Bound, Range};
use self::index::Index;
use Error;
use op::local::{LocalOp, DeleteText, InsertText};
use op::remote::UpdateAttributedString;
use Replica;
use std::mem;

#[derive(Debug,Clone,PartialEq)]
pub struct AttributedString{
    len: usize,
    elements: Vec<Element>,
}

impl AttributedString {
    pub fn new() -> Self {
        AttributedString{
            elements: vec![Element::start_marker(), Element::end_marker()],
            len: 0,
        }
    }

    pub fn assemble(elements: Vec<Element>, len: usize) -> Self {
        AttributedString{elements: elements, len: len}
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn insert_text(&mut self, index: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if index > self.len { return Err(Error::OutOfBounds) }
        if text.is_empty() { return Err(Error::Noop) }

        self.len += text.len();
        let bound = Bound::new(&self.elements, index);
        Ok(match bound.offset {
            0 => self.insert_at_index(bound.index, text, replica),
            _ => self.insert_in_index(bound.index, bound.offset, text, replica),
        })
    }

    pub fn delete_text(&mut self, index: usize, len: usize, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if index + len > self.len { return Err(Error::OutOfBounds) }
        if len == 0 { return Err(Error::Noop) }

        self.len -= len;
        let range = Range::new(&self.elements, index, len);
        Ok(match range.lower.index == range.upper.index {
            true  => self.delete_in_element(&range, replica),
            false => self.delete_in_range(&range, replica),
        })
    }

    pub fn replace_text(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if index + len > self.len { return Err(Error::OutOfBounds) }
        if len == 0 && text.is_empty() { return Err(Error::Noop) }

        let mut op1 = self.delete_text(index, len, replica).unwrap_or(UpdateAttributedString::default());
        let mut op2 = self.insert_text(index, text, replica).unwrap_or(UpdateAttributedString::default());
        op1.merge(&mut op2);
        Ok(op1)
    }

    pub fn execute_remote(&mut self, op: &UpdateAttributedString) -> Vec<LocalOp> {
        let elements = mem::replace(&mut self.elements, Vec::new());
        let mut insert_iter = op.inserts.iter().peekable();
        let mut delete_iter = op.deletes.iter().peekable();
        let mut local_ops = Vec::new();

        let mut char_index = 0;
        let max_elt = Element::end_marker();

        for elt in elements {
            let should_delete_elt = {
                let deleted_elt = *delete_iter.peek().unwrap_or(&&max_elt);
                elt < max_elt && elt == *deleted_elt
            };
            // if elt matches the next deleted UID, delete elt
            if should_delete_elt {
                self.len -= elt.len;
                delete_iter.next();
                let op = DeleteText::new(char_index, elt.len);
                local_ops.push(LocalOp::DeleteText(op));

            // otherwise insert all new elements that come before elt,
            // then re-insert elt
            } else {
                while *insert_iter.peek().unwrap_or(&&max_elt) < &elt {
                    let ins = insert_iter.next().unwrap().clone();
                    let text = ins.text.to_string();
                    let text_len = text.len();
                    let op = InsertText::new(char_index, text);
                    local_ops.push(LocalOp::InsertText(op));
                    self.elements.push(ins);
                    self.len += text_len;
                    char_index += text_len;
                }
                char_index += elt.len;
                self.elements.push(elt);
            }
        }
        local_ops
    }

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
            let (text_pre, text_post) = original_elt.text.split_at(offset);
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
            vec![original_elt],
        )
    }

    fn delete_in_element(&mut self, range: &Range, replica: &Replica) -> UpdateAttributedString {
        let ref mut element = self.elements[range.lower.index];
        let deleted_element = element.clone();
        element.cut_middle(range.lower.offset, range.upper.offset, replica);
        let insert = element.clone();
        UpdateAttributedString::new(vec![insert], vec![deleted_element])
    }

    fn delete_in_range(&mut self, range: &Range, replica: &Replica) -> UpdateAttributedString {
        let mut deletes: Vec<Element> = vec![];
        let mut inserts: Vec<Element> = vec![];
        let mut lower_index = range.lower.index;
        let upper_index = range.upper.index;

        // if part of the lower-bound element is deleted, update the
        // element in-place instead of deleting and re-inserting it
        if range.lower.offset > 0 {
            let ref mut element = self.elements[range.lower.index];
            deletes.push(element.clone());
            element.cut_right(range.lower.offset, replica);
            inserts.push(element.clone());
            lower_index += 1;
        }

        // same for the upper-bound element
        if range.upper.offset > 0 {
            let ref mut element = self.elements[range.upper.index];
            deletes.push(element.clone());
            element.cut_left(range.upper.offset, replica);
            inserts.push(element.clone());
        }

        for _ in lower_index..upper_index {
            deletes.push(self.elements.remove(lower_index));
        }

        deletes.sort();
        UpdateAttributedString::new(inserts, deletes)
    }

    pub fn elements(&self) -> &[Element] {
        let lower = 1;
        let upper = self.elements.len() - 1;
        &self.elements[lower..upper]
    }

    pub fn raw_string(&self) -> String {
        let mut raw = String::with_capacity(self.len());
        for elt in &self.elements {
            if elt.is_text() {
                raw.push_str(&elt.text);
            }
        }
        raw
    }

    pub fn index(&self, index: &Index, distance: usize) -> Result<Index, Error> {
        let location = index.location + distance;
        if location > self.len { return Err(Error::OutOfBounds) }
        if location == self.len { return Ok(self.end_index()) }

        let mut eidx = index.eidx;
        let mut cidx = index.cidx;
        let mut bidx = index.bidx;
        let mut distance_left = distance;

        for element in &self.elements[eidx..] {
            if distance_left < element.len - cidx {
                for c in element.text[cidx..].chars() {
                    if distance_left == 0 {
                        return Ok(Index{eidx: eidx, cidx: cidx, bidx: bidx, location: location})
                    }

                    cidx += 1;
                    bidx += c.len_utf8();
                    distance_left -= 1;
                }
            }

            distance_left -= element.len - cidx;
            eidx += 1;
            cidx = 0;
            bidx = 0;
        }
        Err(Error::OutOfBounds)
    }

    pub fn start_index(&self) -> Index {
        Index{eidx: 1, cidx: 0, bidx: 0, location: 0}
    }

    pub fn end_index(&self) -> Index {
        Index{eidx: self.elements.len() - 1, bidx: 0, cidx: 0, location: self.len}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::element::Element;
    use super::index::Index;
    use Error;
    use op::remote::UpdateAttributedString;
    use Replica;
    use sequence::uid::{self, UID};

    const REPLICA1: Replica = Replica{site: 5, counter: 1023};
    const REPLICA2: Replica = Replica{site: 8, counter: 16};

    #[test]
    fn test_new() {
        let string = AttributedString::new();
        assert!(string.len() == 0);
        assert!(string.elements[0] == Element::start_marker());
        assert!(string.elements[1] == Element::end_marker());
    }

    #[test]
    fn test_insert_empty_string() {
        let mut string = AttributedString::new();
        let op = string.insert_text(0, "".to_string(), &REPLICA1);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_insert_text_when_empty() {
        let mut string = AttributedString::new();
        let op = string.insert_text(0, "quick".to_string(), &REPLICA1).unwrap();

        assert!(string.len() == 5);
        assert!(text(&string, 1) == "quick");

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == string.elements[1].uid);
        assert!(op.inserts[0].text == "quick");
        assert!(op.deletes.len() == 0);
    }

    #[test]
    fn test_insert_text_before_index() {
        let mut string = AttributedString::new();
        let  _ = string.insert_text(0, "the ".to_string(), &REPLICA1);
        let  _ = string.insert_text(4, "brown".to_string(), &REPLICA1);
        let op = string.insert_text(4, "quick ".to_string(), &REPLICA2).unwrap();

        assert!(string.len() == 15);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "quick ");
        assert!(text(&string, 3) == "brown");

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == string.elements[2].uid);
        assert!(op.inserts[0].text == "quick ");
        assert!(op.deletes.len() == 0);
    }

    #[test]
    fn test_insert_text_in_index() {
        let mut string = AttributedString::new();
        let op1 = string.insert_text(0, "the  ".to_string(), &REPLICA1).unwrap();
        let   _ = string.insert_text(5, "brown".to_string(), &REPLICA1);
        let op2 = string.insert_text(4, "quick".to_string(), &REPLICA2).unwrap();

        assert!(string.len() == 15);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "quick");
        assert!(text(&string, 3) == " ");
        assert!(text(&string, 4) == "brown");

        assert!(op2.inserts.len() == 3);
        assert!(op2.inserts[0].uid == string.elements[1].uid);
        assert!(op2.inserts[1].uid == string.elements[2].uid);
        assert!(op2.inserts[2].uid == string.elements[3].uid);
        assert!(op2.inserts[0].text == "the ");
        assert!(op2.inserts[1].text == "quick");
        assert!(op2.inserts[2].text == " ");

        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_insert_text_invalid() {
        let mut string = AttributedString::new();
        let op = string.insert_text(1, "quick".to_string(), &REPLICA1);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_delete_zero_text() {
        let mut string = AttributedString::new();
        let  _ = string.insert_text(0, "the ".to_string(), &REPLICA1);
        let op = string.delete_text(1, 0, &REPLICA2);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_delete_text_whole_single_element() {
        let mut string = AttributedString::new();
        let   _ = string.insert_text(0, "the ".to_string(), &REPLICA1);
        let op1 = string.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        let   _ = string.insert_text(10, "brown".to_string(), &REPLICA1);
        let op2 = string.delete_text(4, 6, &REPLICA2).unwrap();

        assert!(string.len() == 9);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "brown");

        assert!(op2.inserts.len() == 0);
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_delete_text_whole_multiple_elements() {
        let mut string = AttributedString::new();
        let   _ = string.insert_text(0, "the ".to_string(), &REPLICA1);
        let op1 = string.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        let op2 = string.insert_text(10, "brown".to_string(), &REPLICA1).unwrap();
        let op3 = string.delete_text(4, 11, &REPLICA2).unwrap();

        assert!(string.len() == 4);
        assert!(text(&string, 1) == "the ");

        assert!(op3.inserts.len() == 0);
        assert!(op3.deletes.len() == 2);
        assert!(op3.deletes[0] == op1.inserts[0]);
        assert!(op3.deletes[1] == op2.inserts[0]);
    }

    #[test]
    fn test_delete_text_split_single_element() {
        let mut string = AttributedString::new();
        let   _ = string.insert_text(0, "the ".to_string(), &REPLICA1);
        let op1 = string.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        let   _ = string.insert_text(10, "brown".to_string(), &REPLICA1);
        let op2 = string.delete_text(5, 3, &REPLICA2).unwrap();

        assert!(string.len() == 12);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "qk ");
        assert!(text(&string, 3) == "brown");

        assert!(op2.inserts.len() == 1);
        assert!(op2.inserts[0].text == "qk ");
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_delete_text_split_multiple_elements() {
        let mut string = AttributedString::new();
        let op1 = string.insert_text(0, "the ".to_string(), &REPLICA1).unwrap();
        let   _ = string.insert_text(4, "quick ".to_string(), &REPLICA1);
        let   _ = string.insert_text(10, "brown ".to_string(), &REPLICA1);
        let   _ = string.insert_text(16, "fox ".to_string(), &REPLICA1);
        let op2 = string.insert_text(20, "jumps ".to_string(), &REPLICA1).unwrap();
        let   _ = string.insert_text(26, "over".to_string(), &REPLICA1);
        let op3 = string.delete_text(2, 19, &REPLICA2).unwrap();

        assert!(string.len() == 11);
        assert!(text(&string, 1) == "th");
        assert!(text(&string, 2) == "umps ");
        assert!(text(&string, 3) == "over");

        assert!(op3.inserts.len() == 2);
        assert!(op3.inserts[0].text == "th");
        assert!(op3.inserts[1].text == "umps ");
        assert!(op3.deletes.len() == 5);
        assert!(op3.deletes[0] == op1.inserts[0]);
        assert!(op3.deletes[4] == op2.inserts[0]);
    }

    #[test]
    fn test_delete_text_invalid() {
        let mut string = AttributedString::new();
        let op = string.delete_text(0, 1, &REPLICA2);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_replace_text_delete_only() {
        let mut string = AttributedString::new();
        let op1 = string.insert_text(0, "hello world".to_string(), &REPLICA1).unwrap();
        let op2 = string.replace_text(2, 6, "".to_string(), &REPLICA2).unwrap();

        assert!(string.len() == 5);
        assert!(text(&string, 1) == "herld");

        assert!(op2.inserts.len() == 1);
        assert!(op2.inserts[0].text == "herld");
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_replace_text_insert_only() {
        let mut string = AttributedString::new();
        let op1 = string.insert_text(0, "the fox".to_string(), &REPLICA1).unwrap();
        let op2 = string.replace_text(4, 0, "quick ".to_string(), &REPLICA2).unwrap();

        assert!(string.len() == 13);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "quick ");
        assert!(text(&string, 3) == "fox");

        assert!(op2.inserts.len() == 3);
        assert!(op2.inserts[0].text == "the ");
        assert!(op2.inserts[1].text == "quick ");
        assert!(op2.inserts[2].text == "fox");
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_replace_text_delete_and_insert() {
        let mut string = AttributedString::new();
        let op1 = string.insert_text(0, "the brown fox".to_string(), &REPLICA1).unwrap();
        let op2 = string.replace_text(4, 5, "qwik".to_string(), &REPLICA2).unwrap();

        assert!(string.len() == 12);
        assert!(text(&string, 1) == "the ");
        assert!(text(&string, 2) == "qwik");
        assert!(text(&string, 3) == " fox");

        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
        assert!(op2.inserts.len() == 3);
        assert!(op2.inserts[0].text == "the ");
        assert!(op2.inserts[1].text == "qwik");
        assert!(op2.inserts[2].text == " fox");
    }

    #[test]
    fn test_replace_invalid() {
        let mut string = AttributedString::new();
        let   _ = string.insert_text(0, "the quick brown fox".to_string(), &REPLICA1);
        let op2 = string.replace_text(4, 16, "slow green turtle".to_string(), &REPLICA2);
        assert!(op2 == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_execute_remote_empty() {
        let mut string = AttributedString::new();
        let mut op = UpdateAttributedString::default();
        let local_ops = string.execute_remote(&mut op);
        assert!(local_ops.len() == 0);
    }

    #[test]
    fn test_execute_remote() {
        let mut string1 = AttributedString::new();
        let mut op1 = string1.insert_text(0, "the brown".to_string(), &REPLICA1).unwrap();
        let mut op2 = string1.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        let mut op3 = string1.replace_text(6, 1, "a".to_string(), &REPLICA1).unwrap();

        let mut string2 = AttributedString::new();
        let lops1 = string2.execute_remote(&mut op1);
        let lops2 = string2.execute_remote(&mut op2);
        let lops3 = string2.execute_remote(&mut op3);

        assert!(string1 == string2);
        assert!(lops1.len() == 1);
        assert!(lops2.len() == 4);
        assert!(lops3.len() == 4);

        let lop1 = lops1[0].insert_text().unwrap();
        let lop2 = lops2[0].delete_text().unwrap();
        let lop3 = lops2[1].insert_text().unwrap();
        let lop4 = lops2[2].insert_text().unwrap();
        let lop5 = lops2[3].insert_text().unwrap();
        let lop6 = lops3[0].delete_text().unwrap();
        let lop7 = lops3[1].insert_text().unwrap();
        let lop8 = lops3[2].insert_text().unwrap();
        let lop9 = lops3[3].insert_text().unwrap();

        assert!(lop1.index == 0 && lop1.text == "the brown");
        assert!(lop2.index == 0 && lop2.len == 9);
        assert!(lop3.index == 0 && lop3.text == "the ");
        assert!(lop4.index == 4 && lop4.text == "quick ");
        assert!(lop5.index == 10 && lop5.text == "brown");
        assert!(lop6.index == 4 && lop6.len == 6);
        assert!(lop7.index == 4 && lop7.text == "qu");
        assert!(lop8.index == 6 && lop8.text == "a");
        assert!(lop9.index == 7 && lop9.text == "ck ");
    }

    #[test]
    fn test_raw_string() {
        let mut string = AttributedString::new();
        string.insert_text(0, "the brown".to_string(), &REPLICA1).unwrap();
        string.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        assert!(string.raw_string() == "the quick brown");
    }

    #[test]
    fn test_index_from_start() {
        let s = build_string(vec!["thé","qüiçk","brøwn","fox🇨🇦😀"]);
        let i = s.start_index();

        assert!(s.index(&i, 0).unwrap()  == idx(1,0,0,0));
        assert!(s.index(&i, 1).unwrap()  == idx(1,1,1,1));
        assert!(s.index(&i, 2).unwrap()  == idx(1,2,2,2));
        assert!(s.index(&i, 3).unwrap()  == idx(2,0,0,3));
        assert!(s.index(&i, 4).unwrap()  == idx(2,1,1,4));
        assert!(s.index(&i, 5).unwrap()  == idx(2,2,3,5));
        assert!(s.index(&i, 6).unwrap()  == idx(2,3,4,6));
        assert!(s.index(&i, 7).unwrap()  == idx(2,4,6,7));
        assert!(s.index(&i, 8).unwrap()  == idx(3,0,0,8));
        assert!(s.index(&i, 9).unwrap()  == idx(3,1,1,9));
        assert!(s.index(&i, 10).unwrap() == idx(3,2,2,10));
        assert!(s.index(&i, 11).unwrap() == idx(3,3,4,11));
        assert!(s.index(&i, 12).unwrap() == idx(3,4,5,12));
        assert!(s.index(&i, 13).unwrap() == idx(4,0,0,13));
        assert!(s.index(&i, 14).unwrap() == idx(4,1,1,14));
        assert!(s.index(&i, 15).unwrap() == idx(4,2,2,15));
        assert!(s.index(&i, 16).unwrap() == idx(4,3,3,16));
        assert!(s.index(&i, 17).unwrap() == idx(4,4,7,17));
        assert!(s.index(&i, 18).unwrap() == idx(4,5,11,18));
        assert!(s.index(&i, 19).unwrap() == idx(5,0,0,19));
        assert!(s.index(&i, 20) == Err(Error::OutOfBounds));
    }

    fn test_index() {
        let s = build_string(vec!["thé","qüiçk","brøwn","fox🇨🇦😀"]);
        let j = s.index(&s.start_index(), 10).unwrap();

        assert!(s.index(&j, 0).unwrap() == idx(3,2,2,10));
        assert!(s.index(&j, 1).unwrap() == idx(3,3,4,11));
        assert!(s.index(&j, 2).unwrap() == idx(3,4,5,12));
        assert!(s.index(&j, 3).unwrap() == idx(4,0,0,13));
        assert!(s.index(&j, 4).unwrap() == idx(4,1,1,14));
        assert!(s.index(&j, 5).unwrap() == idx(4,2,2,15));
        assert!(s.index(&j, 6).unwrap() == idx(4,3,3,16));
        assert!(s.index(&j, 7).unwrap() == idx(4,4,7,17));
        assert!(s.index(&j, 8).unwrap() == idx(4,5,11,18));
        assert!(s.index(&j, 9).unwrap() == idx(5,0,0,19));
        assert!(s.index(&j, 10) == Err(Error::OutOfBounds));
    }

    fn text<'a>(string: &'a AttributedString, index: usize) -> &'a str {
        &string.elements[index].text
    }

    fn build_string(text_vec: Vec<&'static str>) -> AttributedString {
        let mut elements: Vec<Element> = vec![Element::start_marker()];
        let end_marker = Element::end_marker();
        let replica = Replica::new(1,1);
        let mut len = 0;

        for text in text_vec {
            let uid = UID::between(&elements.last().unwrap().uid, &*uid::MAX, &replica);
            elements.push(Element::text(text.to_owned(), uid));
            len += text.chars().count();
        }
        elements.push(end_marker);
        AttributedString{len: len, elements: elements}
    }

    fn idx(eidx: usize, cidx: usize, bidx: usize, location: usize) -> Index {
        Index{eidx: eidx, cidx: cidx, bidx: bidx, location: location}
    }
}
