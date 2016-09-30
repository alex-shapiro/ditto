use Value;
use std::any::Any;
use op::LocalOp;

pub struct InsertItem {
    pub path: Vec<i64>,
    pub index: usize,
    pub value: Value,
}

impl InsertItem {
    pub fn new(index: usize, value: Value) -> InsertItem {
        InsertItem{path: vec![], index: index, value: value}
    }
}

impl LocalOp for InsertItem {
    fn path(&self) -> &Vec<i64> { &self.path }
    fn as_any(&self) -> &Any { self }
}
