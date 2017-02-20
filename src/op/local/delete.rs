#[derive(Serialize, Deserialize)]
pub struct Delete {
    pub key: String,
}

impl Delete {
    pub fn new(key: String) -> Delete {
        Delete{key: key}
    }
}
