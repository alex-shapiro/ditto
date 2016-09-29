mod element;
mod uid;

pub use self::element::Element;
pub use self::uid::UID;
use std::collections::HashMap;
use op::remote::UpdateObject;
use op::Local;
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

    pub fn execute_remote(&mut self, op: UpdateObject) -> Local {
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
        let local_op =
            match op.new_element {
                Some(element) => {
                    key_elements.push(element.clone());
                    Local::put(key.clone(), element.value)},
                None =>
                    Local::delete(key.clone()),
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
