use super::Reverse;
use attributed_string::element::Element;

#[derive(Clone,Debug,PartialEq,Default)]
pub struct UpdateAttributedString {
    pub inserts: Vec<Element>,
    pub deletes: Vec<Element>,
}

impl UpdateAttributedString {
    pub fn new(inserts: Vec<Element>, deletes: Vec<Element>) -> Self {
        UpdateAttributedString{
            inserts: inserts,
            deletes: deletes,
        }
    }

    pub fn merge(&mut self, other: UpdateAttributedString) {
        let UpdateAttributedString{mut inserts, mut deletes} = other;
        self.inserts.append(&mut inserts);
        self.deletes.append(&mut deletes);
        self.inserts.sort();
        self.deletes.sort();
    }
}

impl Reverse for UpdateAttributedString {
    fn reverse(&self) -> Self {
        UpdateAttributedString {
            inserts: self.deletes.clone(),
            deletes: self.inserts.clone(),
        }
    }
}
