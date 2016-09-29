use Value;
use op::LocalOp;

pub struct Put {
    pub path: Vec<i64>,
    pub key: String,
    pub value: Value,
}

impl Put {
    pub fn new(key: String, value: Value) -> Put {
        Put{path: vec![], key: key, value: value}
    }
}

impl LocalOp for Put { }
