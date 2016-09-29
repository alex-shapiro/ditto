mod element;
mod uid;

pub use self::element::Element;
pub use self::uid::UID;
use std::collections::HashMap;
use op::LocalOp;
use op::local::Put;
use op::local::Delete;
use op::remote::UpdateObject;
use Counter;
use Site;
use Value;

#[derive(Clone)]
pub struct Object(HashMap<String, Vec<Element>>);

impl Object {
    pub fn new() -> Object {
        Object(HashMap::new())
    }

    pub fn put(&mut self, key: &str, value: Value, site: Site, counter: Counter) -> UpdateObject {
        let mut elements = &mut self.0;
        let new_element = Element::new(key, value, site, counter);
        let deleted_elts = elements.insert(key.to_string(), vec![new_element.clone()]);
        let deleted_uids = uids(deleted_elts);
        UpdateObject::new(key.to_string(), Some(new_element), deleted_uids)
    }

    pub fn delete(&mut self, key: &str) -> UpdateObject {
        let mut elements = &mut self.0;
        let deleted_elts = elements.remove(key);
        let deleted_uids = uids(deleted_elts);
        UpdateObject::new(key.to_string(), None, deleted_uids)
    }

    pub fn get(&mut self, key: &str) -> Option<&mut Element> {
        let key_elements: Option<&mut Vec<Element>> = self.0.get_mut(key);
        match key_elements {
            None =>
                None,
            Some(key_elements) =>
                key_elements.iter_mut().min_by_key(|e| e.uid.clone()),
        }
    }

    pub fn replace(&mut self, key: &str, value: Value) -> bool {
        match self.get(key) {
            None =>
                false,
            Some(element) => {
                element.value = value;
                true},
        }
    }

    pub fn execute_remote(&mut self, op: UpdateObject) -> Box<LocalOp> {
        let mut elements = &mut self.0;
        let deleted_uids = op.deleted_uids;
        let default: Vec<Element> = vec![];
        let mut key_elements: Vec<Element> =
            elements
            .get(&op.key)
            .unwrap_or(&default)
            .iter()
            .filter(|e| !deleted_uids.contains(&e.uid))
            .map(|e| e.clone())
            .collect();

        let key = op.key;
        let local_op: Box<LocalOp> =
            match op.new_element {
                Some(element) => {
                    key_elements.push(element.clone());
                    Box::new(Put::new(key.clone(), element.value))},
                None =>
                    Box::new(Delete::new(key.clone())),
            };
        elements.insert(key, key_elements);
        local_op
    }
}

impl PartialEq for Object {
    fn eq(&self, _: &Object) -> bool { false }
}

fn uids(elements: Option<Vec<Element>>) -> Vec<UID> {
    match elements {
        None =>
            vec![],
        Some(elts) =>
            elts.iter().map(|e| e.uid.clone()).collect(),
    }
}

#[test]
fn new() {
    let object = Object::new();
    assert!(object.0.len() == 0);
}
