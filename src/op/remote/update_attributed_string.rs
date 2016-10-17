use std::mem;
use attributed_string::element::Element;
use sequence::uid::UID;
use op::RemoteOp;

#[derive(Debug,Default)]
pub struct UpdateAttributedString {
    pub path: Vec<i64>,
    pub inserts: Vec<Element>,
    pub deletes: Vec<UID>,
}

impl UpdateAttributedString {
    pub fn new(inserts: Vec<Element>, deletes: Vec<UID>) -> Self {
        UpdateAttributedString{
            path: vec![],
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
            .partition(|e| !deletes.contains(&e.uid));

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
        self.inserts = inserts;
        self.deletes = deletes;
        self.inserts.sort();
        self.deletes.sort();
    }
}

impl RemoteOp for UpdateAttributedString { }
