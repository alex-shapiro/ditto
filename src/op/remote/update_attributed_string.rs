use attributed_string::element::Element;
use sequence::uid::UID;
use op::RemoteOp;

#[derive(Debug)]
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
        self.inserts.append(&mut other.inserts);
        self.deletes.append(&mut other.deletes);
        self.inserts.sort();
        self.deletes.sort();
    }
}

impl RemoteOp for UpdateAttributedString { }
