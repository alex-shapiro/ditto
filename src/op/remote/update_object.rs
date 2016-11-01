use super::Reverse;
use object::Element;

#[derive(Clone,PartialEq,Debug)]
pub struct UpdateObject {
    pub key: String,
    pub inserts: Vec<Element>,
    pub deletes: Vec<Element>,
}

impl UpdateObject {
    pub fn new(key: String, new_element: Option<Element>, deleted_elements: Vec<Element>) -> UpdateObject {
        let inserts = match new_element {
            Some(element) => vec![element],
            None => vec![],
        };

        UpdateObject{
            key: key,
            inserts: inserts,
            deletes: deleted_elements,
        }
    }

}

impl Reverse for UpdateObject {
    fn reverse(&self) -> Self {
        UpdateObject{
            key: self.key.clone(),
            inserts: self.deletes.clone(),
            deletes: self.inserts.clone(),
        }
    }
}
