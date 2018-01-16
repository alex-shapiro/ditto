//! A CRDT that stores mutable text

use text::text_edit;
use text::element;

use Error;
use replica::{Dot, Summary, SiteId};
use sequence::uid::{self, UID};
use traits2::*;
use std::borrow::Cow;
use std::mem;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    inner:      Inner,
    site_id:    SiteId,
    summary:    Summary,
    cached_ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextState<'a> {
    inner: Cow<'a, Inner<T>>,
    summary: Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inner(pub Tree<Element>, pub Option<TextEdit>);

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
        let counter = self.summary.increment(self.site_id);

        let op = self.inner.replace(idx, len, text, self.site_id, counter)?;
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

impl From<&str> for Text {
    fn from(local_value: &str) -> Self {
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

    pub fn replace(&mut self, idx: usize, len: usize, text: &str, dot: Dot) -> Option<Result<Op, Error>> {
        if idx + len > self.len() { return Some(Err(Error::OutOfBounds)) }
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
            let prev = self.get_prev_element(idx)?;
            let next = self.get_element(idx)?.0;
            Element::between(prev, next, text, dot)
        }
    }


}
