use compact;
use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp};
use raw;
use Replica;
use serde_json::Value as Json;
use serde_json;
use Value;
use std::cmp::Ordering;

type R<T> = Result<T, Error>;

pub struct CRDT {
    root_value: Value,
    replica: Replica,
    session_counter: usize,
    rewindable: Vec<NestedRemoteOp>,
}

impl CRDT {
    pub fn new(json: &Json, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        let value = raw::decode(json, &replica);
        CRDT{
            root_value: value,
            replica: replica,
            session_counter: 0,
            rewindable: vec![],
        }
    }

    pub fn new_str(string: &str, site: u32) -> Self {
        let json: Json = serde_json::de::from_str(string).expect("invalid JSON!");
        CRDT::new(&json, site)
    }

    pub fn serialize(&self) -> Json {
        compact::encode(&self.root_value)
    }

    pub fn deserialize(json: &Json, replica: Replica) -> R<Self> {
        let value = try!(compact::decode(json));
        Ok(CRDT{
            root_value: value,
            replica: replica,
            session_counter: 0,
            rewindable: vec![],
        })
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
            Some(serde_json::ser::to_string(&json).ok().unwrap())
        })
    }

    pub fn put(&mut self, session_counter: usize, pointer: String, key: String, value: &Json) -> R<NestedRemoteOp> {
        let op = op::local::Put{key: key, value: raw::decode(value, &self.replica)};
        self.execute_local(session_counter, pointer, LocalOp::Put(op))
    }

    pub fn put_str(&mut self, session_counter: usize, pointer: String, key: String, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.put(session_counter, pointer, key, &json)
    }

    pub fn delete(&mut self, session_counter: usize, pointer: String, key: String) -> R<NestedRemoteOp> {
        let op = op::local::Delete{key: key};
        self.execute_local(session_counter, pointer, LocalOp::Delete(op))
    }

    pub fn insert_item(&mut self, session_counter: usize, pointer: String, index: usize, item: &Json) -> R<NestedRemoteOp> {
        let op = op::local::InsertItem{index: index, value: raw::decode(item, &self.replica)};
        self.execute_local(session_counter, pointer, LocalOp::InsertItem(op))
    }

    pub fn insert_item_str(&mut self, session_counter: usize, pointer: String, index: usize, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.insert_item(session_counter, pointer, index, &json)
    }

    pub fn delete_item(&mut self, session_counter: usize, pointer: String, index: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteItem{index: index};
        self.execute_local(session_counter, pointer, LocalOp::DeleteItem(op))
    }

    pub fn insert_text(&mut self, session_counter: usize, pointer: String, index: usize, text: String) -> R<NestedRemoteOp> {
        let op = op::local::InsertText{index: index, text: text};
        self.execute_local(session_counter, pointer, LocalOp::InsertText(op))
    }

    pub fn delete_text(&mut self, session_counter: usize, pointer: String, index: usize, len: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteText{index: index, len: len};
        self.execute_local(session_counter, pointer, LocalOp::DeleteText(op))
    }

    pub fn replace_text(&mut self, session_counter: usize, pointer: String, index: usize, len: usize, text: String) -> R<NestedRemoteOp> {
        let op = op::local::ReplaceText{index: index, len: len, text: text};
        self.execute_local(session_counter, pointer, LocalOp::ReplaceText(op))
    }

    pub fn increment(&mut self, session_counter: usize, pointer: String, amount: f64) -> R<NestedRemoteOp> {
        let op = op::local::IncrementNumber{amount: amount};
        self.execute_local(session_counter, pointer, LocalOp::IncrementNumber(op))
    }

    pub fn execute_local(&mut self, session_counter: usize, pointer: String, op: LocalOp) -> R<NestedRemoteOp> {
        match session_counter.cmp(&self.session_counter) {
            Ordering::Greater => {
                Err(Error::InvalidSessionCounter)
            },
            Ordering::Equal => {
                self.rewindable.clear();
                self.replica.counter += 1;
                let (mut value, remote_ptr) = try!(self.root_value.get_nested_local(&pointer));
                let remote_op = try!(value.execute_local(op, &self.replica));
                Ok(NestedRemoteOp{pointer: remote_ptr, op: remote_op})
            },
            Ordering::Less => {
                Err(Error::InvalidSessionCounter)
            },
        }
    }

    pub fn execute_remote(&mut self, nested_op: NestedRemoteOp) -> R<Vec<NestedLocalOp>> {
        let (mut value, local_ptr) = {
            let ref ptr = nested_op.pointer;
            try!(self.root_value.get_nested_remote(ptr))
        };
        let local_ops = try!(value.execute_remote(&nested_op.op));
        self.rewindable.push(nested_op);
        self.session_counter += 1;
        let session_counter = self.session_counter;

        Ok(local_ops
            .into_iter()
            .map(|local_op| NestedLocalOp{
                pointer: local_ptr.clone(),
                session_counter: session_counter,
                op: local_op})
            .collect())
    }

    pub fn execute_remote_json(&mut self, nested_op_json: &Json) -> R<Vec<NestedLocalOp>> {
        let nested_op = try!(compact::decode_op(nested_op_json));
        self.execute_remote(nested_op)
    }
}
