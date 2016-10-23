use std::any::Any;
use op::LocalOp;

pub struct DeleteText {
    pub path: String,
    pub index: usize,
    pub len: usize,
}

impl DeleteText {
    pub fn new(index: usize, len: usize) -> DeleteText {
        DeleteText{path: String::new(), index: index, len: len}
    }
}

impl LocalOp for DeleteText {
    fn as_any(&self) -> &Any { self }
}
