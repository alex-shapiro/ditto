//! A mutable text CRDT. It can efficiently insert, remove,
//! and replace text in very large strings. TextValues
//! are indexed by unicode character.

use {Error, Replica, Tombstones};
use order_statistic_tree::Tree;
use super::element::{self, Element};
use super::text_edit::TextEdit;
use super::{RemoteOp, LocalOp, LocalChange};
use sequence::uid::UID;
use traits::CrdtValue;
use char_fns::CharFns;
use serde::{Serialize, Serializer, Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub struct TextValue(pub Tree<Element>, pub Option<TextEdit>);

impl TextValue {

    /// Constructs a new, empty TextValue.
    pub fn new() -> Self {
        TextValue(Tree::new(), None)
    }

    /// Constructs a new TextValue from a str and a replica.
    /// Each paragraph in the str is split into a separate element.
    pub fn from_str(string: &str, replica: &Replica) -> Self {
        let mut text = TextValue::new();
        if string.is_empty() { return text }

        let mut iter = string.rsplit('\n');
        if !string.ends_with('\n') {
            let last_substring = iter.next().unwrap().to_owned();
            let _ = text.do_insert(0, last_substring, replica).unwrap();
        }

        for substring in iter {
            let substring = format!("{}\n", substring);
            let _ = text.do_insert(0, substring, replica).unwrap();
        }

        text
    }


    /// Returns the number of unicode characters in the TextValue.
    pub fn len(&self) -> usize {
        self.0.len()
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
        if offset == 0 && merged_edit.len == 0 {
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
                changes.push(LocalChange{idx: char_index, len: element.len, text: "".into()});
            }
        }

        for element in &op.inserts {
            if let Ok(_) = self.0.insert(element.clone()) {
                let char_index = self.0.get_idx(&element.uid).expect("Element must exist I!");
                changes.push(LocalChange{idx: char_index, len: 0, text: element.text.clone()});
            }
        }

        self.shift_merged_edit(&changes);

        match changes.len() {
            0 => None,
            _ => Some(LocalOp(changes))
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
            edit.shift_or_destroy(change.idx, change.len, &change.text)
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
        for e in &op.inserts {
            if let Some(mut element) = self.0.remove(&e.uid) {
                element.uid.site = site;
                self.0.insert(element).unwrap();
            }
        }
    }

    fn add_site_to_all(&mut self, site: u32) {
        let old_tree = ::std::mem::replace(&mut self.0, Tree::new());
        for mut element in old_tree {
            element.uid.site = site;
            self.0.insert(element).unwrap();
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        for element in self.0.iter() {
            try_assert!(element.uid.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }

    fn merge(&mut self, other: TextValue, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
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

        self.1 = None;
    }
}

// rust-msgpack encodes NewType values nontransparently,
// as single-element arrays. The TextValue struct used to be
// a NewType, and I am recreating that encapsulation to
// maintain backwards compatibility.
#[derive(Serialize, Deserialize)]
struct TreeNewType<T>(T);

impl Serialize for TextValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        TreeNewType(&self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TextValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let newtype: TreeNewType<Tree<Element>> = TreeNewType::deserialize(deserializer)?;
        Ok(TextValue(newtype.0, None))
    }
}

impl PartialEq for TextValue {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
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
        let op = text.replace(0, 0, "", &REPLICA1);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_insert_when_empty() {
        let mut text = TextValue::new();
        let op = text.replace(0, 0, "quick", &REPLICA1).unwrap();
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
        let  _ = text.replace(0, 0, "the\n", &REPLICA1).unwrap();
        let  _ = text.replace(4, 0, "brown", &REPLICA1).unwrap();
        let op = text.replace(4, 0, "quick ", &REPLICA2).unwrap();

        assert!(text.len() == 15);
        assert!(text.local_value() == "the\nquick brown");

        let _  = elt_at(&text,  0, "the\n");
        let e1 = elt_at(&text,  4, "quick brown");

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].uid == e1.uid);
        assert!(op.inserts[0].text == e1.text);
        assert!(op.removes.len() == 1);
    }

    #[test]
    fn test_insert_in_index() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "the\n\n", &REPLICA1).unwrap();
        let   _ = text.replace(5, 0, "brown", &REPLICA1);
        let op2 = text.replace(4, 0, "quick", &REPLICA2).unwrap();

        assert!(text.len() == 15);
        assert!(text.local_value() == "the\nquick\nbrown");

        let e0 = elt_at(&text,  0, "the\n");
        let e1 = elt_at(&text,  4, "quick");
        let e2 = elt_at(&text,  9, "\n");
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
        let op = text.replace(1, 0, "quick", &REPLICA1);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_remove_zero_text() {
        let mut text = TextValue::new();
        let  _ = text.replace(0, 0, "the ", &REPLICA1);
        let op = text.replace(1, 0, "", &REPLICA2);
        assert!(op == Err(Error::Noop));
    }

    #[test]
    fn test_remove_whole_single_element() {
        let mut text = TextValue::new();
        let   _ = text.replace(0, 0, "the\n", &REPLICA1).unwrap();
        let op1 = text.replace(4, 0, "quick\n", &REPLICA1).unwrap();
        let   _ = text.replace(10, 0, "brown\n", &REPLICA1).unwrap();
        let op2 = text.replace(4, 6, "", &REPLICA2).unwrap();

        assert!(text.len() == 10);
        assert!(text.local_value() == "the\nbrown\n");

        let _ = elt_at(&text, 0, "the\n");
        let _ = elt_at(&text, 4, "brown\n");

        assert!(op2.inserts.len() == 0);
        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_remove_whole_multiple_elements() {
        let mut text = TextValue::new();
        let   _ = text.replace(0, 0, "the\n", &REPLICA1);
        let op1 = text.replace(4, 0, "quick\n", &REPLICA1).unwrap();
        let op2 = text.replace(10, 0, "brown\n", &REPLICA1).unwrap();
        let op3 = text.replace(4, 12, "", &REPLICA2).unwrap();

        assert!(text.len() == 4);
        assert!(text.local_value() == "the\n");

        let _ = elt_at(&text, 0, "the\n");

        assert!(op3.inserts.len() == 0);
        assert!(op3.removes.len() == 2);
        assert!(op3.removes[0] == op1.inserts[0].uid);
        assert!(op3.removes[1] == op2.inserts[0].uid);
    }

    #[test]
    fn test_remove_split_single_element() {
        let mut text = TextValue::new();
        let   _ = text.replace(0, 0, "the ", &REPLICA1).unwrap();
        let   _ = text.replace(4, 0, "quick ", &REPLICA1).unwrap();
        let op1 = text.replace(10, 0, "brown", &REPLICA1).unwrap();
        let op2 = text.replace(5, 3, "", &REPLICA2).unwrap();

        assert!(text.len() == 12);
        assert!(text.local_value() == "the qk brown");

        let _ = elt_at(&text, 0, "the qk brown");

        assert!(op2.inserts.len() == 1);
        assert!(op2.inserts[0].text == "the qk brown");
        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_remove_split_multiple_elements() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "the\n", &REPLICA1).unwrap();
        let   _ = text.replace(4, 0, "quick\n", &REPLICA1);
        let   _ = text.replace(10, 0, "brown\n", &REPLICA1);
        let   _ = text.replace(16, 0, "fox\n", &REPLICA1);
        let op2 = text.replace(20, 0, "jumps\n", &REPLICA1).unwrap();
        let   _ = text.replace(26, 0, "over", &REPLICA1);
        let op3 = text.replace(2, 19, "", &REPLICA2).unwrap();

        assert!(text.len() == 11);
        assert!(text.local_value() == "thumps\nover");

        let _ = elt_at(&text, 0, "th");
        let _ = elt_at(&text, 2, "umps\n");
        let _ = elt_at(&text, 7, "over");

        assert!(op3.inserts.len() == 2);
        assert!(op3.inserts[0].text == "th");
        assert!(op3.inserts[1].text == "umps\n");
        assert!(op3.removes.len() == 5);
        assert!(op3.removes[0] == op1.inserts[0].uid);
        assert!(op3.removes[4] == op2.inserts[0].uid);
    }

    #[test]
    fn test_remove_invalid() {
        let mut text = TextValue::new();
        let op = text.replace(0, 1, "", &REPLICA2);
        assert!(op == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_replace_remove_only() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "hello world", &REPLICA1).unwrap();
        let op2 = text.replace(2, 6, "", &REPLICA2).unwrap();

        assert!(text.len() == 5);
        assert!(text.local_value() == "herld");

        let _ = elt_at(&text, 0, "herld");

        assert!(op2.inserts.len() == 1);
        assert!(op2.removes.len() == 1);
        assert!(op2.inserts[0].text == "herld");
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_replace_insert_only() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "the fox", &REPLICA1).unwrap();
        let op2 = text.replace(4, 0, "quick ", &REPLICA2).unwrap();

        assert!(text.len() == 13);
        let e0 = elt_at(&text,  0, "the quick fox");

        assert!(op2.inserts.len() == 1);
        assert!(op2.removes.len() == 1);
        assert!(op2.inserts[0].text == e0.text);
        assert!(op2.removes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_replace_remove_and_insert() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "the brown fox", &REPLICA1).unwrap();
        let op2 = text.replace(4, 5, "qwik", &REPLICA2).unwrap();

        assert!(text.len() == 12);
        let e0 = elt_at(&text,  0, "the qwik fox");

        assert!(op2.inserts.len() == 1);
        assert!(op2.removes.len() == 1);
        assert!(op2.removes[0] == op1.inserts[0].uid);
        assert!(op2.inserts[0].text == e0.text);
    }

    #[test]
    fn test_replace_invalid() {
        let mut text = TextValue::new();
        let   _ = text.replace(0, 0, "the quick brown fox", &REPLICA1);
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
        let op1 = text1.replace(0, 0, "the brown", &REPLICA1).unwrap();
        let op2 = text1.replace(4, 0, "quick ", &REPLICA1).unwrap();
        let op3 = text1.replace(6, 1, "a", &REPLICA1).unwrap();

        let mut text2 = TextValue::new();
        let changes1 = text2.execute_remote(&op1).unwrap().0;
        let changes2 = text2.execute_remote(&op2).unwrap().0;
        let changes3 = text2.execute_remote(&op3).unwrap().0;

        assert!(text1 == text2);
        assert!(changes1.len() == 1);
        assert!(changes2.len() == 2);
        assert!(changes3.len() == 2);

        assert_eq!(changes1[0], LocalChange{idx: 0, len: 0, text: "the brown".into()});
        assert_eq!(changes2[0], LocalChange{idx: 0, len: 9, text: "".into()});
        assert_eq!(changes2[1], LocalChange{idx: 0, len: 0, text: "the quick brown".into()});
        assert_eq!(changes3[0], LocalChange{idx: 0, len: 15, text: "".into()});
        assert_eq!(changes3[1], LocalChange{idx: 0, len: 0, text: "the quack brown".into()});
    }

    #[test]
    fn test_ignore_duplicate_inserts_and_removes() {
        let mut text1 = TextValue::new();
        let mut text2 = TextValue::new();

        let op = text1.replace(0, 0, "hi", &REPLICA1).unwrap();
        assert!(text2.execute_remote(&op).is_some());
        assert!(text2.execute_remote(&op).is_none());
        assert!(text1 == text2);
    }

    #[test]
    fn test_add_site() {
        let mut text = TextValue::new();
        let op1 = text.replace(0, 0, "a\n", &Replica::new(0, 1)).unwrap();
        let op2 = text.replace(2, 0, "b", &Replica::new(0, 2)).unwrap();

        text.add_site(&op1, 4);
        text.add_site(&op2, 8);

        let (e1, _) = text.get_element(0).unwrap();
        assert!(e1.uid.site == 4);
        assert!(e1.uid.counter == 1);

        let (e2, _) = text.get_element(2).unwrap();
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
}
