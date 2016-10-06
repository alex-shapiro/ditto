use std::any::Any;
use op::LocalOp;

pub struct DeleteItem {
    pub path: Vec<i64>,
    pub index: usize,
}

impl DeleteItem {
    pub fn new(index: usize) -> DeleteItem {
        DeleteItem{path: vec![], index: index}
    }
}

impl LocalOp for DeleteItem {
    fn path(&self) -> &Vec<i64> { &self.path }
    fn as_any(&self) -> &Any { self }
}
