use Value;

pub struct Put {
    pub path: String,
    pub key: String,
    pub value: Value,
}

impl Put {
    pub fn new(key: String, value: Value) -> Put {
        Put{path: String::new(), key: key, value: value}
    }
}
