use array::element::Element;
use sequence::uid::UID;
use op::RemoteOp;

pub struct UpdateArray {
    pub path: Vec<i64>,
    pub inserts: Vec<Element>,
    pub deletes: Vec<UID>,
}

impl UpdateArray {
    fn new(inserts: Vec<Element>, deletes: Vec<UID>) -> UpdateArray {
        UpdateArray{
            path: vec![],
            inserts: inserts,
            deletes: deletes,
        }
    }

    pub fn insert(element: Element) -> UpdateArray {
        UpdateArray::new(vec![element], vec![])
    }

    pub fn delete(uid: UID) -> UpdateArray {
        UpdateArray::new(vec![], vec![uid])
    }
}

impl RemoteOp for UpdateArray { }
