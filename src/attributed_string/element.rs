use std::cmp::Ordering;
use sequence::uid::UID;
use Replica;
use std::fmt;
use std::fmt::Debug;

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

impl Debug for EltValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &EltValue::None => write!(f, "None"),
            &EltValue::Text(ref str) => write!(f, "\"{}\"", str),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Element {
    pub uid: UID,
    value: EltValue,
}

impl Element {
    pub fn new(value: EltValue, uid: UID) -> Self {
        Element{uid: uid, value: value}
    }

    pub fn new_text(text: String, uid: UID) -> Self {
        Element{uid: uid, value: EltValue::Text(text)}
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

    pub fn is_end_marker(&self) -> bool {
        match &self.value {
            &EltValue::None => (self.uid == UID::max()),
            _ => false,
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


    pub fn cut_left(&mut self, index: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.value = EltValue::Text({
            let (_, t) = self.text().unwrap().split_at(index);
            t.to_string()
        });
    }

    pub fn cut_middle(&mut self, lower: usize, upper: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.value = EltValue::Text({
            let original_text = self.text().unwrap();
            let (pre, _)  = original_text.split_at(lower);
            let (_, post) = original_text.split_at(upper);
            let mut text = pre.to_string();
            text.push_str(post);
            text
        });
    }

    pub fn cut_right(&mut self, index: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.value = EltValue::Text({
            let (t, _) = self.text().unwrap().split_at(index);
            t.to_string()
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;
    use sequence::uid::UID;

    #[test]
    fn test_cut_left() {
        let mut elt = Element::new(EltValue::Text("hello world".to_string()), UID::min());
        let replica = Replica{site: 101, counter: 202};
        elt.cut_left(3, &replica);
        assert!(elt.text().unwrap() == "lo world");
        assert!(elt.uid.site == 101);
        assert!(elt.uid.counter == 202);
    }

    #[test]
    fn test_cut_middle() {
        let mut elt = Element::new(EltValue::Text("hello world!".to_string()), UID::min());
        let replica = Replica{site: 8, counter: 999};
        elt.cut_middle(3, 7, &replica);
        assert!(elt.text().unwrap() == "helorld!");
        assert!(elt.uid.site == 8);
        assert!(elt.uid.counter == 999);
    }

    #[test]
    fn test_cut_right() {
        let mut elt = Element::new(EltValue::Text("hello world".to_string()), UID::min());
        let replica = Replica{site: 483, counter: 8328};
        elt.cut_right(6, &replica);
        assert!(elt.text().unwrap() == "hello ");
        assert!(elt.uid.site == 483);
        assert!(elt.uid.counter == 8328);
    }
}
