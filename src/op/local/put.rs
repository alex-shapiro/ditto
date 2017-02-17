use Value;

#[derive(Serialize, Deserialize)]
pub struct Put {
    pub key: String,
    pub value: Value,
}

impl Put {
    pub fn new(key: String, value: Value) -> Put {
        Put{key: key, value: value}
    }
}
