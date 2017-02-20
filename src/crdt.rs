use Error;
use op::{self, NestedLocalOp, NestedRemoteOp, LocalOp};
use LocalValue;
use Replica;
use serde_json;
use Value;

type R<T> = Result<T, Error>;

#[derive(Debug)]
pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    pub fn create(local_value_str: &str) -> R<Self> {
        let replica = Replica::new(1, 0);
        let local_value: LocalValue = serde_json::from_str(local_value_str)?;
        let value = local_value.to_value(&replica);
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn load(value_str: &str, site: u32, counter: u32) -> R<Self> {
        let replica = Replica::new(site, counter);
        let value: Value = serde_json::from_str(value_str)?;
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn dump(&self) -> String {
        serde_json::to_string(&self.root_value).unwrap()
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

    pub fn local_value(self) -> LocalValue {
        self.root_value.into()
    }

    pub fn put(&mut self, pointer: &str, key: &str, local_value_str: &str) -> R<NestedRemoteOp> {
        let local_value: LocalValue = serde_json::from_str(&local_value_str)?;
        let op = op::local::Put{key: key.to_owned(), value: local_value};
        self.do_execute_local(pointer, LocalOp::Put(op))
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> R<NestedRemoteOp> {
        let op = op::local::Delete{key: key.to_owned()};
        self.do_execute_local(pointer, LocalOp::Delete(op))
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, local_value_str: &str) -> R<NestedRemoteOp> {
        let local_value: LocalValue = serde_json::from_str(&local_value_str)?;
        let op = op::local::InsertItem{index: index, value: local_value};
        self.do_execute_local(pointer, LocalOp::InsertItem(op))
    }

    pub fn delete_item(&mut self, pointer: &str, index: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteItem{index: index};
        self.do_execute_local(pointer, LocalOp::DeleteItem(op))
    }

    pub fn insert_text(&mut self, pointer: &str, index: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::InsertText{index: index, text: text.to_owned()};
        self.do_execute_local(pointer, LocalOp::InsertText(op))
    }

    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> R<NestedRemoteOp> {
        let op = op::local::DeleteText{index: index, len: len};
        self.do_execute_local(pointer, LocalOp::DeleteText(op))
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: &str) -> R<NestedRemoteOp> {
        let op = op::local::ReplaceText{index: index, len: len, text: text.to_owned()};
        self.do_execute_local(pointer, LocalOp::ReplaceText(op))
    }

    pub fn increment(&mut self, pointer: &str, amount: f64) -> R<NestedRemoteOp> {
        let op = op::local::IncrementNumber{amount: amount};
        self.do_execute_local(pointer, LocalOp::IncrementNumber(op))
    }

    pub fn execute_local(&mut self, local_op: NestedLocalOp) -> R<NestedRemoteOp> {
        self.do_execute_local(&local_op.pointer, local_op.op)
    }

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
