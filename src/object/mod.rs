pub mod element;
pub mod uid;

pub use self::element::Element;
pub use self::uid::UID;
use Error;
use op::local::{LocalOp, Put, Delete};
use op::remote::UpdateObject;
use Replica;
use std::collections::HashMap;
use std::mem;
use Value;

#[derive(Debug,Clone,PartialEq)]
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
        UpdateObject::new(key.to_string(), Some(new_element), deleted_elts.unwrap_or(vec![]))
    }

    pub fn delete(&mut self, key: &str) -> Result<UpdateObject, Error> {
        let mut elements = &mut self.0;
        let deleted_elts = elements.remove(key).unwrap_or(vec![]);
        if deleted_elts.is_empty() {
            Err(Error::Noop)
        } else {
            Ok(UpdateObject::new(key.to_string(), None, deleted_elts))
        }
    }

    pub fn get_by_key(&mut self, key: &str) -> Result<&mut Element, Error> {
        let key_elements: Option<&mut Vec<Element>> = self.0.get_mut(key);
        match key_elements {
            None =>
                Err(Error::KeyDoesNotExist),
            Some(elements) => {
                match elements.iter_mut().min_by_key(|e| e.uid.site) {
                    Some(elt) => Ok(elt),
                    None => Err(Error::KeyDoesNotExist),
                }
            }
        }
    }

    pub fn get_by_uid(&mut self, uid: &UID) -> Result<&mut Element, Error> {
        let key_elements = self.0.get_mut(&uid.key);
        match key_elements {
            None =>
                Err(Error::UIDDoesNotExist),
            Some(key_elements) => {
                match key_elements.binary_search_by(|elt| elt.uid.cmp(uid)) {
                    Ok(index) => Ok(&mut key_elements[index]),
                    Err(_) => Err(Error::UIDDoesNotExist),
                }
            }
        }
    }

    pub fn execute_remote(&mut self, op: &UpdateObject) -> LocalOp {
        let key_elements = {
            let mut empty_vec = Vec::new();
            let key_elements_ref = self.0.get_mut(&op.key).unwrap_or(&mut empty_vec);
            mem::replace(key_elements_ref, vec![])
        };

        // remote op deletes
        let mut new_key_elements: Vec<Element> =
            key_elements
                .into_iter()
                .filter(|e| !op.deletes.contains(&e))
                .collect();

        // add op inserts
        for element in &op.inserts {
            new_key_elements.push(element.clone());
        }

        if new_key_elements.is_empty() {
            self.0.remove(&op.key);
            LocalOp::Delete(Delete::new(op.key.to_owned()))
        } else {
            self.0.insert(op.key.clone(), new_key_elements);
            let element = self.get_by_key(&op.key).expect("key must have elements!");
            LocalOp::Put(Put::new(op.key.to_owned(), element.value.clone()))
        }
    }

    pub fn elements(&self) -> &HashMap<String,Vec<Element>> {
        &self.0
    }

    pub fn elements_vec<'a>(&'a self) -> Vec<&'a Element> {
        let mut vec = vec![];
        for (_, elements) in &self.0 {
            for e in elements { vec.push(e); }
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Error;
    use op::remote::UpdateObject;
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

        assert!(op.key == "foo".to_string());
        assert!(op.inserts[0].uid == UID::new("foo", &REPLICA));
        assert!(op.deletes.is_empty());

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

        assert!(op.key == "bar".to_string());
        assert!(op.inserts.is_empty());
        assert!(op.deletes.len() == 1);
        assert!(object.get_by_key("bar") == Err(Error::KeyDoesNotExist));
    }

    #[test]
    fn test_delete_no_values_for_key() {
        let mut object = Object::new();
        assert!(object.delete("foo") == Err(Error::Noop));
    }

    #[test]
    fn test_execute_remote_put() {
        let mut object = Object::new();
        let _ = object.put("baz", Value::Num(0.0), &Replica::new(3,69));
        let element = Element::new("baz", Value::Num(1.0), &Replica::new(2,101));
        let mut remote_op = UpdateObject::new("baz".to_string(), Some(element), vec![]);
        let local_op = object.execute_remote(&mut remote_op);

        assert!(local_op.put().unwrap().key == "baz".to_owned());
        assert!(local_op.put().unwrap().value == Value::Num(1.0));
        assert!(object.0.get("baz").unwrap().len() == 2);
        assert!(remote_op.deletes.is_empty());
    }

    #[test]
    fn test_execute_remote_delete() {
        let mut object = Object::new();
        let elt1 = Element::new("foo", Value::Bool(false), & Replica::new(1,1));
        let elt2 = Element::new("foo", Value::Bool(true), &Replica::new(2,1));
        let mut remote_op1 = UpdateObject::new("foo".to_string(), Some(elt1.clone()), vec![]);
        let mut remote_op2 = UpdateObject::new("foo".to_string(), Some(elt2.clone()), vec![]);
        let mut remote_op3 = UpdateObject::new("foo".to_string(), None, vec![elt1]);
        object.execute_remote(&mut remote_op1);
        object.execute_remote(&mut remote_op2);

        assert!(object.get_by_key("foo").unwrap().value == Value::Bool(false));
        let local_op = object.execute_remote(&mut remote_op3);
        assert!(local_op.put().unwrap().key == "foo".to_owned());
        assert!(local_op.put().unwrap().value == Value::Bool(true));
        assert!(remote_op3.deletes[0].value == Value::Bool(false));
    }
}
