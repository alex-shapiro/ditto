use std::cmp::Ordering;
use sequence::uid::{self, UID};
use Replica;

#[derive(Debug, Clone)]
pub struct Element {
    pub uid: UID,
    pub len: usize,
    pub text: String,
}

lazy_static! {
    pub static ref START: Element = Element{uid: UID::min(), len: 0, text: String::new()};
    pub static ref END: Element = Element{uid: UID::max(), len: 0, text: String::new()};
}

impl Element {
    pub fn text(text: String, uid: UID) -> Self {
        Element{uid: uid, len: text.chars().count(), text: text}
    }

    pub fn between(elt1: &Element, elt2: &Element, text: String, replica: &Replica) -> Self {
        Self::text(text, UID::between(&elt1.uid, &elt2.uid, replica))
    }

    #[inline]
    pub fn is_end_marker(&self) -> bool {
        self.uid == *uid::MAX
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Element) -> bool {
        self.uid.eq(&other.uid)
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
