use object::Element;
use object::UID;

#[derive(Clone,PartialEq,Debug)]
pub struct UpdateObject {
    pub key: String,
    pub new_element: Option<Element>,
    pub deleted_uids: Vec<UID>,
    pub deleted_elements: Vec<Element>, // used for reverse execution
}

impl UpdateObject {
    pub fn new(key: String, new_element: Option<Element>, deleted_elements: Vec<Element>) -> UpdateObject {
        UpdateObject{
            key: key,
            new_element: new_element,
            deleted_uids: deleted_elements.iter().map(|e| e.uid.clone()).collect(),
            deleted_elements: deleted_elements,
        }
    }
}
