use compact;
use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp};
use raw;
use Replica;
use serde_json::Value as Json;
use serde_json;
use Value;

type R<T> = Result<T, Error>;

#[derive(Debug)]
pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    pub fn create(raw: &str) -> R<Self> {
        let raw_json = serde_json::from_str(raw)?;
        let replica = Replica::new(1, 0);
        let value = raw::decode(&raw_json, &replica)?;
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn load(compact: &str, site: u32, counter: u32) -> R<Self> {
        let compact_json = try!(serde_json::from_str(compact));
        let replica = Replica::new(site, counter);
        let value = try!(compact::decode(&compact_json));
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn dump(&self) -> String {
        let json = compact::encode(&self.root_value);
        serde_json::to_string(&json).unwrap()
    }

    pub fn site(&self) -> u32 {
        self.replica.site
    }

    pub fn counter(&self) -> u32 {
        self.replica.counter
    }

    pub fn value<'a>(&'a self) -> &'a Value {
        &self.root_value
    }

    pub fn as_value(self) -> Value {
        self.root_value
    }

    pub fn put(&mut self, pointer: &str, key: &str, value: &str) -> R<NestedRemoteOp> {
        let value_json: Json = serde_json::from_str(value)?;
        let value = raw::decode(&value_json, &self.replica)?;
        let op = op::local::Put{key: key.to_owned(), value: value};
        self.execute_local(pointer, LocalOp::Put(op))
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> R<NestedRemoteOp> {
        let op = op::local::Delete{key: key.to_owned()};
        self.execute_local(pointer, LocalOp::Delete(op))
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, item: &str) -> R<NestedRemoteOp> {
        let item_json: Json = serde_json::from_str(item)?;
        let item = raw::decode(&item_json, &self.replica)?;
        let op = op::local::InsertItem{index: index, value: item};
        self.execute_local(pointer, LocalOp::InsertItem(op))
    }

    pub fn delete_item(&mut self, pointer: &str, index: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteItem{index: index};
        self.execute_local(pointer, LocalOp::DeleteItem(op))
    }

    pub fn insert_text(&mut self, pointer: &str, index: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::InsertText{index: index, text: text.to_owned()};
        self.execute_local(pointer, LocalOp::InsertText(op))
    }

    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteText{index: index, len: len};
        self.execute_local(pointer, LocalOp::DeleteText(op))
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::ReplaceText{index: index, len: len, text: text.to_owned()};
        self.execute_local(pointer, LocalOp::ReplaceText(op))
    }

    pub fn increment(&mut self, pointer: &str, amount: f64) -> R<NestedRemoteOp> {
        let op = op::local::IncrementNumber{amount: amount};
        self.execute_local(pointer, LocalOp::IncrementNumber(op))
    }

    pub fn execute_remote(&mut self, nested_op: NestedRemoteOp) -> R<Vec<NestedLocalOp>> {
        let (mut value, local_ptr) = {
            let ref ptr = nested_op.pointer;
            try!(self.root_value.get_nested_remote(ptr))
        };
        let local_ops = try!(value.execute_remote(&nested_op.op));
        Ok(local_ops
            .into_iter()
            .map(|op| NestedLocalOp{pointer: local_ptr.clone(), op: op})
            .collect())
    }

    pub fn execute_remote_json(&mut self, nested_op_json: &Json) -> R<Vec<NestedLocalOp>> {
        let nested_op = try!(compact::decode_op(nested_op_json));
        self.execute_remote(nested_op)
    }

    fn execute_local(&mut self, pointer: &str, op: LocalOp) -> R<NestedRemoteOp> {
        self.replica.counter += 1;
        let (mut value, remote_ptr) = try!(self.root_value.get_nested_local(pointer));
        let remote_op = try!(value.execute_local(op, &self.replica));
        Ok(NestedRemoteOp{pointer: remote_ptr, op: remote_op})
    }
}
