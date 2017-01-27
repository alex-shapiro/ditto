pub mod element;
mod btree;

use self::btree::{BTree, BTreeIter};
use self::element::Element;
use Error;
use op::local::{LocalOp, DeleteText, InsertText};
use op::remote::UpdateAttributedString;
use Replica;

#[derive(Debug, Clone, PartialEq)]
pub struct AttributedString(BTree);

impl AttributedString {
    pub fn new() -> Self {
        AttributedString(BTree::new())
    }

    pub fn assemble(elements: Vec<Element>) -> Self {
        let mut btree = BTree::new();
        for element in elements {
            let _ = btree.insert(element);
        }
        AttributedString(btree)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn elements(&self) -> BTreeIter {
        self.0.into_iter()
    }

    pub fn to_string(&self) -> String {
        let mut string = String::with_capacity(self.0.len());
        for elt in self.elements() { string.push_str(&elt.text) }
        string
    }

    pub fn insert_text(&mut self, index: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if text.is_empty() { return Err(Error::Noop) }

        let (uid, offset) = {
            let (ref element, offset) = self.0.get_element(index)?;
            (element.uid.clone(), offset)
        };

        let deletes = match offset {
            0 => vec![],
            _ => vec![self.0.delete(&uid).expect("Element must exist!")],
        };

        let inserts = {
            let index = index - offset;
            let prev  = self.get_prev_element(index)?;
            let (next, _) = self.0.get_element(index)?;
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

        for e in &inserts { let _ = self.0.insert(e.clone()); }
        Ok(UpdateAttributedString{inserts: inserts, deletes: deletes})
    }

    pub fn delete_text(&mut self, index: usize, len: usize, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if len == 0 { return Err(Error::Noop) }
        if index + len > self.len() { return Err(Error::OutOfBounds) }

        let (element, offset) = self.delete_at(index)?;
        let border_index = index - offset;
        let mut deleted_len = element.len - offset;
        let mut deletes = vec![element];

        while deleted_len < len {
            let (element, _) = self.delete_at(border_index)?;
            deleted_len += element.len;
            deletes.push(element);
        }

        let mut inserts = vec![];
        if offset > 0 || deleted_len > len {
            let prev = self.get_prev_element(border_index)?;
            let (next, _) = self.0.get_element(border_index)?;

            if offset > 0 {
                let (text, _) = split_at_char(&deletes[0].text, offset);
                inserts.push(Element::between(prev, next, text.to_owned(), replica));
            }

            if deleted_len > len {
                let overdeleted_elt = &deletes.last().expect("Element must exist!");
                let offset = overdeleted_elt.len + len - deleted_len;
                let (_, text) = split_at_char(&overdeleted_elt.text, offset);
                let element = {
                    let prev = if inserts.is_empty() { prev } else { &inserts[0] };
                    Element::between(prev, next, text.to_owned(), replica)
                };
                inserts.push(element);
            }
        };

        for e in &inserts { let _ = self.0.insert(e.clone()); }
        Ok(UpdateAttributedString{inserts: inserts, deletes: deletes})
    }

    pub fn replace_text(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Result<UpdateAttributedString, Error> {
        if index + len > self.len() { return Err(Error::OutOfBounds) }
        if len == 0 && text.is_empty() { return Err(Error::Noop) }

        let mut op1 = self.delete_text(index, len, replica).unwrap_or(UpdateAttributedString::default());
        if let Ok(op2) = self.insert_text(index, text, replica) { op.merge(op2) };
        Ok(op1)
    }

    pub fn execute_remote(&mut self, op: &UpdateAttributedString) -> Vec<LocalOp> {
        let mut local_ops = Vec::with_capacity(op.inserts.len() + op.deletes.len());

        for element in &op.deletes {
            if let Some(char_index) = self.0.get_index(&element.uid) {
                self.0.delete(&element.uid);
                let op = DeleteText::new(char_index, element.len);
                local_ops.push(LocalOp::DeleteText(op));
            }
        }

        for element in &op.inserts {
            let _ = self.0.insert(element.clone());
            let char_index = self.0.get_index(&element.uid).expect("Element must exist!");
            let op = InsertText::new(char_index, element.text.clone());
            local_ops.push(LocalOp::InsertText(op));
        }

        local_ops
    }

    fn delete_at(&mut self, index: usize) -> Result<(Element, usize), Error> {
        let (uid, offset) = {
            let (element, offset) = self.0.get_element(index)?;
            (element.uid.clone(), offset)
        };
        let element = self.0.delete(&uid).expect("Element must exist for UID!");
        Ok((element, offset))
    }

    fn get_prev_element(&self, index: usize) -> Result<&Element, Error> {
        if index == 0 {
            Ok(&*element::START)
        } else {
            let (prev, _) = self.0.get_element(index-1)?;
            Ok(prev)
        }
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
    use Error;
    use op::remote::UpdateAttributedString;
    use Replica;

    const REPLICA1: Replica = Replica{site: 5, counter: 1023};
    const REPLICA2: Replica = Replica{site: 8, counter: 16};

    #[test]
    fn test_new() {
        let attrstr = AttributedString::new();
        assert!(attrstr.len() == 0);
        assert!(attrstr.to_string() == "");
    }

    #[test]
    fn test_insert_empty_string() {
        let mut attrstr = AttributedString::new();
        let op = attrstr.insert_text(0, "".to_string(), &REPLICA1);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_insert_text_when_empty() {
        let mut attrstr = AttributedString::new();
        let op = attrstr.insert_text(0, "quick".to_string(), &REPLICA1).unwrap();
        let element = elt_at(&attrstr, 0, "quick");

        assert!(attrstr.len() == 5);
        assert!(attrstr.to_string() == "quick");
        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == element.uid);
        assert!(op.inserts[0].text == element.text);
        assert!(op.deletes.is_empty());
    }

    #[test]
    fn test_insert_text_before_index() {
        let mut attrstr = AttributedString::new();
        let  _ = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1);
        let  _ = attrstr.insert_text(4, "brown".to_owned(), &REPLICA1);
        let op = attrstr.insert_text(4, "quick ".to_owned(), &REPLICA2).unwrap();

        assert!(attrstr.len() == 15);
        assert!(attrstr.to_string() == "the quick brown");

        let _  = elt_at(&attrstr,  0, "the ");
        let e1 = elt_at(&attrstr,  4, "quick ");
        let _  = elt_at(&attrstr, 10, "brown");

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == e1.uid);
        assert!(op.inserts[0].text == e1.text);
        assert!(op.deletes.len() == 0);
    }

    #[test]
    fn test_insert_text_in_index() {
        let mut attrstr = AttributedString::new();
        let op1 = attrstr.insert_text(0, "the  ".to_owned(), &REPLICA1).unwrap();
        let   _ = attrstr.insert_text(5, "brown".to_owned(), &REPLICA1);
        let op2 = attrstr.insert_text(4, "quick".to_owned(), &REPLICA2).unwrap();

        assert!(attrstr.len() == 15);
        assert!(attrstr.to_string() == "the quick brown");

        let e0 = elt_at(&attrstr,  0, "the ");
        let e1 = elt_at(&attrstr,  4, "quick");
        let e2 = elt_at(&attrstr,  9, " ");
        let _  = elt_at(&attrstr, 10, "brown");

        assert!(op2.inserts.len() == 3);
        assert!(op2.inserts[0].uid == e0.uid);
        assert!(op2.inserts[1].uid == e1.uid);
        assert!(op2.inserts[2].uid == e2.uid);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);

        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_insert_text_invalid() {
        let mut attrstr = AttributedString::new();
        let op = attrstr.insert_text(1, "quick".to_owned(), &REPLICA1);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_delete_zero_text() {
        let mut attrstr = AttributedString::new();
        let  _ = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1);
        let op = attrstr.delete_text(1, 0, &REPLICA2);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_delete_text_whole_single_element() {
        let mut attrstr = AttributedString::new();
        let   _ = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1);
        let op1 = attrstr.insert_text(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let   _ = attrstr.insert_text(10, "brown".to_owned(), &REPLICA1);
        let op2 = attrstr.delete_text(4, 6, &REPLICA2).unwrap();

        assert!(attrstr.len() == 9);
        assert!(attrstr.to_string() == "the brown");

        let _ = elt_at(&attrstr, 0, "the ");
        let _ = elt_at(&attrstr, 4, "brown");

        assert!(op2.inserts.len() == 0);
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_delete_text_whole_multiple_elements() {
        let mut attrstr = AttributedString::new();
        let   _ = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1);
        let op1 = attrstr.insert_text(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let op2 = attrstr.insert_text(10, "brown".to_owned(), &REPLICA1).unwrap();
        let op3 = attrstr.delete_text(4, 11, &REPLICA2).unwrap();

        assert!(attrstr.len() == 4);
        assert!(attrstr.to_string() == "the ");

        let _ = elt_at(&attrstr, 0, "the ");

        assert!(op3.inserts.len() == 0);
        assert!(op3.deletes.len() == 2);
        assert!(op3.deletes[0] == op1.inserts[0]);
        assert!(op3.deletes[1] == op2.inserts[0]);
    }

    #[test]
    fn test_delete_text_split_single_element() {
        let mut attrstr = AttributedString::new();
        let   _ = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1);
        let op1 = attrstr.insert_text(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let   _ = attrstr.insert_text(10, "brown".to_owned(), &REPLICA1);
        let op2 = attrstr.delete_text(5, 3, &REPLICA2).unwrap();

        assert!(attrstr.len() == 12);
        assert!(attrstr.to_string() == "the qk brown");

        let _ = elt_at(&attrstr, 0, "the ");
        let _ = elt_at(&attrstr, 4, "q");
        let _ = elt_at(&attrstr, 5, "k ");
        let _ = elt_at(&attrstr, 7, "brown");

        assert!(op2.inserts.len() == 2);
        assert!(op2.inserts[0].text == "q");
        assert!(op2.inserts[1].text == "k ");
        assert!(op2.deletes.len() == 1);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_delete_text_split_multiple_elements() {
        let mut attrstr = AttributedString::new();
        let op1 = attrstr.insert_text(0, "the ".to_owned(), &REPLICA1).unwrap();
        let   _ = attrstr.insert_text(4, "quick ".to_owned(), &REPLICA1);
        let   _ = attrstr.insert_text(10, "brown ".to_owned(), &REPLICA1);
        let   _ = attrstr.insert_text(16, "fox ".to_owned(), &REPLICA1);
        let op2 = attrstr.insert_text(20, "jumps ".to_owned(), &REPLICA1).unwrap();
        let   _ = attrstr.insert_text(26, "over".to_owned(), &REPLICA1);
        let op3 = attrstr.delete_text(2, 19, &REPLICA2).unwrap();

        assert!(attrstr.len() == 11);
        assert!(attrstr.to_string() == "thumps over");

        let _ = elt_at(&attrstr, 0, "th");
        let _ = elt_at(&attrstr, 2, "umps ");
        let _ = elt_at(&attrstr, 7, "over");

        assert!(op3.inserts.len() == 2);
        assert!(op3.inserts[0].text == "th");
        assert!(op3.inserts[1].text == "umps ");
        assert!(op3.deletes.len() == 5);
        assert!(op3.deletes[0] == op1.inserts[0]);
        assert!(op3.deletes[4] == op2.inserts[0]);
    }

    #[test]
    fn test_delete_text_invalid() {
        let mut attrstr = AttributedString::new();
        let op = attrstr.delete_text(0, 1, &REPLICA2);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_replace_text_delete_only() {
        let mut attrstr = AttributedString::new();
        let op1 = attrstr.insert_text(0, "hello world".to_owned(), &REPLICA1).unwrap();
        let op2 = attrstr.replace_text(2, 6, "".to_owned(), &REPLICA2).unwrap();

        assert!(attrstr.len() == 5);
        assert!(attrstr.to_string() == "herld");

        let _ = elt_at(&attrstr, 0, "he");
        let _ = elt_at(&attrstr, 2, "rld");

        assert!(op2.inserts.len() == 2);
        assert!(op2.deletes.len() == 1);
        assert!(op2.inserts[0].text == "he");
        assert!(op2.inserts[1].text == "rld");
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_replace_text_insert_only() {
        let mut attrstr = AttributedString::new();
        let op1 = attrstr.insert_text(0, "the fox".to_owned(), &REPLICA1).unwrap();
        let op2 = attrstr.replace_text(4, 0, "quick ".to_owned(), &REPLICA2).unwrap();

        assert!(attrstr.len() == 13);
        let e0 = elt_at(&attrstr,  0, "the ");
        let e1 = elt_at(&attrstr,  4, "quick ");
        let e2 = elt_at(&attrstr, 10, "fox");

        assert!(op2.inserts.len() == 3);
        assert!(op2.deletes.len() == 1);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);
        assert!(op2.deletes[0] == op1.inserts[0]);
    }

    #[test]
    fn test_replace_text_delete_and_insert() {
        let mut attrstr = AttributedString::new();
        let op1 = attrstr.insert_text(0, "the brown fox".to_owned(), &REPLICA1).unwrap();
        let op2 = attrstr.replace_text(4, 5, "qwik".to_owned(), &REPLICA2).unwrap();

        assert!(attrstr.len() == 12);
        let e0 = elt_at(&attrstr,  0, "the ");
        let e1 = elt_at(&attrstr,  4, "qwik");
        let e2 = elt_at(&attrstr,  8, " fox");

        assert!(op2.deletes.len() == 1);
        assert!(op2.inserts.len() == 3);
        assert!(op2.deletes[0] == op1.inserts[0]);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);
    }

    #[test]
    fn test_replace_invalid() {
        let mut attrstr = AttributedString::new();
        let   _ = attrstr.insert_text(0, "the quick brown fox".to_owned(), &REPLICA1);
        let op2 = attrstr.replace_text(4, 16, "slow green turtle".to_owned(), &REPLICA2);
        assert!(op2 == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_execute_remote_empty() {
        let mut attrstr = AttributedString::new();
        let mut op = UpdateAttributedString::default();
        let local_ops = attrstr.execute_remote(&mut op);
        assert!(local_ops.len() == 0);
    }

    #[test]
    fn test_execute_remote() {
        let mut attrstr1 = AttributedString::new();
        let mut op1 = attrstr1.insert_text(0, "the brown".to_owned(), &REPLICA1).unwrap();
        let mut op2 = attrstr1.insert_text(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let mut op3 = attrstr1.replace_text(6, 1, "a".to_owned(), &REPLICA1).unwrap();

        let mut attrstr2 = AttributedString::new();
        let lops1 = attrstr2.execute_remote(&mut op1);
        let lops2 = attrstr2.execute_remote(&mut op2);
        let lops3 = attrstr2.execute_remote(&mut op3);

        assert!(attrstr1 == attrstr2);
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
    fn test_to_string() {
        let mut attrstr = AttributedString::new();
        attrstr.insert_text(0, "the brown".to_string(), &REPLICA1).unwrap();
        attrstr.insert_text(4, "quick ".to_string(), &REPLICA1).unwrap();
        assert!(attrstr.to_string() == "the quick brown");
    }

    fn elt_at<'a>(string: &'a AttributedString, index: usize, text: &'static str) -> &'a Element {
        let (element, offset) = string.0.get_element(index).expect("Element does not exist!");
        assert!(offset == 0);
        assert!(element.text == text);
        assert!(element.len == element.text.chars().count());
        element
    }
}
