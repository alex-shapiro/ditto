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
    pub static ref START: Element = Element::start_marker();
    pub static ref END: Element = Element::end_marker();
}

impl Element {
    pub fn text(text: String, uid: UID) -> Self {
        Element{uid: uid, len: text.chars().count(), text: text}
    }

    pub fn start_marker() -> Self {
        Element{uid: UID::min(), len: 0, text: String::new()}
    }

    pub fn end_marker() -> Self {
        Element{uid: UID::max(), len: 0, text: String::new()}
    }

    pub fn between(elt1: &Element, elt2: &Element, text: String, replica: &Replica) -> Self {
        Self::text(text, UID::between(&elt1.uid, &elt2.uid, replica))
    }

    #[inline]
    pub fn is_start_marker(&self) -> bool {
        self.uid == *uid::MIN
    }

    #[inline]
    pub fn is_end_marker(&self) -> bool {
        self.uid == *uid::MAX
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        self.len > 0
    }

    pub fn cut_left(&mut self, index: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.text = {
            let (_, t) = self.text.split_at(index);
            t.to_owned()
        };
    }

    pub fn cut_middle(&mut self, lower: usize, upper: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.text = {
            let (pre, _) = self.text.split_at(lower);
            let (_, post) = self.text.split_at(upper);
            let mut text = String::with_capacity(pre.len() + post.len());
            text.push_str(pre);
            text.push_str(post);
            text
        };
    }

    pub fn cut_right(&mut self, index: usize, replica: &Replica) {
        self.uid.set_replica(replica);
        self.text = {
            let (t, _) = self.text.split_at(index);
            t.to_string()
        };
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
        let mut elt = Element::text("hello world".to_owned(), UID::min());
        let replica = Replica{site: 101, counter: 202};
        elt.cut_left(3, &replica);
        assert!(elt.text == "lo world");
        assert!(elt.uid.site == 101);
        assert!(elt.uid.counter == 202);
    }

    #[test]
    fn test_cut_middle() {
        let mut elt = Element::text("hello world!".to_owned(), UID::min());
        let replica = Replica{site: 8, counter: 999};
        elt.cut_middle(3, 7, &replica);
        assert!(elt.text == "helorld!");
        assert!(elt.uid.site == 8);
        assert!(elt.uid.counter == 999);
    }

    #[test]
    fn test_cut_right() {
        let mut elt = Element::text("hello world!".to_owned(), UID::min());
        let replica = Replica{site: 483, counter: 8328};
        elt.cut_right(6, &replica);
        assert!(elt.text == "hello ");
        assert!(elt.uid.site == 483);
        assert!(elt.uid.counter == 8328);
    }
}
