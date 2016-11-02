use compact;
use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp};
use raw;
use Replica;
use serde_json::Value as Json;
use serde_json;
use Value;

type R<T> = Result<T, Error>;

pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    pub fn new(json: &Json, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        let value = raw::decode(json, &replica);
        CRDT{root_value: value, replica: replica}
    }

    pub fn new_str(string: &str, site: u32) -> Self {
        let json: Json = serde_json::from_str(string).expect("invalid JSON!");
        CRDT::new(&json, site)
    }

    pub fn serialize(&self) -> Json {
        compact::encode(&self.root_value)
    }

    pub fn deserialize(json: &Json, replica: Replica) -> R<Self> {
        let value = try!(compact::decode(json));
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn get_replica(&self) -> &Replica {
        &self.replica
    }

    pub fn get(&mut self, pointer: &str) -> Option<Json> {
        let value = self.root_value.get_nested_local(pointer).ok();
        value.and_then(|(v, _)| Some(raw::encode(v)))
    }

    pub fn get_str(&mut self, pointer: &str) -> Option<String> {
        self.get(pointer).and_then(|json| {
            Some(serde_json::to_string(&json).ok().unwrap())
        })
    }

    pub fn put(&mut self, pointer: String, key: String, value: &Json) -> R<NestedRemoteOp> {
        let op = op::local::Put{key: key, value: raw::decode(value, &self.replica)};
        self.execute_local(pointer, LocalOp::Put(op))
    }

    pub fn put_str(&mut self, pointer: String, key: String, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.put(pointer, key, &json)
    }

    pub fn delete(&mut self, pointer: String, key: String) -> R<NestedRemoteOp> {
        let op = op::local::Delete{key: key};
        self.execute_local(pointer, LocalOp::Delete(op))
    }

    pub fn insert_item(&mut self, pointer: String, index: usize, item: &Json) -> R<NestedRemoteOp> {
        let op = op::local::InsertItem{index: index, value: raw::decode(item, &self.replica)};
        self.execute_local(pointer, LocalOp::InsertItem(op))
    }

    pub fn insert_item_str(&mut self, pointer: String, index: usize, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.insert_item(pointer, index, &json)
    }

    pub fn delete_item(&mut self, pointer: String, index: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteItem{index: index};
        self.execute_local(pointer, LocalOp::DeleteItem(op))
    }

    pub fn insert_text(&mut self, pointer: String, index: usize, text: String) -> R<NestedRemoteOp> {
        let op = op::local::InsertText{index: index, text: text};
        self.execute_local(pointer, LocalOp::InsertText(op))
    }

    pub fn delete_text(&mut self, pointer: String, index: usize, len: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteText{index: index, len: len};
        self.execute_local(pointer, LocalOp::DeleteText(op))
    }

    pub fn replace_text(&mut self, pointer: String, index: usize, len: usize, text: String) -> R<NestedRemoteOp> {
        let op = op::local::ReplaceText{index: index, len: len, text: text};
        self.execute_local(pointer, LocalOp::ReplaceText(op))
    }

    pub fn increment(&mut self, pointer: String, amount: f64) -> R<NestedRemoteOp> {
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

    fn execute_local(&mut self, pointer: String, op: LocalOp) -> R<NestedRemoteOp> {
        self.replica.counter += 1;
        let (mut value, remote_ptr) = try!(self.root_value.get_nested_local(&pointer));
        let remote_op = try!(value.execute_local(op, &self.replica));
        Ok(NestedRemoteOp{pointer: remote_ptr, op: remote_op})
    }
}
