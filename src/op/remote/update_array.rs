use array::element::Element;
use sequence::uid::UID;


#[derive(Clone,Debug,PartialEq)]
pub struct UpdateArray {
    pub inserts: Vec<Element>,
    pub deletes: Vec<UID>,
    pub deleted_elements: Vec<Element>, // used for reverse execution
}

impl UpdateArray {
    fn new(inserts: Vec<Element>, deleted_elements: Vec<Element>) -> UpdateArray {
        UpdateArray{
            inserts: inserts,
            deletes: deleted_elements.iter().map(|e| e.uid.clone()).collect(),
            deleted_elements: deleted_elements,
        }
    }

    pub fn insert(element: Element) -> UpdateArray {
        UpdateArray::new(vec![element], vec![])
    }

    pub fn delete(element: Element) -> UpdateArray {
        UpdateArray::new(vec![], vec![element])
    }
}
