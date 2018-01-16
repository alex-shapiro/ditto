//! A CRDT that stores mutable text

use dot::{Dot, Summary, SiteId};
use Error;
use order_statistic_tree::{self, Tree};
use sequence::uid::UID;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::mem;
use text::text_edit::TextEdit;

lazy_static! {
    pub static ref START: Element = Element{uid: UID::min(), text: String::new()};
    pub static ref END: Element = Element{uid: UID::max(), text: String::new()};
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    inner:      Inner,
    site_id:    SiteId,
    summary:    Summary,
    cached_ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextState<'a> {
    inner: Cow<'a, Inner>,
    summary: Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inner(pub Tree<Element>, #[serde(skip_serializing, default)] pub Option<TextEdit>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub uid: UID,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    inserted_elements: Vec<Element>,
    removed_uids: Vec<UID>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalOp {
    pub idx:  usize,
    pub len:  usize,
    pub text: String,
}

impl Text {
    /// Constructs and returns a new Text CRDT with site id 1.
    pub fn new() -> Self {
        let inner   = Inner::new();
        let summary = Summary::new();
        let site_id = 1;
        Text{inner, summary, site_id, cached_ops: vec![]}
    }

    /// Constructs and returns a new Text CRDT from a string.
    /// The Text has site id 1.
    pub fn from_str(string: &str) -> Self {
        let text = Text::new();
        let _ = text.replace(0, 0, string).unwrap();
        text
    }

    /// Returns the number of unicode characters in the text.
    pub fn len(&self) -> usize {
        self.inner.0.len()
    }

    /// Replaces the text in the range [index..<index+len] with new text.
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the Text does not have a site id, it caches
    /// the op and returns an `AwaitingSiteId` error.
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

    pub fn replace(&mut self, idx: usize, len: usize, text: &str, dot: Dot) -> Option<Op, Error> {
        if idx + len > self.len() { panic!("index is out of bounds") }
        if len == 0 && text.is_empty() { return None }

        let merged_edit = self.gen_merged_edit(idx, len, text);
        let offset = self.get_element(idx)?.1;

        if offset == 0 && merged_edit.len == 0 {
            Some(Ok(self.do_insert(merged_edit.idx, merged_edit.text, dot)))
        } else {
            Some(Ok(self.do_replace(merged_edit.idx, merged_edit.len, merged_edit.text, dot)))
        }
    }

    pub fn do_insert(&mut self, idx: usize, text: String, dot: Dot) -> Op {
        let element = {
            let prev = self.get_prev_element(idx).unwrap();
            let next = self.get_element(idx).unwrap().0;
            Element::between(prev, next, text, dot)
        };

        self.0.insert(element.clone()).unwrap();
        Op{inserted_elements: vec![element], removed_uids: vec![]}
    }

    pub fn do_replace(&mut self, idx: usize, len: usize, text: String, dot: Dot) -> Op {
        let (element, offset) = self.remove_at(idx).unwrap();
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
            let prev = self.get_prev_element(border_idx).unwrap();
            let (next, _) = self.get_element(border_idx).unwrap();

            if offset > 0 {
                let (text, _) = removes[0].text.char_split(offset);
                inserts.push(Element::between(prev, next, text.into(), dot));
            }

            if !text.is_empty() {
                let element = Element::between(inserts.last().unwrap_or(prev), next, text, dot);
                inserts.push(element);
            }

            if removed_len > len {
                let old_elt = &removes.last().unwrap();
                let offset = old_elt.len + len - removed_len;
                let (_, text) = old_elt.text.char_split(offset);
                let element = Element::between(inserts.last().unwrap_or(prev), next, text.into(), dot);
                inserts.push(element);
            }
        }

        for element in &inserts {
            self.0.insert(element.clone()).unwrap();
        }

        let removed_uids = removes.into_iter().map(|e| e.uid).collect();
        Ok(Op{inserted_elements: inserts, removed_uids})
    }

    pub fn execute_op(&mut self, op: Op) -> Vec<LocalOp> {
        let mut local_ops = Vec::with_capacity(op.inserted_elements.len() + op.removed_uids.len());

        for uid in &op.removed_uids {
            if let Some(char_index) = self.0.get_idx(&uid) {
                let element = self.0.remove(&uid).expect("Element must exist H!");
                local_ops.push(LocalOp{idx: char_index, len: element.len, text: "".into()});
            }
        }

        for element in &op.inserted_elements {
            if let Ok(_) = self.0.insert(element.clone()) {
                let char_index = self.0.get_idx(&element.uid).expect("Element must exist I!");
                local_ops.push(LocalOp{idx: char_index, len: 0, text: element.text.clone()});
            }
        }

        self.shift_merged_edit(&local_ops);
        local_ops
    }

    pub fn merge(&mut self, other: Inner, summary: &Summary, other_summary: &Summary) {
        // ids that are in other_summary and not in other
        let removed_uids: Vec<UID> = self.0.iter()
            .filter(|e| other.0.get_idx(&e.uid).is_none() && other_summary.contains(e.uid.dot()))
            .map(|e| e.uid.clone())
            .collect();

        // ids that are not in self and not in summary
        let new_elements: Vec<Element> = other.0.into_iter()
            .filter(|e| self.0.get_idx(&e.uid).is_none() && !summary.contains(e.uid.dot()))
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
        let uids: Vec<UID> = self.0.iter().filter(|e| e.uid.site_id == 0).map(|e| e.uid).collect();
        for uid in uids {
            let (element, _) = self.0.remove(&uid).unwrap();
            element.uid.site_id = site_id;
            let _ = self.0.insert(element).unwrap();
        }
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        if self.0.iter().any(|e| e.uid.site_id == 0) {
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

    fn remove_at(&mut self, index: usize) -> (Element, usize) {
        let (uid, offset) = {
            let (element, offset) = self.0.get_elt(index).expect("Element must exist for UID!");
            (element.uid.clone(), offset)
        };
        let element = self.0.remove(&uid).expect("Element must exist for UID!");
        Ok((element, offset))
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
        self.inserted_elements.iter().map(UID::dot).collect()
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
        Some(self.cmp(&other))
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
