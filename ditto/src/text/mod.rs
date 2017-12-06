//! A CRDT that stores text.

mod value;
mod element;
mod text_edit;

use {Error, Replica, Tombstones};
pub use self::value::TextValue;
use self::element::Element;
use sequence::uid::UID;
use traits::*;

use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    value: TextValue,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextState<'a> {
    value: Cow<'a, TextValue>,
    tombstones: Cow<'a, Tombstones>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp {
    pub inserts: Vec<Element>,
    pub removes: Vec<UID>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalOp(pub Vec<LocalChange>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalChange {
    pub idx:  usize,
    pub len:  usize,
    pub text: String,
}

impl Text {

    crdt_impl!(Text, TextState, TextState, TextState<'static>, TextValue);

    /// Constructs and returns a new `Text` crdt.
    /// The crdt has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1, 0);
        let value = TextValue::new();
        let tombstones = Tombstones::new();
        Text{replica, value, tombstones, awaiting_site: vec![]}
    }

    /// Constructs and returns a new `Text` crdt from a string.
    /// Each paragraph is split into a separate element.
    pub fn from_str(string: &str) -> Self {
        let replica = Replica::new(1, 0);
        let value = TextValue::from_str(string, &replica);
        let tombstones = Tombstones::new();
        Text{replica, value, tombstones, awaiting_site: vec![]}
    }

    /// Returns the number of unicode characters in the text.
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Replaces the text in the range [index..<index+len] with new text.
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn replace(&mut self, index: usize, len: usize, text: &str) -> Result<RemoteOp, Error> {
        let op = self.value.replace(index, len, text, &self.replica)?;
        self.after_op(op)
    }
}

impl RemoteOp {
    pub fn merge(&mut self, other: RemoteOp) {
        let RemoteOp{mut inserts, mut removes} = other;
        self.inserts.append(&mut inserts);
        self.removes.append(&mut removes);
        self.inserts.sort();
        self.removes.sort();
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn deleted_replicas(&self) -> Vec<Replica> {
        self.removes.iter()
            .map(|uid| Replica{site: uid.site, counter: uid.counter})
            .collect()
    }

    fn add_site(&mut self, site: u32) {
        for element in &mut self.inserts {
            element.uid.site = site;
        }
        for uid in &mut self.removes {
            if uid.site == 0 { uid.site = site; }
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        for element in &self.inserts {
            try_assert!(element.uid.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }
}
