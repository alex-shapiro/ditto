use super::RemoteOpTrait;
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

impl RemoteOpTrait for UpdateArray {
    fn validate(&self, site: u32) -> bool {
        for i in &self.inserts {
            if i.uid.site != site { return false }
        }
        true
    }

    fn reverse(&self) -> Self {
        UpdateArray {
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
        let op1 = UpdateArray{
            inserts: vec![element(42), element(42)],
            deletes: vec![element(1), element(32)],
        };

        let op2 = UpdateArray{
            inserts: vec![element(42), element(32)],
            deletes: vec![element(1), element(32)],
        };

        assert!(op1.validate(42));
        assert!(!op1.validate(32));
        assert!(!op2.validate(42));
        assert!(!op2.validate(32));
    }

    fn element(site: u32) -> Element {
        use replica::Replica;
        use value::Value;
        use sequence::uid;
        let uid = uid::UID::between(&uid::UID::min(), &uid::MAX, &Replica::new(site, 1));
        Element::new(Value::Num(1.0), uid)
    }
}
