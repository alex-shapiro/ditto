use LocalValue;

#[derive(Serialize, Deserialize)]
pub struct Put {
    pub key: String,
    pub value: LocalValue,
}

impl Put {
    pub fn new(key: String, value: LocalValue) -> Put {
        Put{key: key, value: value}
    }
}
