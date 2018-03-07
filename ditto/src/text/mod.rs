//! A CRDT that stores mutable text

mod text_edit;

use self::text_edit::TextEdit;
use dot::{Dot, Summary, SiteId};
use Error;
use order_statistic_tree::{self, Tree};
use sequence::uid::UID;
use std::borrow::Cow;
use std::cmp::Ordering;

pub type LocalOp = TextEdit;

lazy_static! {
    pub static ref START_ELEMENT: Element = Element{uid: UID::min(), text: String::new()};
    pub static ref END_ELEMENT: Element = Element{uid: UID::max(), text: String::new()};
}

/// Text is a `String`-like UTF-encoded growable string.
/// It contains a number of optimizations that improve
/// replacement and op execution performance on large strings.
///
/// Internally, Text is based on LSEQ. It allows op-based replication
/// via [`execute_op`](#method.execute_op) and state-based replication
/// via [`merge`](#method.merge). State-based replication allows
/// out-of-order delivery but op-based replication does not.
///
/// Text has the following performance characteristics:
///
/// * [`replace`](#method.replace): *O(log N)*
/// * [`execute_op`](#method.execute_op): *O(log N)*
/// * [`merge`](#method.merge): *O(N1 + N2 + S1 + S2)*, where *N1* and
///   *N2* are the number of values in each Text being merged,
///   and *S1* and *S2* are the number of sites that have edited
///   each Text being merged.
///
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    inner:      Inner,
    site_id:    SiteId,
    summary:    Summary,
    cached_ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextState<'a> {
    #[serde(rename = "i")]
    inner: Cow<'a, Inner>,
    #[serde(rename = "s")]
    summary: Cow<'a, Summary>,
}

#[derive(Debug)]
pub struct Inner(pub Tree<Element>, pub Option<TextEdit>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    #[serde(rename = "u")]
    pub uid: UID,
    #[serde(rename = "t")]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    #[serde(rename = "i")]
    inserted_elements: Vec<Element>,
    #[serde(rename = "r")]
    removed_uids: Vec<UID>,
}

impl Text {
    /// Constructs and returns a new Text CRDT with site id 1.
    pub fn new() -> Self {
        let inner   = Inner::new();
        let summary = Summary::default();
        let site_id = 1;
        Text{inner, summary, site_id, cached_ops: vec![]}
    }

    /// Constructs and returns a new Text CRDT from a string.
    /// The Text has site id 1.
    pub fn from_str(string: &str) -> Self {
        let mut text = Text::new();
        let _ = text.replace(0, 0, string).unwrap();
        text
    }

    /// Returns the number of unicode characters in the text.
    pub fn len(&self) -> usize {
        self.inner.0.len()
    }

    /// Returns true if the Text CRDT has a length of 0.
    /// Returns false otherwise.
    pub fn is_empty(&self) -> bool {
        self.inner.0.len() == 0
    }

    /// Replaces the text in the range [idx..<idx+len] with new text.
    /// Panics if the start or stop idx is larger than the `Text`'s
    /// length, or if it does not lie on a `char` boundary. If the
    /// Text does not have a site id, it caches the op and returns an
    /// `AwaitingSiteId` error.
    pub fn replace(&mut self, idx: usize, len: usize, text: &str) -> Option<Result<Op, Error>> {
        let dot = self.summary.get_dot(self.site_id);
        let op = self.inner.replace(idx, len, text, dot)?;
        Some(self.after_op(op))
    }

    crdt_impl2! {
        Text,
        TextState,
        TextState<'static>,
        TextState,
        Inner,
        Op,
        Vec<LocalOp>,
        String,
    }
}

impl<'a> From<&'a str> for Text {
    fn from(local_value: &'a str) -> Self {
        Text::from_str(local_value)
    }
}

impl Inner {
    pub fn new() -> Self {
        Inner(Tree::new(), None)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }


    pub fn replace(&mut self, idx: usize, len: usize, text: &str, dot: Dot) -> Option<Op> {
        if idx + len > self.len() {
            panic!("index is out of bounds");
        }
        if len == 0 && text.is_empty() {
            return None;
        }

        let merged_edit = self.gen_merged_edit(idx, len, text);
        let offset = self.get_element_offset(merged_edit.idx);

        if offset == 0 && merged_edit.len == 0 {
            Some(self.do_insert(merged_edit.idx, merged_edit.text, dot))
        } else {
            Some(self.do_replace(merged_edit.idx, merged_edit.len, merged_edit.text, dot))
        }
    }

    pub fn do_insert(&mut self, idx: usize, text: String, dot: Dot) -> Op {
        let element = {
            let prev = self.get_prev_element(idx);
            let next = self.get_element(idx);
            Element::between(prev, next, text, dot)
        };

        self.0.insert(element.clone()).unwrap();
        Op{inserted_elements: vec![element], removed_uids: vec![]}
    }

    pub fn do_replace(&mut self, idx: usize, len: usize, text: String, dot: Dot) -> Op {
        let (element, offset) = self.remove_at(idx);
        let border_idx = idx - offset;
        let mut removed_len = element.text.len() - offset;
        let mut removes = vec![element];
        let mut inserts = vec![];

        while removed_len < len {
            let (element, _) = self.remove_at(border_idx);
            removed_len += element.text.len();
            removes.push(element);
        }

        if offset > 0 || !text.is_empty() || removed_len > len {
            let prev = self.get_prev_element(border_idx);
            let next = self.get_element(border_idx);

            if offset > 0 {
                let text = removes[0].text[..offset].to_owned();
                inserts.push(Element::between(prev, next, text, dot));
            }

            if !text.is_empty() {
                let element = Element::between(inserts.last().unwrap_or(prev), next, text, dot);
                inserts.push(element);
            }

            if removed_len > len {
                let old_elt = &removes.last().unwrap();
                let offset  = old_elt.text.len() + len - removed_len;
                let text    = old_elt.text[offset..].to_owned();
                let element = Element::between(inserts.last().unwrap_or(prev), next, text, dot);
                inserts.push(element);
            }
        }

        for element in &inserts {
            self.0.insert(element.clone()).unwrap();
        }

        let removed_uids = removes.into_iter().map(|e| e.uid).collect();
        Op{inserted_elements: inserts, removed_uids}
    }

    pub fn execute_op(&mut self, op: Op) -> Vec<LocalOp> {
        let mut local_ops = vec![];

        for uid in &op.removed_uids {
            if let Some(idx) = self.0.get_idx(uid) {
                let element = self.0.remove(uid).expect("Element must exist H!");
                TextEdit::push(&mut local_ops, idx, element.text.len(), "");
            }
        }

        for element in &op.inserted_elements {
            if self.0.insert(element.clone()).is_ok() {
                let idx = self.0.get_idx(&element.uid).expect("Element must exist I!");
                TextEdit::push(&mut local_ops, idx, 0, &element.text);
            }
        }

        self.shift_merged_edit(&local_ops);
        local_ops
    }

    pub fn merge(&mut self, other: Inner, summary: &Summary, other_summary: &Summary) {
        // ids that are in other_summary and not in other
        let removed_uids: Vec<UID> = self.0.iter()
            .filter(|e| other.0.get_idx(&e.uid).is_none() && other_summary.contains(&e.uid.dot()))
            .map(|e| e.uid.clone())
            .collect();

        // ids that are not in self and not in summary
        let new_elements: Vec<Element> = other.0.into_iter()
            .filter(|e| self.0.get_idx(&e.uid).is_none() && !summary.contains(&e.uid.dot()))
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

    pub fn add_site_id(&mut self, site_id: SiteId) {
        let uids: Vec<UID> = self.0.iter().filter(|e| e.uid.site_id == 0).map(|e| e.uid.clone()).collect();
        for uid in uids {
            let mut element = self.0.remove(&uid).unwrap();
            element.uid.site_id = site_id;
            self.0.insert(element).unwrap();
        }
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        if self.0.iter().any(|e| e.uid.site_id == 0) {
            Err(Error::InvalidSiteId)
        } else {
            Ok(())
        }
    }

    pub fn validate_all(&self, site_id: SiteId) -> Result<(), Error> {
        if self.0.iter().any(|e| e.uid.site_id != site_id) {
            Err(Error::InvalidSiteId)
        } else {
            Ok(())
        }
    }

    pub fn local_value(&self) -> String {
        let mut string = String::with_capacity(self.0.len());
        for element in self.0.iter() {
            string.push_str(&element.text)
        }
        string
    }

    fn remove_at(&mut self, idx: usize) -> (Element, usize) {
        let (uid, offset) = {
            let (element, offset) = self.0.get_elt(idx).expect("Element must exist for UID!");
            (element.uid.clone(), offset)
        };
        let element = self.0.remove(&uid).expect("Element must exist for UID!");
        (element, offset)
    }

    fn get_element(&self, idx: usize) -> &Element {
        if idx == self.len() { return &*END_ELEMENT }
        self.0.get_elt(idx).unwrap().0
    }

    fn get_prev_element(&self, idx: usize) -> &Element {
        if idx == 0 { return &*START_ELEMENT }
        self.0.get_elt(idx-1).unwrap().0
    }

    fn get_element_offset(&self, idx: usize) -> usize {
        if idx == self.len() { return 0 }
        self.0.get_elt(idx).unwrap().1
    }

    fn gen_merged_edit(&mut self, idx: usize, len: usize, text: &str) -> TextEdit {
        if let Some(ref mut old_edit) = self.1 {
            if old_edit.try_overwrite(idx, len, text) {
                return old_edit.clone()
            }
        }

        let edit = TextEdit{idx, len, text: text.into()};
        self.1 = Some(edit.clone());
        edit
    }

    fn shift_merged_edit(&mut self, local_ops: &[LocalOp]) {
        for op in local_ops {
            if let Some(edit) = self.1.take() {
                self.1 = edit.shift_or_destroy(op.idx, op.len, &op.text);
            } else {
                return
            }
        }
    }
}

impl Op {
    pub fn add_site_id(&mut self, site_id: SiteId) {
        for e in &mut self.inserted_elements {
            if e.uid.site_id == 0 { e.uid.site_id = site_id };
        }
        for uid in &mut self.removed_uids {
            if uid.site_id == 0 { uid.site_id = site_id };
        }
    }

    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if self.inserted_elements.iter().any(|e| e.uid.site_id != site_id) {
            Err(Error::InvalidOp)
        } else {
            Ok(())
        }
    }

    pub fn inserted_dots(&self) -> Vec<Dot> {
        self.inserted_elements.iter().map(|elt| elt.uid.dot()).collect()
    }

    #[doc(hidden)]
    pub fn inserted_elements(&self) -> &[Element] {
        &self.inserted_elements
    }

    #[doc(hidden)]
    pub fn removed_uids(&self) -> &[UID] {
        &self.removed_uids
    }
}

impl Element {
    fn between(elt1: &Element, elt2: &Element, text: String, dot: Dot) -> Self {
        Element{text, uid: UID::between(&elt1.uid, &elt2.uid, dot)}
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid.eq(&other.uid)
    }
}

impl Eq for Element { }

impl PartialOrd for Element {
    fn partial_cmp(&self, other: &Element) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Element {
    fn cmp(&self, other: &Element) -> Ordering {
        self.uid.cmp(&other.uid)
    }
}

impl order_statistic_tree::Element for Element {
    type Id = UID;

    fn id(&self) -> &UID {
        &self.uid
    }

    fn element_len(&self) -> usize {
        self.text.len()
    }
}

use serde::{Serialize, Deserialize, Serializer, Deserializer};

impl Clone for Inner {
    fn clone(&self) -> Self {
        Inner(self.0.clone(), None)
    }
}

impl PartialEq for Inner {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Serialize for Inner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Inner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let tree: Tree<Element> = Tree::deserialize(deserializer)?;
        Ok(Inner(tree, None))
    }
}
