//! A `Text` CRDT is a string-like CRDT for mutable text.

mod value;
mod element;
mod btree;

use Error;
use Replica;
pub use self::value::TextValue;
use self::element::Element;
use sequence::uid::UID;
use traits::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    value: TextValue,
    replica: Replica,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp {
    pub inserts: Vec<Element>,
    pub removes: Vec<UID>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalOp {
    changes: Vec<LocalChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalChange {
    Insert{index: usize, text: String},
    Remove{index: usize, len: usize},
}

impl Text {

    crdt_impl!(Text, TextValue);

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
        let op = self.value.insert(index, text, &self.replica)?;
        self.after_op(op)
    }

    /// Removes the text in the range [index..<index+len].
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, index: usize, len: usize) -> Result<RemoteOp, Error> {
        let op = self.value.remove(index, len, &self.replica)?;
        self.after_op(op)
    }

    /// Replaces the text in the range [index..<index+len] with new text.
    /// Returns an error if the start or stop index is out-of-bounds.
    /// If the crdt does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn replace(&mut self, index: usize, len: usize, text: String) -> Result<RemoteOp, Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use rmp_serde;

    #[test]
    fn test_new() {
        let text = Text::new();
        assert!(text.len() == 0);
        assert!(text.replica.site == 1);
        assert!(text.replica.counter == 0);
    }

    #[test]
    fn test_insert() {
        let mut text = Text::new();
        let remote_op = text.insert(0, "🇺🇸😀Hello".to_owned()).unwrap();
        assert!(text.len() == 8);
        assert!(text.local_value() == "🇺🇸😀Hello");
        assert!(text.replica.counter == 1);
        assert!(remote_op.inserts[0].uid.site == 1);
        assert!(remote_op.inserts[0].uid.counter == 0);
        assert!(remote_op.inserts[0].len == 8);
        assert!(remote_op.inserts[0].text == "🇺🇸😀Hello");
    }

    #[test]
    fn test_insert_out_of_bounds() {
        let mut text = Text::new();
        let _ = text.insert(0, "Hello".to_owned()).unwrap();
        assert!(text.insert(6, "A".to_owned()).unwrap_err() == Error::OutOfBounds);
        assert!(text.insert(5, "".to_owned()).unwrap_err() == Error::Noop);
    }

    #[test]
    fn test_remove() {
        let mut text = Text::new();
        let remote_op1 = text.insert(0, "I am going".to_owned()).unwrap();
        let remote_op2 = text.remove(2, 2).unwrap();
        assert!(text.len() == 8);
        assert!(text.local_value() == "I  going");
        assert!(text.replica.counter == 2);
        assert!(remote_op1.inserts[0].uid == remote_op2.removes[0]);
        assert!(remote_op2.inserts[0].text == "I ");
        assert!(remote_op2.inserts[1].text == " going");
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut text = Text::new();
        let _ = text.insert(0, "I am going".to_owned()).unwrap();
        assert!(text.remove(5, 20).unwrap_err() == Error::OutOfBounds);
        assert!(text.remove(5, 0).unwrap_err() == Error::Noop);
    }

    #[test]
    fn test_insert_remove_awaiting_site() {
        let mut text = Text::from_value(TextValue::new(), 0);
        assert!(text.insert(0, "Hello".to_owned()).unwrap_err() == Error::AwaitingSite);
        assert!(text.remove(0, 1).unwrap_err() == Error::AwaitingSite);
        assert!(text.local_value() == "ello");
        assert!(text.len() == 4);
        assert!(text.replica.counter == 2);
        assert!(text.awaiting_site.len() == 2);
    }

    #[test]
    fn test_execute_remote() {
        let mut text1 = Text::new();
        let mut text2 = Text::from_value(text1.clone_value(), 0);

        let remote_op1 = text1.insert(0, "hello".to_owned()).unwrap();
        let remote_op2 = text1.remove(0, 1).unwrap();
        let remote_op3 = text1.replace(2, 1, "orl".to_owned()).unwrap();
        let local_op1  = text2.execute_remote(&remote_op1).unwrap();
        let local_op2  = text2.execute_remote(&remote_op2).unwrap();
        let local_op3  = text2.execute_remote(&remote_op3).unwrap();

        assert!(text1.value() == text2.value());
        assert!(local_op1.changes.len() == 1);
        assert!(local_op2.changes.len() == 2);
        assert!(local_op3.changes.len() == 4);
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut text1 = Text::new();
        let mut text2 = Text::from_value(text1.clone_value(), 0);
        let remote_op = text1.insert(0, "hello".to_owned()).unwrap();
        assert!(text2.execute_remote(&remote_op).is_some());
        assert!(text2.execute_remote(&remote_op).is_none());
        assert!(text1.value() == text2.value());
    }

    #[test]
    fn test_add_site() {
        let mut text = Text::from_value(TextValue::new(), 0);
        let _ = text.insert(0, "hello".to_owned());
        let _ = text.insert(5, "there".to_owned());
        let _ = text.remove(4, 1);
        let mut remote_ops = text.add_site(7).unwrap().into_iter();

        let remote_op1 = remote_ops.next().unwrap();
        let remote_op2 = remote_ops.next().unwrap();
        let remote_op3 = remote_ops.next().unwrap();

        assert!(remote_op1.inserts[0].uid.site == 7);
        assert!(remote_op2.inserts[0].uid.site == 7);
        assert!(remote_op3.removes[0].site == 7);
        assert!(remote_op3.inserts[0].uid.site == 7);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut text = Text::from_value(TextValue::new(), 123);
        let _ = text.insert(0, "hello".to_owned()).unwrap();
        let _ = text.insert(5, "there".to_owned()).unwrap();
        let _ = text.remove(4, 1).unwrap();
        assert!(text.add_site(7).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let mut text1 = Text::new();
        let _ = text1.insert(0, "hello".to_owned());
        let _ = text1.insert(5, "there".to_owned());

        let s_json = serde_json::to_string(&text1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&text1).unwrap();
        let text2: Text = serde_json::from_str(&s_json).unwrap();
        let text3: Text = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(text1 == text2);
        assert!(text1 == text3);
    }

    #[test]
    fn test_serialize_value() {
        let mut text1 = Text::new();
        let _ = text1.insert(0, "hello".to_owned());
        let _ = text1.insert(5, "there".to_owned());

        let s_json = serde_json::to_string(text1.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(text1.value()).unwrap();
        let value2: TextValue = serde_json::from_str(&s_json).unwrap();
        let value3: TextValue = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*text1.value() == value2);
        assert!(*text1.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut text = Text::new();
        let _ = text.insert(0, "hello".to_owned()).unwrap();
        let remote_op1 = text.insert(2, "bonjour".to_owned()).unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let mut text1 = Text::new();
        let mut text2 = Text::from_value(text1.clone_value(), 2);
        let remote_op1 = text1.insert(0, "hello".to_owned()).unwrap();
        let remote_op2 = text1.insert(2, "bonjour".to_owned()).unwrap();
        let _ = text2.execute_remote(&remote_op1).unwrap();
        let local_op1 = text2.execute_remote(&remote_op2).unwrap();

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }
}
