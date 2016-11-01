use super::Reverse;
use attributed_string::element::Element;
use std::mem;

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

    pub fn merge(&mut self, other: &mut UpdateAttributedString) {
        let inserts = mem::replace(&mut self.inserts, Vec::new());
        let deletes = mem::replace(&mut other.deletes, Vec::new());

        // delete inserts and deletes that negate each other
        let (mut inserts, removed_inserts): (Vec<Element>, Vec<Element>) =
            inserts
                .into_iter()
                .partition(|e| !deletes.contains(&e));

        let mut deletes: Vec<Element> =
            deletes
                .into_iter()
                .filter(|e| !removed_inserts.contains(e))
                .collect();

        inserts.append(&mut other.inserts);
        deletes.append(&mut self.deletes);
        inserts.sort();
        deletes.sort();
        self.inserts = inserts;
        self.deletes = deletes;
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
