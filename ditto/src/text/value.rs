//! A mutable text CRDT. It can efficiently insert, remove,
//! and replace text in very large strings. TextValues
//! are indexed by unicode character.

use {Error, Replica, Tombstones};
use order_statistic_tree::Tree;
use super::element::{self, Element};
use super::text_edit::TextEdit;
use super::{RemoteOp, LocalOp, LocalChange};
use sequence::uid::UID;
use traits::{CrdtValue, AddSiteToAll};
use char_fns::CharFns;
use serde::{Serialize, Serializer, Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq)]
pub struct TextValue(pub Tree<Element>, pub Option<TextEdit>);

impl TextValue {

    /// Constructs a new, empty TextValue.
    pub fn new() -> Self {
        TextValue(Tree::new(), None)
    }

    /// Returns the number of unicode characters in the TextValue.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn insert(&mut self, idx: usize, text: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        self.replace(idx, 0, text, replica)
    }

    pub fn remove(&mut self, idx: usize, len: usize, replica: &Replica) -> Result<RemoteOp, Error> {
        self.replace(idx, len, "", replica)
    }

    /// Replaces a text range that starts at `idx` and includes `len`
    /// unicode characters with new text. Returns an error if the
    /// range is out-of-bounds or if the replacement has no effect.
    /// A successful replacement returns an op that can be sent to
    /// remote sites for replication.
    pub fn replace(&mut self, idx: usize, len: usize, text: &str, replica: &Replica) -> Result<RemoteOp, Error> {
        if idx + len > self.len() { return Err(Error::OutOfBounds) }
        if len == 0 && text.is_empty() { return Err(Error::Noop) }

        let merged_edit = self.gen_merged_edit(idx, len, text);

        let offset = self.get_element(idx)?.1;
        if offset == 0 && len == 0 {
            self.do_insert(merged_edit.idx, merged_edit.text, replica)
        } else {
            self.do_replace(merged_edit.idx, merged_edit.len, merged_edit.text, replica)
        }
    }

    fn do_insert(&mut self, idx: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        let element = {
            let prev = self.get_prev_element(idx)?;
            let next = self.get_element(idx)?.0;
            Element::between(prev, next, text, replica)
        };

        self.0.insert(element.clone()).unwrap();
        Ok(RemoteOp{inserts: vec![element], removes: vec![]})
    }

    fn do_replace(&mut self, idx: usize, len: usize, text: String, replica: &Replica) -> Result<RemoteOp, Error> {
        let (element, offset) = self.remove_at(idx)?;
        let border_idx = idx - offset;
        let mut removed_len = element.len - offset;
        let mut removes = vec![element];
        let mut inserts = vec![];

        while removed_len < len {
            let (element, _) = self.remove_at(border_idx)?;
            removed_len += element.len;
            removes.push(element);
        }

        if offset > 0 || !text.is_empty() || removed_len > len {
            let prev = self.get_prev_element(border_idx)?;
            let (next, _) = self.get_element(border_idx)?;

            if offset > 0 {
                let (text, _) = removes[0].text.char_split(offset);
                inserts.push(Element::between(prev, next, text.into(), replica));
            }

            if !text.is_empty() {
                let element = Element::between(inserts.last().unwrap_or(prev), next, text, replica);
                inserts.push(element);
            }

            if removed_len > len {
                let old_elt = &removes.last().unwrap();
                let offset = old_elt.len + len - removed_len;
                let (_, text) = old_elt.text.char_split(offset);
                let element = Element::between(inserts.last().unwrap_or(prev), next, text.into(), replica);
                inserts.push(element);
            }
        }

        for element in &inserts {
            self.0.insert(element.clone()).unwrap();
        }

        let removes = removes.into_iter().map(|e| e.uid).collect();
        Ok(RemoteOp{inserts, removes})
    }

    /// Executes remotely-generated ops to replicate state from other
    /// sites. Returns a Vec of LocalOps that can be used to replicate
    /// the remotely-generated op on raw string representations of the
    /// TextValue.
    pub fn execute_remote(&mut self, op: &RemoteOp) -> Option<LocalOp> {
        let mut changes = Vec::with_capacity(op.inserts.len() + op.removes.len());

        for uid in &op.removes {
            if let Some(char_index) = self.0.get_idx(&uid) {
                let element = self.0.remove(&uid).expect("Element must exist H!");
                changes.push(LocalChange::Remove{index: char_index, len: element.len});
            }
        }

        for element in &op.inserts {
            if let Ok(_) = self.0.insert(element.clone()) {
                let char_index = self.0.get_idx(&element.uid).expect("Element must exist I!");
                changes.push(LocalChange::Insert{index: char_index, text: element.text.clone()});
            }
        }

        self.shift_merged_edit(&changes);

        match changes.len() {
            0 => None,
            _ => Some(LocalOp{changes: changes})
        }
    }

    pub fn merge(&mut self, other: TextValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        let removed_uids: Vec<UID> = self.0.iter()
            .filter(|e| other.0.get_idx(&e.uid).is_none() && other_tombstones.contains_pair(e.uid.site, e.uid.counter))
            .map(|e| e.uid.clone())
            .collect();

        let new_elements: Vec<Element> = other.0.into_iter()
            .filter(|e| self.0.get_idx(&e.uid).is_none() && !self_tombstones.contains_pair(e.uid.site, e.uid.counter))
            .map(|e| e.clone())
            .collect();

        for uid in removed_uids {
            let _ = self.0.remove(&uid);
        }

        for element in new_elements {
            let _ = self.0.insert(element);
        }
    }

    fn remove_at(&mut self, index: usize) -> Result<(Element, usize), Error> {
        let (uid, offset) = {
            let (element, offset) = self.0.get_elt(index)?;
            (element.uid.clone(), offset)
        };
        let element = self.0.remove(&uid).expect("Element must exist for UID!");
        Ok((element, offset))
    }

    fn get_prev_element(&self, index: usize) -> Result<&Element, Error> {
        if index == 0 {
            Ok(&*element::START)
        } else {
            Ok(self.0.get_elt(index-1)?.0)
        }
    }

    fn get_element(&self, index: usize) -> Result<(&Element, usize), Error> {
        if index == self.len() {
            Ok((&*element::END, 0))
        } else {
            Ok(self.0.get_elt(index)?)
        }
    }

    fn gen_merged_edit(&mut self, idx: usize, len: usize, text: &str) -> TextEdit {
        if let Some(ref mut edit) = self.1 {
            edit.merge_or_replace(idx, len, text)
        } else {
            let edit = TextEdit{idx, len, text: text.into()};
            self.1 = Some(edit.clone());
            edit
        }
    }

    fn shift_merged_edit(&mut self, changes: &[LocalChange]) {
        let edit = some!(self.1.take());
        let edit = changes.iter().fold(Some(edit), |edit, change| {
            let edit = try_opt!(edit);
            match *change {
                LocalChange::Insert{index, ref text} => edit.shift_or_destroy(index, 0, text),
                LocalChange::Remove{index, len} => edit.shift_or_destroy(index, len, ""),
            }
        });

        self.1 = edit;
    }
}

impl CrdtValue for TextValue {
    type LocalValue = String;
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;

    fn local_value(&self) -> String {
        let mut string = String::with_capacity(self.0.len());
        for element in self.0.iter() { string.push_str(&element.text) }
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
        let old_tree = ::std::mem::replace(&mut self.0, Tree::new());
        for mut element in old_tree {
            element.uid.site = site;
            let _ = self.0.insert(element);
        }
    }

    fn validate_site_for_all(&self, site: u32) -> Result<(), Error> {
        for element in self.0.iter() {
            try_assert!(element.uid.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }
}

impl Serialize for TextValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let tree = Tree::<Element>::deserialize(deserializer)?;
        Ok(TextValue(tree, None))
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
        let op = text.insert(0, "", &REPLICA1);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_insert_when_empty() {
        let mut text = TextValue::new();
        let op = text.insert(0, "quick", &REPLICA1).unwrap();
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
        let  _ = text.insert(0, "the ", &REPLICA1);
        let  _ = text.insert(4, "brown", &REPLICA1);
        let op = text.insert(4, "quick ", &REPLICA2).unwrap();

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
        let op1 = text.insert(0, "the  ", &REPLICA1).unwrap();
        let   _ = text.insert(5, "brown", &REPLICA1);
        let op2 = text.insert(4, "quick", &REPLICA2).unwrap();

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
        let op = text.insert(1, "quick", &REPLICA1);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_remove_zero_text() {
        let mut text = TextValue::new();
        let  _ = text.insert(0, "the ", &REPLICA1);
        let op = text.remove(1, 0, &REPLICA2);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_remove_whole_single_element() {
        let mut text = TextValue::new();
        let   _ = text.insert(0, "the ", &REPLICA1);
        let op1 = text.insert(4, "quick ", &REPLICA1).unwrap();
        let   _ = text.insert(10, "brown", &REPLICA1);
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
        let   _ = text.insert(0, "the ", &REPLICA1);
        let op1 = text.insert(4, "quick ", &REPLICA1).unwrap();
        let op2 = text.insert(10, "brown", &REPLICA1).unwrap();
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
        let   _ = text.insert(0, "the ", &REPLICA1);
        let op1 = text.insert(4, "quick ", &REPLICA1).unwrap();
        let   _ = text.insert(10, "brown", &REPLICA1);
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
        let op1 = text.insert(0, "the ", &REPLICA1).unwrap();
        let   _ = text.insert(4, "quick ", &REPLICA1);
        let   _ = text.insert(10, "brown ", &REPLICA1);
        let   _ = text.insert(16, "fox ", &REPLICA1);
        let op2 = text.insert(20, "jumps ", &REPLICA1).unwrap();
        let   _ = text.insert(26, "over", &REPLICA1);
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
        let op1 = text.insert(0, "hello world", &REPLICA1).unwrap();
        let op2 = text.replace(2, 6, "", &REPLICA2).unwrap();

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
        let op1 = text.insert(0, "the fox", &REPLICA1).unwrap();
        let op2 = text.replace(4, 0, "quick ", &REPLICA2).unwrap();

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
        let op1 = text.insert(0, "the brown fox", &REPLICA1).unwrap();
        let op2 = text.replace(4, 5, "qwik", &REPLICA2).unwrap();

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
        let   _ = text.insert(0, "the quick brown fox", &REPLICA1);
        let op2 = text.replace(4, 16, "slow green turtle", &REPLICA2);
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
        let op1 = text1.insert(0, "the brown", &REPLICA1).unwrap();
        let op2 = text1.insert(4, "quick ", &REPLICA1).unwrap();
        let op3 = text1.replace(6, 1, "a", &REPLICA1).unwrap();

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

        let op = text1.insert(0, "hi", &REPLICA1).unwrap();
        assert!(text2.execute_remote(&op).is_some());
        assert!(text2.execute_remote(&op).is_none());
        assert!(text1 == text2);
    }

    #[test]
    fn test_add_site() {
        let mut text = TextValue::new();
        let op1 = text.insert(0, "a", &Replica::new(0, 1)).unwrap();
        let op2 = text.insert(1, "b", &Replica::new(0, 2)).unwrap();

        text.add_site(&op1, 4);
        text.add_site(&op2, 8);

        let (e1, _) = text.get_element(0).unwrap();
        assert!(e1.uid.site == 4);
        assert!(e1.uid.counter == 1);

        let (e2, _) = text.get_element(1).unwrap();
        assert!(e2.uid.site == 8);
        assert!(e2.uid.counter == 2);
    }

    fn elt_at<'a>(string: &'a TextValue, index: usize, text: &'static str) -> &'a Element {
        let (element, offset) = string.get_element(index).expect("Element does not exist!");
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
