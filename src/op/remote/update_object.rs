use object::Element;
use object::UID;

#[derive(Clone,PartialEq,Debug)]
pub struct UpdateObject {
    pub key: String,
    pub new_element: Option<Element>,
    pub deleted_uids: Vec<UID>,
}

impl UpdateObject {
    pub fn new(key: String, new_element: Option<Element>, deleted_uids: Vec<UID>) -> UpdateObject {
        UpdateObject{
            key: key,
            new_element: new_element,
            deleted_uids: deleted_uids,
        }
    }
}
