use std::any::Any;
use op::LocalOp;

pub struct DeleteItem {
    pub path: String,
    pub index: usize,
}

impl DeleteItem {
    pub fn new(index: usize) -> DeleteItem {
        DeleteItem{path: String::new(), index: index}
    }
}

impl LocalOp for DeleteItem {
    fn as_any(&self) -> &Any { self }
}
