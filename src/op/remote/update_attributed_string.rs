use super::RemoteOpTrait;
use attributed_string::element::Element;

#[derive(Clone,Debug,PartialEq,Default)]
pub struct UpdateAttributedString {
    pub inserts: Vec<Element>,
    pub deletes: Vec<Element>,
}

impl UpdateAttributedString {
    pub fn new(inserts: Vec<Element>, deletes: Vec<Element>) -> Self {
        UpdateAttributedString{
            inserts: inserts,
            deletes: deletes,
        }
    }

    pub fn merge(&mut self, other: UpdateAttributedString) {
        let UpdateAttributedString{mut inserts, mut deletes} = other;
        self.inserts.append(&mut inserts);
        self.deletes.append(&mut deletes);
        self.inserts.sort();
        self.deletes.sort();
    }
}

impl RemoteOpTrait for UpdateAttributedString {
    fn validate(&self, site: u32) -> bool {
        for i in &self.inserts {
            if i.uid.site != site { return false }
        }
        true
    }

    fn reverse(&self) -> Self {
        UpdateAttributedString {
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
        let op1 = UpdateAttributedString{
            inserts: vec![element(83), element(83)],
            deletes: vec![element(1), element(77)],
        };

        let op2 = UpdateAttributedString{
            inserts: vec![element(83), element(77)],
            deletes: vec![element(1), element(77)],
        };

        assert!(op1.validate(83));
        assert!(!op1.validate(77));
        assert!(!op2.validate(83));
        assert!(!op2.validate(77));
    }

    fn element(site: u32) -> Element {
        use replica::Replica;
        use sequence::uid;
        let uid = uid::UID::between(&uid::UID::min(), &uid::MAX, &Replica::new(site, 1));
        Element::text(String::new(), uid)
    }
}
