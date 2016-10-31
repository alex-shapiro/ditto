use std::mem;
use attributed_string::element::Element;
use sequence::uid::UID;

#[derive(Clone,Debug,PartialEq,Default)]
pub struct UpdateAttributedString {
    pub inserts: Vec<Element>,
    pub deletes: Vec<UID>,
    pub deleted_elements: Vec<Element>, // used for reverse execution
}

impl UpdateAttributedString {
    pub fn new(inserts: Vec<Element>, deleted_elements: Vec<Element>) -> Self {
        UpdateAttributedString{
            inserts: inserts,
            deletes: deleted_elements.iter().map(|e| e.uid.clone()).collect(),
            deleted_elements: deleted_elements,
        }
    }

    pub fn merge(&mut self, other: &mut UpdateAttributedString) {
        let inserts = mem::replace(&mut self.inserts, Vec::new());
        let deletes = mem::replace(&mut other.deletes, Vec::new());
        let deleted_elements = mem::replace(&mut other.deleted_elements, Vec::new());

        // delete inserts and deletes that negate each other
        let (mut inserts, removed_inserts): (Vec<Element>, Vec<Element>) =
            inserts
            .into_iter()
            .partition(|e| !deletes.contains(&e.uid));

        let mut deleted_elements: Vec<Element> =
            deleted_elements
            .into_iter()
            .filter(|e| removed_inserts.contains(e))
            .collect();

        let removed_insert_uids: Vec<UID> =
            removed_inserts.into_iter()
            .map(|e| e.uid)
            .collect();

        let mut deletes: Vec<UID> =
            deletes
            .into_iter()
            .filter(|uid| !removed_insert_uids.contains(uid))
            .collect();

        inserts.append(&mut other.inserts);
        deletes.append(&mut self.deletes);
        deleted_elements.append(&mut self.deleted_elements);
        self.inserts = inserts;
        self.deletes = deletes;
        self.deleted_elements = deleted_elements;
        self.inserts.sort();
        self.deletes.sort();
    }
}
