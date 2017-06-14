//! A mutable text CRDT. It can efficiently insert, remove,
//! and replace text in very large strings. TextValues
//! are indexed by unicode character.

use {Error, Replica, Tombstones};
use super::btree::BTree;
use super::element::{self, Element};
use super::{RemoteOp, LocalOp, LocalChange};
use sequence::uid::UID;
use traits::{CrdtValue, AddSiteToAll};
use char_fns::CharFns;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextValue(pub BTree);

impl TextValue {

    /// Constructs a new, empty TextValue.
    pub fn new() -> Self {
        TextValue(BTree::new())
    }

    /// Returns the number of unicode characters in the TextValue.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Inserts text at position `index`. Returns an error if the
    /// text string is empty or if the index is out-of-bounds. A
    /// successful insert returns an op that can be sent to remote
    /// sites for replication.
    pub fn insert(&mut self, index: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        if text.is_empty() { return Err(Error::Noop) }

        let (uid, offset) = {
            let (ref element, offset) = self.0.get_element(index)?;
            (element.uid.clone(), offset)
        };

        let removes = match offset {
            0 => vec![],
            _ => vec![self.0.remove(&uid).expect("Element must exist!")],
        };

        let inserts = {
            let index = index - offset;
            let prev  = self.get_prev_element(index)?;
            let (next, _) = self.0.get_element(index)?;
            if offset == 0 {
                vec![Element::between(prev, next, text, replica)]
            } else {
                let (text_pre, text_post) = removes[0].text.char_split(offset);
                let pre = Element::between(prev, next, text_pre.to_owned(), replica);
                let new = Element::between(&pre, next, text, replica);
                let post = Element::between(&new, next, text_post.to_owned(), replica);
                vec![pre, new, post]
            }
        };

        for e in &inserts { let _ = self.0.insert(e.clone()); }
        let removes = removes.into_iter().map(|e| e.uid).collect();
        Ok(RemoteOp{inserts: inserts, removes: removes})
    }

    /// Removes a text range that starts at `index` and includes `len`
    /// unicode characters. Returns an error if the range is empty or
    /// if the range upper bound is out-of-bounds. A successful remove
    /// returns an op that can be sent to remote sites for replication.
    pub fn remove(&mut self, index: usize, len: usize, replica: &Replica) -> Result<RemoteOp, Error> {
        if len == 0 { return Err(Error::Noop) }
        if index + len > self.len() { return Err(Error::OutOfBounds) }

        let (element, offset) = self.remove_at(index)?;
        let border_index = index - offset;
        let mut removed_len = element.len - offset;
        let mut removes = vec![element];

        while removed_len < len {
            let (element, _) = self.remove_at(border_index)?;
            removed_len += element.len;
            removes.push(element);
        }

        let mut inserts = vec![];
        if offset > 0 || removed_len > len {
            let prev = self.get_prev_element(border_index)?;
            let (next, _) = self.0.get_element(border_index)?;

            if offset > 0 {
                let (text, _) = removes[0].text.char_split(offset);
                inserts.push(Element::between(prev, next, text.to_owned(), replica));
            }

            if removed_len > len {
                let overremoved_elt = &removes.last().expect("Element must exist!");
                let offset = overremoved_elt.len + len - removed_len;
                let (_, text) = overremoved_elt.text.char_split(offset);
                let element = {
                    let prev = if inserts.is_empty() { prev } else { &inserts[0] };
                    Element::between(prev, next, text.to_owned(), replica)
                };
                inserts.push(element);
            }
        };

        for e in &inserts { let _ = self.0.insert(e.clone()); }
        let removes = removes.into_iter().map(|e| e.uid).collect();
        Ok(RemoteOp{inserts: inserts, removes: removes})
    }

    /// Replaces a text range that starts at `index` and includes `len`
    /// unicode characters with new text. Returns an error if the
    /// range is empty, if the range upper bound is out-of-bounds,
    /// or if the replacement has no effect. A successful replacement
    /// returns an op that can be sent to remote sites for replication.
    pub fn replace(&mut self, index: usize, len: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        if index + len > self.len() { return Err(Error::OutOfBounds) }
        if len == 0 && text.is_empty() { return Err(Error::Noop) }

        let mut op1 = self.remove(index, len, replica).unwrap_or(RemoteOp::default());
        if let Ok(op2) = self.insert(index, text, replica) { op1.merge(op2) };
        Ok(op1)
    }

    /// Executes remotely-generated ops to replicate state from other
    /// sites. Returns a Vec of LocalOps that can be used to replicate
    /// the remotely-generated op on raw string representations of the
    /// TextValue.
    pub fn execute_remote(&mut self, op: &RemoteOp) -> Option<LocalOp> {
        let mut changes = Vec::with_capacity(op.inserts.len() + op.removes.len());

        for uid in &op.removes {
            if let Some(char_index) = self.0.get_index(&uid) {
                let element = self.0.remove(&uid).expect("Element must exist!");
                changes.push(LocalChange::Remove{index: char_index, len: element.len});
            }
        }

        for element in &op.inserts {
            if let Ok(_) = self.0.insert(element.clone()) {
                let char_index = self.0.get_index(&element.uid).expect("Element must exist!");
                changes.push(LocalChange::Insert{index: char_index, text: element.text.clone()});
            }
        }

        match changes.len() {
            0 => None,
            _ => Some(LocalOp{changes: changes})
        }
    }

    pub fn merge(&mut self, other: TextValue, self_tombstones: &mut Tombstones, other_tombstones: Tombstones) {
        let removed_uids: Vec<UID> = self.0.into_iter()
            .filter(|e| other.0.get_index(&e.uid).is_none() && other_tombstones.contains_pair(e.uid.site, e.uid.counter))
            .map(|e| e.uid.clone())
            .collect();

        let new_elements: Vec<Element> = other.0.into_iter()
            .filter(|e| self.0.get_index(&e.uid).is_none() && !self_tombstones.contains_pair(e.uid.site, e.uid.counter))
            .map(|e| e.clone())
            .collect();

        for uid in removed_uids {
            let _ = self.0.remove(&uid);
        }

        for element in new_elements.into_iter() {
            let _ = self.0.insert(element);
        }

        self_tombstones.merge(other_tombstones);
    }

    fn remove_at(&mut self, index: usize) -> Result<(Element, usize), Error> {
        let (uid, offset) = {
            let (element, offset) = self.0.get_element(index)?;
            (element.uid.clone(), offset)
        };
        let element = self.0.remove(&uid).expect("Element must exist for UID!");
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

impl CrdtValue for TextValue {
    type LocalValue = String;
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;

    fn local_value(&self) -> String {
        let mut string = String::with_capacity(self.0.len());
        for element in self.0.into_iter() { string.push_str(&element.text) }
        string
    }

    fn add_site(&mut self, op: &RemoteOp, site: u32) {
        for element in &op.inserts {
            let mut element = some!(self.0.remove(&element.uid));
            element.uid.site = site;
            let _ = self.0.insert(element);
        }
    }
}

impl AddSiteToAll for TextValue {
    fn add_site_to_all(&mut self, site: u32) {
        let uids: Vec<UID> = self.0.into_iter().map(|e| e.uid.clone()).collect();
        for uid in uids {
            let mut element = self.0.remove(&uid).expect("Element must exist");
            element.uid.site = site;
            let _ = self.0.insert(element);
        }
    }

    fn validate_site_for_all(&self, site: u32) -> Result<(), Error> {
        for element in self.0.into_iter() {
            try_assert!(element.uid.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLICA1: Replica = Replica{site: 5, counter: 1023};
    const REPLICA2: Replica = Replica{site: 8, counter: 16};

    #[test]
    fn test_new() {
        let text = TextValue::new();
        assert!(text.len() == 0);
        assert!(text.local_value() == "");
    }

    #[test]
    fn test_insert_empty_string() {
        let mut text = TextValue::new();
        let op = text.insert(0, "".to_owned(), &REPLICA1);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_insert_when_empty() {
        let mut text = TextValue::new();
        let op = text.insert(0, "quick".to_owned(), &REPLICA1).unwrap();
        let element = elt_at(&text, 0, "quick");

        assert!(text.len() == 5);
        assert!(text.local_value() == "quick");
        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == element.uid);
        assert!(op.inserts[0].text == element.text);
        assert!(op.removes.is_empty());
    }

    #[test]
    fn test_insert_before_index() {
        let mut text = TextValue::new();
        let  _ = text.insert(0, "the ".to_owned(), &REPLICA1);
        let  _ = text.insert(4, "brown".to_owned(), &REPLICA1);
        let op = text.insert(4, "quick ".to_owned(), &REPLICA2).unwrap();

        assert!(text.len() == 15);
        assert!(text.local_value() == "the quick brown");

        let _  = elt_at(&text,  0, "the ");
        let e1 = elt_at(&text,  4, "quick ");
        let _  = elt_at(&text, 10, "brown");

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == e1.uid);
        assert!(op.inserts[0].text == e1.text);
        assert!(op.removes.len() == 0);
    }

    #[test]
    fn test_insert_in_index() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "the  ".to_owned(), &REPLICA1).unwrap();
        let   _ = text.insert(5, "brown".to_owned(), &REPLICA1);
        let op2 = text.insert(4, "quick".to_owned(), &REPLICA2).unwrap();

        assert!(text.len() == 15);
        assert!(text.local_value() == "the quick brown");

        let e0 = elt_at(&text,  0, "the ");
        let e1 = elt_at(&text,  4, "quick");
        let e2 = elt_at(&text,  9, " ");
        let _  = elt_at(&text, 10, "brown");

        assert!(op2.inserts.len() == 3);
        assert!(op2.inserts[0].uid == e0.uid);
        assert!(op2.inserts[1].uid == e1.uid);
        assert!(op2.inserts[2].uid == e2.uid);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);

        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_insert_invalid() {
        let mut text = TextValue::new();
        let op = text.insert(1, "quick".to_owned(), &REPLICA1);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_remove_zero_text() {
        let mut text = TextValue::new();
        let  _ = text.insert(0, "the ".to_owned(), &REPLICA1);
        let op = text.remove(1, 0, &REPLICA2);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_remove_whole_single_element() {
        let mut text = TextValue::new();
        let   _ = text.insert(0, "the ".to_owned(), &REPLICA1);
        let op1 = text.insert(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let   _ = text.insert(10, "brown".to_owned(), &REPLICA1);
        let op2 = text.remove(4, 6, &REPLICA2).unwrap();

        assert!(text.len() == 9);
        assert!(text.local_value() == "the brown");

        let _ = elt_at(&text, 0, "the ");
        let _ = elt_at(&text, 4, "brown");

        assert!(op2.inserts.len() == 0);
        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_remove_whole_multiple_elements() {
        let mut text = TextValue::new();
        let   _ = text.insert(0, "the ".to_owned(), &REPLICA1);
        let op1 = text.insert(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let op2 = text.insert(10, "brown".to_owned(), &REPLICA1).unwrap();
        let op3 = text.remove(4, 11, &REPLICA2).unwrap();

        assert!(text.len() == 4);
        assert!(text.local_value() == "the ");

        let _ = elt_at(&text, 0, "the ");

        assert!(op3.inserts.len() == 0);
        assert!(op3.removes.len() == 2);
        assert!(op3.removes[0] == op1.inserts[0].uid);
        assert!(op3.removes[1] == op2.inserts[0].uid);
    }

    #[test]
    fn test_remove_split_single_element() {
        let mut text = TextValue::new();
        let   _ = text.insert(0, "the ".to_owned(), &REPLICA1);
        let op1 = text.insert(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let   _ = text.insert(10, "brown".to_owned(), &REPLICA1);
        let op2 = text.remove(5, 3, &REPLICA2).unwrap();

        assert!(text.len() == 12);
        assert!(text.local_value() == "the qk brown");

        let _ = elt_at(&text, 0, "the ");
        let _ = elt_at(&text, 4, "q");
        let _ = elt_at(&text, 5, "k ");
        let _ = elt_at(&text, 7, "brown");

        assert!(op2.inserts.len() == 2);
        assert!(op2.inserts[0].text == "q");
        assert!(op2.inserts[1].text == "k ");
        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_remove_split_multiple_elements() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "the ".to_owned(), &REPLICA1).unwrap();
        let   _ = text.insert(4, "quick ".to_owned(), &REPLICA1);
        let   _ = text.insert(10, "brown ".to_owned(), &REPLICA1);
        let   _ = text.insert(16, "fox ".to_owned(), &REPLICA1);
        let op2 = text.insert(20, "jumps ".to_owned(), &REPLICA1).unwrap();
        let   _ = text.insert(26, "over".to_owned(), &REPLICA1);
        let op3 = text.remove(2, 19, &REPLICA2).unwrap();

        assert!(text.len() == 11);
        assert!(text.local_value() == "thumps over");

        let _ = elt_at(&text, 0, "th");
        let _ = elt_at(&text, 2, "umps ");
        let _ = elt_at(&text, 7, "over");

        assert!(op3.inserts.len() == 2);
        assert!(op3.inserts[0].text == "th");
        assert!(op3.inserts[1].text == "umps ");
        assert!(op3.removes.len() == 5);
        assert!(op3.removes[0] == op1.inserts[0].uid);
        assert!(op3.removes[4] == op2.inserts[0].uid);
    }

    #[test]
    fn test_remove_invalid() {
        let mut text = TextValue::new();
        let op = text.remove(0, 1, &REPLICA2);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_replace_remove_only() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "hello world".to_owned(), &REPLICA1).unwrap();
        let op2 = text.replace(2, 6, "".to_owned(), &REPLICA2).unwrap();

        assert!(text.len() == 5);
        assert!(text.local_value() == "herld");

        let _ = elt_at(&text, 0, "he");
        let _ = elt_at(&text, 2, "rld");

        assert!(op2.inserts.len() == 2);
        assert!(op2.removes.len() == 1);
        assert!(op2.inserts[0].text == "he");
        assert!(op2.inserts[1].text == "rld");
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_replace_insert_only() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "the fox".to_owned(), &REPLICA1).unwrap();
        let op2 = text.replace(4, 0, "quick ".to_owned(), &REPLICA2).unwrap();

        assert!(text.len() == 13);
        let e0 = elt_at(&text,  0, "the ");
        let e1 = elt_at(&text,  4, "quick ");
        let e2 = elt_at(&text, 10, "fox");

        assert!(op2.inserts.len() == 3);
        assert!(op2.removes.len() == 1);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_replace_remove_and_insert() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "the brown fox".to_owned(), &REPLICA1).unwrap();
        let op2 = text.replace(4, 5, "qwik".to_owned(), &REPLICA2).unwrap();

        assert!(text.len() == 12);
        let e0 = elt_at(&text,  0, "the ");
        let e1 = elt_at(&text,  4, "qwik");
        let e2 = elt_at(&text,  8, " fox");

        assert!(op2.removes.len() == 1);
        assert!(op2.inserts.len() == 3);
        assert!(op2.removes[0] == op1.inserts[0].uid);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.inserts[1].text == e1.text);
        assert!(op2.inserts[2].text == e2.text);
    }

    #[test]
    fn test_replace_invalid() {
        let mut text = TextValue::new();
        let   _ = text.insert(0, "the quick brown fox".to_owned(), &REPLICA1);
        let op2 = text.replace(4, 16, "slow green turtle".to_owned(), &REPLICA2);
        assert!(op2 == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_execute_remote_empty() {
        let mut text = TextValue::new();
        let mut op = RemoteOp{inserts: vec![], removes: vec![]};
        assert!(text.execute_remote(&mut op).is_none());
    }

    #[test]
    fn test_execute_remote() {
        let mut text1 = TextValue::new();
        let op1 = text1.insert(0, "the brown".to_owned(), &REPLICA1).unwrap();
        let op2 = text1.insert(4, "quick ".to_owned(), &REPLICA1).unwrap();
        let op3 = text1.replace(6, 1, "a".to_owned(), &REPLICA1).unwrap();

        let mut text2 = TextValue::new();
        let changes1 = text2.execute_remote(&op1).unwrap().changes;
        let changes2 = text2.execute_remote(&op2).unwrap().changes;
        let changes3 = text2.execute_remote(&op3).unwrap().changes;

        assert!(text1 == text2);
        assert!(changes1.len() == 1);
        assert!(changes2.len() == 4);
        assert!(changes3.len() == 4);

        assert_insert(&changes1[0], 0, "the brown");
        assert_matches!(changes2[0], LocalChange::Remove{index: 0, len: 9});
        assert_insert(&changes2[1], 0, "the ");
        assert_insert(&changes2[2], 4, "quick ");
        assert_insert(&changes2[3], 10, "brown");
        assert_matches!(changes3[0], LocalChange::Remove{index: 4, len: 6});
        assert_insert(&changes3[1], 4, "qu");
        assert_insert(&changes3[2], 6, "a");
        assert_insert(&changes3[3], 7, "ck ");
    }

    #[test]
    fn test_ignore_duplicate_inserts_and_removes() {
        let mut text1 = TextValue::new();
        let mut text2 = TextValue::new();

        let op = text1.insert(0, "hi".to_owned(), &REPLICA1).unwrap();
        assert!(text2.execute_remote(&op).is_some());
        assert!(text2.execute_remote(&op).is_none());
        assert!(text1 == text2);
    }

    #[test]
    fn test_add_site() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "a".to_owned(), &Replica::new(0, 1)).unwrap();
        let op2 = text.insert(1, "b".to_owned(), &Replica::new(0, 2)).unwrap();

        text.add_site(&op1, 4);
        text.add_site(&op2, 8);

        let (e1, _) = text.0.get_element(0).unwrap();
        assert!(e1.uid.site == 4);
        assert!(e1.uid.counter == 1);

        let (e2, _) = text.0.get_element(1).unwrap();
        assert!(e2.uid.site == 8);
        assert!(e2.uid.counter == 2);
    }

    fn elt_at<'a>(string: &'a TextValue, index: usize, text: &'static str) -> &'a Element {
        let (element, offset) = string.0.get_element(index).expect("Element does not exist!");
        assert!(offset == 0);
        assert!(element.text == text);
        assert!(element.len == element.text.char_len());
        element
    }

    fn assert_insert(local_change: &LocalChange, index: usize, text: &'static str) {
        match *local_change {
            LocalChange::Insert{index: i, text: ref t} => {
                assert!(i == index);
                assert!(t == text);
            },
            _ => {
                assert!(false, "Not an insert!");
            }
        }
    }
}
