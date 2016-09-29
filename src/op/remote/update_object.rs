use object::Element;
use object::UID;

pub struct UpdateObject {
    pub path: Vec<i64>,
    pub key: String,
    pub new_element: Option<Element>,
    pub deleted_uids: Vec<UID>,
}

impl UpdateObject {
    pub fn new(key: String, new_element: Option<Element>, deleted_uids: Vec<UID>) -> UpdateObject {
        UpdateObject{
            path: vec![],
            key: key,
            new_element: new_element,
            deleted_uids: deleted_uids,
        }
    }
}
