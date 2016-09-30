use sequence::uid::UID;
use sequence::path;
use sequence::path::Path;
use Value;
use Counter;

pub struct Element {
    uid: UID,
    value: Value,
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
