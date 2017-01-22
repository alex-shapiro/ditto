pub mod element;
mod btree;

use self::btree::{BTree, BTreeIter};
use self::element::Element;
use Error;
use op::local::{LocalOp, DeleteText, InsertText};
use op::remote::UpdateAttributedString;
use Replica;
use std::fmt::{self, Debug};

#[derive(Clone, PartialEq)]
pub struct AttributedString(BTree);

impl AttributedString {
    pub fn new() -> Self {
        AttributedString(BTree::new())
    }

    pub fn assemble(elements: Vec<Element>) -> Self {
        let mut btree = BTree::new();
        for element in elements {
            btree.insert(element)
        }
        AttributedString(btree)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn elements(&self) -> BTreeIter {
        self.0.into_iter()
    }

    pub fn insert_text(&mut self, index: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if text.is_empty() { return Err(Error::Noop) }

        let (uid, offset) = {
            let (ref element, offset) = self.0.search(index)?;
            (element.uid.clone(), offset)
        };

        let deletes = match offset {
            0 => vec![],
            _ => vec![self.0.delete(&uid).expect("Element must exist!")],
        };

        let inserts = {
            let (ref prev, _) = self.0.search(index-1)?;
            let (ref next, _) = self.0.search(index)?;
            if offset == 0 {
                vec![Element::between(prev, next, text, replica)]
            } else {
                let (text_pre, text_post) = split_at_char(&deletes[0].text, offset);
                let pre = Element::between(prev, next, text_pre.to_owned(), replica);
                let new = Element::between(&pre, next, text, replica);
                let post = Element::between(&new, next, text_post.to_owned(), replica);
                vec![pre, new, post]
            }
        };

        for e in &inserts { self.0.insert(e.clone()) }
        Ok(UpdateAttributedString{inserts: inserts, deletes: deletes})
    }

    pub fn delete_text(&mut self, index: usize, len: usize, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if len == 0 { return Err(Error::Noop) }
        let mut deleted_len = 0;

        let (uid, offset) = {
            let (ref element, offset) = self.0.search(index)?;
            (element.uid.clone(), offset)
        };

        let mut deletes = vec![self.0.delete(&uid).expect("Element must exist!")];

        while deleted_len < len {
            let uid = {
                let (elt, _) = self.0.search(index)?;
                elt.uid.clone()
            };

            let element = self.0.delete(&uid).expect("Element must exist!");
            deleted_len += element.len;
            deletes.push(element);
        }

        let mut inserts = vec![];
        if offset > 0 || deleted_len > len {
            let (prev, _) = self.0.search(index-1)?;
            let (next, _) = self.0.search(index)?;

            if offset > 0 {
                let (text, _) = split_at_char(&deletes[0].text, offset);
                inserts.push(Element::between(prev, next, text.to_owned(), replica));
            }

            if deleted_len > len {
                let overdeleted_text = &deletes.last().expect("Element must exist!").text;
                let (_, text)  = split_at_char(overdeleted_text, deleted_len-len);
                let element = {
                    let prev = if inserts.is_empty() { prev } else { &inserts[0] };
                    Element::between(prev, next, text.to_owned(), replica)
                };
                inserts.push(element);
            }
        };

        for e in &inserts { self.0.insert(e.clone()) }
        Ok(UpdateAttributedString{inserts: inserts, deletes: deletes})
    }

    pub fn replace_text(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if index + len > self.len() { return Err(Error::OutOfBounds) }
        if len == 0 && text.is_empty() { return Err(Error::Noop) }

        let mut op1 = self.delete_text(index, len, replica).unwrap_or(UpdateAttributedString::default());
        let mut op2 = self.insert_text(index, text, replica).unwrap_or(UpdateAttributedString::default());
        op1.merge(&mut op2);
        Ok(op1)
    }

    pub fn execute_remote(&mut self, op: &UpdateAttributedString) -> Vec<LocalOp> {
        let mut local_ops = Vec::with_capacity(op.inserts.len() + op.deletes.len());

        for element in &op.inserts {
            self.0.insert(element.clone());
            let char_index = self.0.index_of(&element.uid).expect("Element must exist!");
            let op = InsertText::new(char_index, element.text.clone());
            local_ops.push(LocalOp::InsertText(op));
        }

        for element in &op.deletes {
            if let Ok(char_index) = self.0.index_of(&element.uid) {
                self.0.delete(&element.uid);
                let op = DeleteText::new(char_index, element.len);
                local_ops.push(LocalOp::DeleteText(op));
            }
        }

        local_ops
    }

    pub fn raw_string(&self) -> String {
        let mut raw = String::with_capacity(self.0.len());
        for elt in self.elements() { raw.push_str(&elt.text) }
        raw
    }
}

impl Debug for AttributedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let text = self.raw_string();
        write!(f, "AttributedString{{text: \"{}\", len: {}}}", text, self.0.len())
    }
}

fn split_at_char(string: &str, char_index: usize) -> (&str, &str) {
    let mut bidx = 0;
    let mut cidx = 0;

    for b in string.bytes() {
        if cidx == char_index { break }
        if (b as i8) >= -0x40 { cidx += 1 }
        bidx += 1;
    }

    string.split_at(bidx)
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

    #[test]
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
