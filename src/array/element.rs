use sequence::uid::UID;
use std::cmp::Ordering;
use Value;

#[derive(Debug,Clone)]
pub struct Element {
    pub uid: UID,
    pub value: Value,
}

impl Element {
    pub fn new(value: Value, uid: UID) -> Element {
        Element{uid: uid, value: value}
    }

    pub fn start_marker() -> Element {
        Element::new(Value::Null, UID::min())
    }

    pub fn end_marker() -> Element {
        Element::new(Value::Null, UID::max())
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid == other.uid
    }
}

impl Eq for Element { }

impl PartialOrd for Element {
    fn partial_cmp(&self, other: &Element) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for Element {
    fn cmp(&self, other: &Element) -> Ordering {
        self.uid.cmp(&other.uid)
    }
}
