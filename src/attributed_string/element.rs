use sequence::uid::UID;
use Replica;

#[derive(Clone)]
pub enum EltValue {
    None,
    Text(String),
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

    pub fn is_marker(&self) -> bool {
        match &self.value {
            &EltValue::None => true,
            _ => false
        }
    }

    pub fn is_end_marker(&self) -> bool {
        match &self.value {
            &EltValue::None => (self.uid == UID::max()),
            _ => false,
        }
    }

    pub fn is_text(&self) -> bool {
        match &self.value {
            &EltValue::Text(_) => true,
            _ => false
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.value {
            &EltValue::Text(ref str) => Some(str),
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
