use std::any::Any;
use op::LocalOp;

pub struct Delete {
    pub path: String,
    pub key: String,
}

impl Delete {
    pub fn new(key: String) -> Delete {
        Delete{path: String::new(), key: key}
    }
}

impl LocalOp for Delete {
    fn as_any(&self) -> &Any { self }
}
