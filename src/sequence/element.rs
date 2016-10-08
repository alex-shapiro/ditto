use sequence::uid::UID;
use sequence::path;
use sequence::path::Path;
use Counter;
use Value;

#[derive(Clone)]
pub enum EltValue {
    None,
    Item(Value),
    Text(String),
    Attr{key: String, value: String},
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
    uid: UID,
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

    pub fn item(value: Value, path: Path, counter: Counter) -> Self {
        Self::new(EltValue::Item(value), path, counter)
    }

    pub fn text(value: String, path: Path, counter: Counter) -> Self {
        Self::new(EltValue::Text(value), path, counter)
    }

    pub fn attr(key: String, value: String, path: Path, counter: Counter) -> Self {
        let attr = EltValue::Attr{key: key, value: value};
        Self::new(attr, path, counter)
    }

    pub fn is_item(&self) -> bool {
        match &self.value {
            &EltValue::Item(_) => true,
            _ => false,
        }
    }

    pub fn is_text(&self) -> bool {
        match &self.value {
            &EltValue::Text(_) => true,
            _ => false,
        }
    }

    pub fn is_attr(&self) -> bool {
        match &self.value {
            &EltValue::Attr{key: _, value: _} => true,
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }
}
