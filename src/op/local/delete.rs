use std::any::Any;
use op::LocalOp;

pub struct Delete {
    pub path: Vec<i64>,
    pub key: String,
}

impl Delete {
    pub fn new(key: String) -> Delete {
        Delete{path: vec![], key: key}
    }
}

impl LocalOp for Delete {
    fn path(&self) -> &Vec<i64> { &self.path }

    fn as_any(&self) -> &Any { self }
}
