//! A `Text` CRDT is a string-like CRDT for mutable text.

mod value;
mod element;
mod btree;

use Error;
use Replica;
use self::value::TextValue;
use self::element::Element;
use traits::*;
use std::mem;

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    value: TextValue,
    replica: Replica,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RemoteOp {
    inserts: Vec<Element>,
    deletes: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalOp {
    changes: Vec<LocalChange>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalChange {
    Insert{index: usize, text: String},
    Delete{index: usize, len: usize},
}

impl Text {

    /// Constructs and returns a new `Text` crdt.
    /// The crdt has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1, 0);
        let value = TextValue::new();
        Text{replica, value, awaiting_site: vec![]}
    }

    /// Returns the number of unicode characters in the text.
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Inserts text at position `index` in the CRDT.
    /// Returns an error if the index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert(&mut self, index: usize, text: String) -> Result<RemoteOp, Error> {
        let remote_op = self.value.insert(index, text, &self.replica)?;
        self.manage_op(remote_op)
    }

    /// Deletes the text in the range [index..<index+len].
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn delete(&mut self, index: usize, len: usize) -> Result<RemoteOp, Error> {
        let remote_op = self.value.delete(index, len, &self.replica)?;
        self.manage_op(remote_op)
    }

    /// Replaces the text in the range [index..<index+len] with new text.
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn replace(&mut self, index: usize, len: usize, text: String) -> Result<RemoteOp, Error> {
        let remote_op = self.value.replace(index, len, text, &self.replica)?;
        self.manage_op(remote_op)
    }

    fn manage_op(&mut self, op: RemoteOp) -> Result<RemoteOp, Error> {
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl Crdt for Text {
    type Value = TextValue;

    fn site(&self) -> u32 {
        self.replica.site
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn clone_value(&self) -> Self::Value {
        self.value.clone()
    }

    fn from_value(value: Self::Value, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        Text{value, replica, awaiting_site: vec![]}
    }

    fn execute_remote(&mut self, op: &RemoteOp) -> Option<LocalOp> {
        self.value.execute_remote(op)
    }

    fn add_site(&mut self, site: u32) -> Result<Vec<RemoteOp>, Error> {
        if self.replica.site != 0 { return Err(Error::AlreadyHasSite) }
        let mut ops = mem::replace(&mut self.awaiting_site, vec![]);
        for op in &mut ops {
            self.value.add_site(op, site);
            op.add_site(site);
        }
        Ok(ops)
    }
}

impl RemoteOp {
    pub fn merge(&mut self, other: RemoteOp) {
        let RemoteOp{mut inserts, mut deletes} = other;
        self.inserts.append(&mut inserts);
        self.deletes.append(&mut deletes);
        self.inserts.sort();
        self.deletes.sort();
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn add_site(&mut self, site: u32) {
        for element in &mut self.inserts {
            if element.uid.site == 0 { element.uid.site = site; }
        }
        for element in &mut self.deletes {
            if element.uid.site == 0 { element.uid.site = site; }
        }
    }
}
