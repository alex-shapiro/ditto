use array::element::Element;
use sequence::uid::UID;


#[derive(Clone,Debug,PartialEq)]
pub struct UpdateArray {
    pub inserts: Vec<Element>,
    pub deletes: Vec<UID>,
}

impl UpdateArray {
    fn new(inserts: Vec<Element>, deletes: Vec<UID>) -> UpdateArray {
        UpdateArray{
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
