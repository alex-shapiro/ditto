use Value;
use std::any::Any;
use op::LocalOp;

pub struct InsertItem {
    pub path: String,
    pub index: usize,
    pub value: Value,
}

impl InsertItem {
    pub fn new(index: usize, value: Value) -> InsertItem {
        InsertItem{path: String::new(), index: index, value: value}
    }
}

impl LocalOp for InsertItem {
    fn as_any(&self) -> &Any { self }
}
