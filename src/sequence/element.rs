use sequence::attr::{AttrOpen,AttrClose};
use sequence::path;
use sequence::path::Path;
use sequence::uid::UID;
use Counter;
use Value;

#[derive(Clone)]
pub enum EltValue {
    None,
    Item(Value),
    Text(String),
    AttrOpen(AttrOpen),
    AttrClose(AttrClose),
}

impl EltValue {
    pub fn len(&self) -> usize {
        match self {
            &EltValue::Text(ref str) => str.len(),
            _ => 0,
        }
    }
}

#[derive(Clone)]
pub struct Element {
    pub uid: UID,
    value: EltValue,
}

impl Element {
    fn new(value: EltValue, path: Path, counter: Counter) -> Self {
        Element{uid: UID::new(path, counter), value: value}
    }

    pub fn start_marker() -> Self {
        Self::new(EltValue::None, path::min(), 0)
    }

    pub fn end_marker() -> Self {
        Self::new(EltValue::None, path::max(), 0)
    }

    pub fn new_item(value: Value, path: Path, counter: Counter) -> Self {
        Self::new(EltValue::Item(value), path, counter)
    }

    pub fn new_text(value: String, path: Path, counter: Counter) -> Self {
        Self::new(EltValue::Text(value), path, counter)
    }

    pub fn new_attr_open(key: String, value: String, path: Path, counter: Counter) -> Self {
        let attr_open = AttrOpen::new(key, value);
        Self::new(EltValue::AttrOpen(attr_open), path, counter)
    }

    pub fn new_attr_close(key: String, path: Path, counter: Counter) -> Self {
        let attr_close = AttrClose::new(key);
        Self::new(EltValue::AttrClose(attr_close), path, counter)
    }

    pub fn item(&self) -> Option<&Value> {
        match &self.value {
            &EltValue::Item(ref item) => Some(item),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.value {
            &EltValue::Text(ref str) => Some(str),
            _ => None,
        }
    }

    pub fn attr_open(&self) -> Option<&AttrOpen> {
        match &self.value {
            &EltValue::AttrOpen(ref attr_open) => Some(attr_open),
            _ => None,
        }
    }

    pub fn attr_close(&self) -> Option<&AttrClose> {
        match &self.value {
            &EltValue::AttrClose(ref attr_close) => Some(attr_close),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid == other.uid
    }
}
