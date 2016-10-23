pub struct Delete {
    pub path: String,
    pub key: String,
}

impl Delete {
    pub fn new(key: String) -> Delete {
        Delete{path: String::new(), key: key}
    }
}
