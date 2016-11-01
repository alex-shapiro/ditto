use super::Reverse;
use array::element::Element;

#[derive(Clone,Debug,PartialEq)]
pub struct UpdateArray {
    pub inserts: Vec<Element>,
    pub deletes: Vec<Element>,
}

impl UpdateArray {
    fn new(inserts: Vec<Element>, deletes: Vec<Element>) -> UpdateArray {
        UpdateArray{
            inserts: inserts,
            deletes: deletes,
        }
    }

    pub fn insert(element: Element) -> UpdateArray {
        UpdateArray::new(vec![element], vec![])
    }

    pub fn delete(element: Element) -> UpdateArray {
        UpdateArray::new(vec![], vec![element])
    }
}

impl Reverse for UpdateArray {
    fn reverse(&self) -> Self {
        UpdateArray {
            inserts: self.deletes.clone(),
            deletes: self.inserts.clone(),
        }
    }
}
