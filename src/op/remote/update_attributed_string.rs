use attributed_string::element::Element;
use sequence::uid::UID;
use op::RemoteOp;

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
}

impl RemoteOp for UpdateAttributedString { }
