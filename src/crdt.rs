use compact;
use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp, RemoteOp};
use raw;
use Replica;
use serde_json::Value as Json;
use serde_json;
use Value;

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

    pub fn put(&mut self, session_counter: usize, pointer: &str, key: &str, value: &Json) -> R<NestedRemoteOp> {
        let op = op::local::Put{key: key.to_owned(), value: raw::decode(value, &self.replica)};
        let nested_op = NestedLocalOp{
            session_counter: session_counter,
            pointer: pointer.to_owned(),
            op: LocalOp::Put(op),
        };

        self.execute_local(nested_op)
    }

    pub fn put_str(&mut self, session_counter: usize, pointer: &str, key: &str, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.put(session_counter, pointer, key, &json)
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> R<NestedRemoteOp> {
        let (mut nested_value, ptr) = try!(self.root_value.get_nested_local(pointer));
        let mut object = try!(nested_value.as_object());
        let op = try!(object.delete(key));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateObject(op)})
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, item: &Json) -> R<NestedRemoteOp> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let (mut nested_value, ptr) = try!(root_value.get_nested_local(pointer));
        let mut array = try!(nested_value.as_array());
        let op = try!(array.insert(index, raw::decode(item, replica), replica));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateArray(op)})
    }

    pub fn insert_item_str(&mut self, pointer: &str, index: usize, item: &str) -> R<NestedRemoteOp> {
        let json: Json = serde_json::from_str(item).expect("invalid JSON!");
        self.insert_item(pointer, index, &json)
    }

    pub fn delete_item(&mut self, pointer: &str, index: usize) -> R<NestedRemoteOp> {
        let (mut nested_value, ptr) = try!(self.root_value.get_nested_local(pointer));
        let mut array = try!(nested_value.as_array());
        let op = try!(array.delete(index));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateArray(op)})
    }

    pub fn insert_text(&mut self, pointer: &str, index: usize, text: String) -> R<NestedRemoteOp> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let (mut nested_value, ptr) = try!(root_value.get_nested_local(pointer));
        let mut attrstr = try!(nested_value.as_attributed_string());
        let op = try!(attrstr.insert_text(index, text, replica));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateAttributedString(op)})
    }

    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> R<NestedRemoteOp> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let (mut nested_value, ptr) = try!(root_value.get_nested_local(pointer));
        let mut attrstr = try!(nested_value.as_attributed_string());
        let op = try!(attrstr.delete_text(index, len, replica));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateAttributedString(op)})
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: String) -> R<NestedRemoteOp> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let (mut nested_value, ptr) = try!(root_value.get_nested_local(pointer));
        let mut attrstr = try!(nested_value.as_attributed_string());
        let op = try!(attrstr.replace_text(index, len, text, replica));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::UpdateAttributedString(op)})
    }

    pub fn increment(&mut self, pointer: &str, amount: f64) -> R<NestedRemoteOp> {
        let (mut nested_value, ptr) = try!(self.root_value.get_nested_local(pointer));
        let op = try!(nested_value.increment(amount));
        Ok(NestedRemoteOp{pointer: ptr, op: RemoteOp::IncrementNumber(op)})
    }

    pub fn execute_local(&mut self, nested_op: NestedLocalOp) -> R<NestedRemoteOp> {
        let (mut value, ptr) = {
            let ref ptr = nested_op.pointer;
            try!(self.root_value.get_nested_local(ptr))
        };

       let remote_op = try!(value.execute_local(nested_op.op, &self.replica));
       self.replica.counter += 1;
       Ok(NestedRemoteOp{pointer: ptr, op: remote_op})
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
