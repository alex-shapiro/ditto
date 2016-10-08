use sequence::uid::UID;
use sequence::path;
use sequence::path::Path;
use Counter;
use Value;

#[derive(Clone)]
pub struct Element {
    pub uid: UID,
    pub value: Value,
}

impl Element {
    pub fn new(value: Value, path: Path, counter: Counter) -> Element {
        Element{uid: UID::new(path, counter), value: value}
    }

    pub fn start_marker() -> Element {
        Element::new(Value::Null, path::min(), 0)
    }

    pub fn end_marker() -> Element {
        Element::new(Value::Null, path::max(), 0)
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid == other.uid
    }
}
