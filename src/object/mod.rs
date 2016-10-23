pub mod element;
pub mod uid;

pub use self::element::Element;
pub use self::uid::UID;
use std::collections::HashMap;
use op::LocalOp;
use op::local::{Put,Delete};
use op::remote::UpdateObject;
use Replica;
use Value;

#[derive(Clone,PartialEq)]
pub struct Object(HashMap<String, Vec<Element>>);

impl Object {
    pub fn new() -> Object {
        Object(HashMap::new())
    }

    pub fn assemble(map: HashMap<String,Vec<Element>>) -> Self {
        Object(map)
    }

    pub fn put(&mut self, key: &str, value: Value, replica: &Replica) -> UpdateObject {
        let mut elements = &mut self.0;
        let new_element = Element::new(key, value, replica);
        let deleted_elts = elements.insert(key.to_string(), vec![new_element.clone()]);
        let deleted_uids = uids(deleted_elts);
        UpdateObject::new(key.to_string(), Some(new_element), deleted_uids)
    }

    pub fn delete(&mut self, key: &str) -> Option<UpdateObject> {
        let mut elements = &mut self.0;
        let deleted_elts = elements.remove(key);
        let deleted_uids = uids(deleted_elts);
        if deleted_uids.is_empty() {
            None
        } else {
            Some(UpdateObject::new(key.to_string(), None, deleted_uids))
        }
    }

    pub fn get_by_key(&mut self, key: &str) -> Option<&mut Element> {
        let key_elements = self.0.get_mut(key);
        match key_elements {
            None =>
                None,
            Some(elements) =>
                elements.iter_mut().min_by_key(|e| e.uid.site),
        }
    }

    pub fn get_by_uid(&mut self, uid: &UID) -> Option<&mut Element> {
        let key_elements = self.0.get_mut(&uid.key);
        match key_elements {
            None =>
                None,
            Some(key_elements) =>
                key_elements.iter_mut().find(|e| &e.uid == uid),
        }
    }

    pub fn replace_by_uid(&mut self, uid: &UID, value: Value) -> bool {
        match self.get_by_uid(uid) {
            None =>
                false,
            Some(element) => {
                element.value = value;
                true},
        }
    }

    pub fn execute_remote(&mut self, op: UpdateObject) -> Box<LocalOp> {
        let mut key_elements: Vec<Element> = {
            let deleted_uids = op.deleted_uids;
            let default: Vec<Element> = vec![];
            self.0
                .get(&op.key)
                .unwrap_or(&default)
                .iter()
                .filter(|e| !deleted_uids.contains(&e.uid))
                .map(|e| e.clone())
                .collect()
        };

        op.new_element.map(|e| key_elements.push(e));
        let key = op.key;
        match key_elements.len() > 0 {
            true => {
                self.0.insert(key.clone(), key_elements);
                let elt = self.get_by_key(&key).unwrap();
                Box::new(Put::new(key, elt.value.clone()))},
            false => {
                self.0.remove(&key);
                Box::new(Delete::new(key))},
        }
    }

    pub fn elements(&self) -> &HashMap<String,Vec<Element>> {
        &self.0
    }
}

fn uids(elements: Option<Vec<Element>>) -> Vec<UID> {
    match elements {
        None =>
            vec![],
        Some(elts) =>
            elts.iter().map(|e| e.uid.clone()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op::remote::UpdateObject;
    use op::local::Put;
    use Replica;
    use Value;

    const REPLICA: Replica = Replica{site: 1, counter: 2};

    #[test]
    fn test_new() {
        let object = Object::new();
        assert!(object.0.len() == 0);
    }

    #[test]
    fn test_put() {
        let mut object = Object::new();
        let op = object.put("foo", Value::Num(23.0), &REPLICA);

        assert!(op.path == vec![]);
        assert!(op.key == "foo".to_string());
        assert!(op.new_element.unwrap().uid == UID::new("foo", &REPLICA));
        assert!(op.deleted_uids == vec![]);

        assert!(object.0.get("foo").unwrap().len() == 1);
        {
            let element = object.get_by_key("foo").unwrap();
            assert!(element.value == Value::Num(23.0));
        }
    }

    #[test]
    fn test_delete() {
        let mut object = Object::new();
        let replica = Replica::new(2,4);
        let _  = object.put("bar", Value::Bool(true), &replica);
        let op = object.delete("bar").unwrap();

        assert!(op.path == vec![]);
        assert!(op.key == "bar".to_string());
        assert!(op.new_element == None);
        assert!(op.deleted_uids.len() == 1);
        assert!(object.get_by_key("bar") == None);
    }

    #[test]
    fn test_delete_no_values_for_key() {
        let mut object = Object::new();
        assert!(None == object.delete("foo"));
    }

    #[test]
    fn test_execute_remote() {
        let mut object = Object::new();
        let replica1 = Replica::new(2,101);
        let replica2 = Replica::new(3,69);
        let elt = Element::new("baz", Value::Num(1.0), &replica1);
        let _   = object.put("baz", Value::Num(0.0), &replica2);
        let op2 = UpdateObject::new("baz".to_string(), Some(elt), vec![]);
        let op3 = object.execute_remote(op2);
        let op3_unwrapped = op3.as_any().downcast_ref::<Put>().unwrap();

        assert!(op3_unwrapped.path == vec![]);
        assert!(op3_unwrapped.key == "baz".to_string());
        assert!(op3_unwrapped.value == Value::Num(1.0));
        assert!(object.0.get("baz").unwrap().len() == 2);
    }

    #[test]
    fn test_execute_remote_2() {
        let mut object = Object::new();
        let replica1 = Replica::new(1,1);
        let replica2 = Replica::new(2,1);
        let elt1 = Element::new("foo", Value::Bool(false), &replica1);
        let elt2 = Element::new("foo", Value::Bool(true), &replica2);
        let op1 = UpdateObject::new("foo".to_string(), Some(elt1.clone()), vec![]);
        let op2 = UpdateObject::new("foo".to_string(), Some(elt2.clone()), vec![]);
        let op3 = UpdateObject::new("foo".to_string(), None, vec![elt1.uid]);

        object.execute_remote(op1);
        object.execute_remote(op2);
        { assert!(object.get_by_key("foo").unwrap().value == Value::Bool(false)) }

        let op4 = object.execute_remote(op3);
        let op4_unwrapped = op4.as_any().downcast_ref::<Put>().unwrap();

        assert!(op4_unwrapped.path == vec![]);
        assert!(op4_unwrapped.key == "foo".to_string());
        assert!(op4_unwrapped.value == Value::Bool(true));
    }

    #[test]
    fn test_replace_by_uid() {
        let mut object = Object::new();
        let replica = Replica::new(1,1);
        let op1 = object.put("foo", Value::Num(1.0), &replica);
        let uid = op1.new_element.unwrap().uid;
        assert!(object.replace_by_uid(&uid, Value::Bool(true)));
        assert!(object.get_by_uid(&uid).unwrap().value == Value::Bool(true));
    }
}
