use super::RemoteOpTrait;
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

impl RemoteOpTrait for UpdateObject {
    fn validate(&self, site: u32) -> bool {
        for i in &self.inserts { if i.uid.site != site { return false } }
        true
    }

    fn update_site(&mut self, site: u32) {
        for i in &mut self.inserts { i.uid.site = site; }
        for d in &mut self.deletes { if d.uid.site == 0 { d.uid.site = site; } }
    }

    fn reverse(&self) -> Self {
        UpdateObject{
            key: self.key.clone(),
            inserts: self.deletes.clone(),
            deletes: self.inserts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate() {
        let op1 = UpdateObject{
            key: String::new(),
            inserts: vec![element(55), element(55)],
            deletes: vec![element(1), element(144)],
        };

        let op2 = UpdateObject{
            key: String::new(),
            inserts: vec![element(55), element(144)],
            deletes: vec![element(1), element(144)],
        };

        assert!(op1.validate(55));
        assert!(!op1.validate(144));
        assert!(!op2.validate(55));
        assert!(!op2.validate(144));
    }

    #[test]
    fn test_update_site() {
        let mut op = UpdateObject{
            key: "foo".to_owned(),
            inserts: vec![element(0), element(0)],
            deletes: vec![element(32), element(0)],
        };

        op.update_site(123);
        assert!(op.inserts[0].uid.site == 123);
        assert!(op.inserts[1].uid.site == 123);
        assert!(op.deletes[0].uid.site == 32);
        assert!(op.deletes[1].uid.site == 123);
    }

    fn element(site: u32) -> Element {
        use replica::Replica;
        use value::Value;
        Element::new("foo", Value::Num(1.0), &Replica::new(site,1))
    }
}
