use std::any::Any;
use op::LocalOp;

pub struct DeleteText {
    pub path: Vec<i64>,
    pub index: usize,
    pub len: usize,
}

impl DeleteText {
    pub fn new(index: usize, len: usize) -> DeleteText {
        DeleteText{path: vec![], index: index, len: len}
    }
}

impl LocalOp for DeleteText {
    fn path(&self) -> &Vec<i64> { &self.path }
    fn as_any(&self) -> &Any { self }
}
