use sequence::uid::UID;
use sequence::path;
use sequence::path::Path;
use Value;
use Counter;

#[derive(Clone)]
pub struct Element {
    pub uid: UID,
    pub value: Value,
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid == other.uid
    }
}

impl Element {
    pub fn new(value: Value, path: Path, counter: Counter) -> Element {
        let uid = UID{path: path, counter: counter};
        Element{uid: uid, value: value}
    }

    pub fn start_marker() -> Element {
        Element::new(Value::Null, path::min(), 0)
    }

    pub fn end_marker() -> Element {
        Element::new(Value::Null, path::max(), 0)
    }
}
