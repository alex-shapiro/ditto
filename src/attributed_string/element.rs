use super::attr::{AttrOpen,AttrClose};
use sequence::uid::UID;
use Replica;

#[derive(Clone)]
pub enum EltValue {
    None,
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
    pub fn new(value: EltValue, uid: UID) -> Self {
        Element{uid: uid, value: value}
    }

    pub fn start_marker() -> Self {
        Self::new(EltValue::None, UID::min())
    }

    pub fn end_marker() -> Self {
        Self::new(EltValue::None, UID::max())
    }

    pub fn between(elt1: &Element, elt2: &Element, text: String, replica: &Replica) -> Self {
        let uid = UID::between(&elt1.uid, &elt2.uid, replica);
        Self::new(EltValue::Text(text), uid)
    }

    pub fn new_text(value: String, uid: UID) -> Self {
        Self::new(EltValue::Text(value), uid)
    }

    pub fn new_attr_open(key: String, value: String, uid: UID) -> Self {
        let attr_open = AttrOpen::new(key, value);
        Self::new(EltValue::AttrOpen(attr_open), uid)
    }

    pub fn new_attr_close(key: String, uid: UID) -> Self {
        let attr_close = AttrClose::new(key);
        Self::new(EltValue::AttrClose(attr_close), uid)
    }

    pub fn is_marker(&self) -> bool {
        match &self.value {
            &EltValue::None => true,
            _ => false
        }
    }

    pub fn is_text(&self) -> bool {
        match &self.value {
            &EltValue::Text(_) => true,
            _ => false
        }
    }

    pub fn is_attr_open(&self) -> bool {
        match &self.value {
            &EltValue::AttrOpen(_) => true,
            _ => false
        }
    }

    pub fn is_attr_close(&self) -> bool {
        match &self.value {
            &EltValue::AttrClose(_) => true,
            _ => false
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
