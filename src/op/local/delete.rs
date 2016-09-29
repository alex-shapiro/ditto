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

impl LocalOp for Delete { }
