use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp};
use LocalValue;
use Replica;
use serde_json;
use Value;

type R<T> = Result<T, Error>;

/// A conflict-free replicated datatype that supports all
/// JSON-representable values, plus a mutable string type
/// called `AttributedString`.
///
/// CRDT is designed to be edited concurrently at many locations.
/// It avoids conflict by marking each value with IDs that
/// uniquely identify the location (the CRDT's `site`) and time
/// (the CRDT's `counter`).
///
/// The CRDT has functions for persistence, loading and user-friendly
/// JSON serialization. Network operations, namely unique site allocation
/// and `NestedRemoteOp` syncing, are left to the user.
///
#[derive(Debug)]
pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    /// Constructs a new CRDT from a user-friendly JSON string.
    /// The site and counter are both set to 1. Use this function
    /// only when creating a CRDT for the very first time. To load
    /// a pre-existing CRDT, use `load`.
    pub fn create(local_value_str: &str) -> R<Self> {
        let mut replica = Replica::new(1, 0);
        let local_value: LocalValue = serde_json::from_str(local_value_str)?;
        let value = local_value.to_value(&replica);
        replica.counter = 1;
        Ok(CRDT{root_value: value, replica: replica})
    }

    /// Generates a CRDT from a compactly-encoded JSON string, a
    /// site, and a counter. Use this function only when loading
    /// a pre-existing CRDT. To create a new CRDT, use `create`.
    pub fn load(value_str: &str, site: u32, counter: u32) -> R<Self> {
        let replica = Replica::new(site, counter);
        let value: Value = serde_json::from_str(value_str)?;
        Ok(CRDT{root_value: value, replica: replica})
    }

    /// Generates a compactly-encoded JSON string that represents
    /// the value of the CRDT. Use this function to persist the CRDT
    /// or send it over a network connection; for presenting information
    /// to the user, serialize `local_value`.
    pub fn dump(&self) -> String {
        serde_json::to_string(&self.root_value).unwrap()
    }

    /// Returns the CRDT's site. A site is an integer that
    /// uniquely identifies each location editing the CRDT.
    /// Together the with counter, it uniquely identifies
    /// each CRDT value and makes "conflict-free replication"
    /// possible.
    pub fn site(&self) -> u32 {
        self.replica.site
    }

    /// Returns the CRDT's counter. A counter is an integer
    /// that increments after each unique operation. Together
    /// with the site, it uniquely identifies each CRDT value
    /// and makes "conflict-free replication" possible.
    pub fn counter(&self) -> u32 {
        self.replica.counter
    }

    /// Returns the CRDT's value.
    pub fn value<'a>(&'a self) -> &'a Value {
        &self.root_value
    }

    /// Returns the CRDT's value, consuming the CRDT.
    pub fn into_value(self) -> Value {
        self.root_value
    }

    /// Returns the CRDT's user-friendly value, consuming the CRDT.
    pub fn local_value(self) -> LocalValue {
        self.root_value.into()
    }

    /// Executes a local operation on the CRDT. Most users will
    /// prefer the convenience functions, each of which executes
    /// a specific type of local operation.
    pub fn execute_local(&mut self, local_op: NestedLocalOp) -> R<NestedRemoteOp> {
        self.do_execute_local(&local_op.pointer, local_op.op)
    }

    /// Sets or updates an object value in the CRDT and returns a
    ///`NestedRemoteOp` for replicating the operation at other sites.
    pub fn put(&mut self, pointer: &str, key: &str, local_value_str: &str) -> R<NestedRemoteOp> {
        let local_value: LocalValue = serde_json::from_str(&local_value_str)?;
        let op = op::local::Put{key: key.to_owned(), value: local_value};
        self.do_execute_local(pointer, LocalOp::Put(op))
    }

    /// Deletes an object value from the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn delete(&mut self, pointer: &str, key: &str) -> R<NestedRemoteOp> {
        let op = op::local::Delete{key: key.to_owned()};
        self.do_execute_local(pointer, LocalOp::Delete(op))
    }

    /// Inserts a value into an array in the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn insert_item(&mut self, pointer: &str, index: usize, local_value_str: &str) -> R<NestedRemoteOp> {
        let local_value: LocalValue = serde_json::from_str(&local_value_str)?;
        let op = op::local::InsertItem{index: index, value: local_value};
        self.do_execute_local(pointer, LocalOp::InsertItem(op))
    }

    /// Deletes a value from an array in the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn delete_item(&mut self, pointer: &str, index: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteItem{index: index};
        self.do_execute_local(pointer, LocalOp::DeleteItem(op))
    }

    /// Inserts text into an AttributedString in the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn insert_text(&mut self, pointer: &str, index: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::InsertText{index: index, text: text.to_owned()};
        self.do_execute_local(pointer, LocalOp::InsertText(op))
    }

    /// Deletes text from an AttributedString in the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteText{index: index, len: len};
        self.do_execute_local(pointer, LocalOp::DeleteText(op))
    }

    /// Replaces text in an AttributedString in the CRDT and returns a
    /// `NestedRemoteOp` for replicating the operation at other sites.
    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::ReplaceText{index: index, len: len, text: text.to_owned()};
        self.do_execute_local(pointer, LocalOp::ReplaceText(op))
    }

    /// Executes a `NestedRemoteOp`, replicating an operation that
    /// was generated at another site.
    pub fn execute_remote(&mut self, nested_op: NestedRemoteOp) -> R<Vec<NestedLocalOp>> {
        let (mut value, local_ptr) = {
            let ref ptr = nested_op.pointer;
            self.root_value.get_nested_remote(ptr)?
        };
        let local_ops = value.execute_remote(&nested_op.op)?;
        Ok(local_ops
            .into_iter()
            .map(|op| NestedLocalOp{pointer: local_ptr.clone(), op: op})
            .collect())
    }

    fn do_execute_local(&mut self, pointer: &str, op: LocalOp) -> R<NestedRemoteOp> {
        self.replica.counter += 1;
        let (mut value, remote_ptr) = self.root_value.get_nested_local(pointer)?;
        let remote_op = value.execute_local(op, &self.replica)?;
        Ok(NestedRemoteOp{pointer: remote_ptr, op: remote_op})
    }
}
