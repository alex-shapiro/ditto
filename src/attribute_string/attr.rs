#[derive(Clone,PartialEq)]
pub struct AttrOpen {
    key: String,
    value: String,
}

impl AttrOpen {
    pub fn new(key: String, value: String) -> Self {
        AttrOpen{key: key, value: value}
    }
}

#[derive(Clone,PartialEq)]
pub struct AttrClose {
    key: String,
}

impl AttrClose {
    pub fn new(key: String) -> Self {
        AttrClose{key: key}
    }
}
