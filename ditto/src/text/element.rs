use char_fns::CharFns;
use order_statistic_tree;
use Replica;
use sequence::uid::UID;
use std::cmp::Ordering;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Serialize)]
pub struct Element {
    pub uid: UID,
    pub text: String,
    #[serde(skip_serializing)]
    pub len: usize,
}

#[derive(Debug, Deserialize)]
struct DeserializedElement {
    pub uid: UID,
    pub text: String,
}

lazy_static! {
    pub static ref START: Element = Element{uid: UID::min(), len: 0, text: String::new()};
    pub static ref END: Element = Element{uid: UID::max(), len: 0, text: String::new()};
}

impl Element {
    pub fn text(text: String, uid: UID) -> Self {
        Element{uid: uid, len: text.char_len(), text: text}
    }

    pub fn between(elt1: &Element, elt2: &Element, text: String, replica: &Replica) -> Self {
        Self::text(text, UID::between(&elt1.uid, &elt2.uid, *replica))
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

impl order_statistic_tree::Element for Element {
    type Id = UID;

    fn id(&self) -> &UID {
        &self.uid
    }

    fn element_len(&self) -> usize {
        self.len
    }
}

impl<'de> Deserialize<'de> for Element {
    fn deserialize<D>(deserializer: D) -> Result<Element, D::Error> where D: Deserializer<'de> {
        let DeserializedElement{uid, text} = DeserializedElement::deserialize(deserializer)?;
        let len = text.char_len();
        Ok(Element{uid, text, len})
    }
}
