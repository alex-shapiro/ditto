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

    pub fn execute_remote(&mut self, op: &mut UpdateObject) -> LocalOp {
        let key_elements = {
            let mut empty_vec = Vec::new();
            let key_elements_ref = self.0.get_mut(&op.key).unwrap_or(&mut empty_vec);
            mem::replace(key_elements_ref, vec![])
        };

        // remove op.deleted_uids and modify op.deleted_elements
        let deleted_uids = &op.deleted_uids;
        let (mut new_key_elements, deleted_elements): (Vec<Element>, _) =
            key_elements
                .into_iter()
                .partition(|e| !deleted_uids.contains(&e.uid));

        // insert the new element if it exists
        if let Some(ref e) = op.new_element {
            new_key_elements.push(e.clone());
        }

        op.deleted_elements = deleted_elements;
        match new_key_elements.len() {
            0 => {
                self.0.remove(&op.key);
                LocalOp::Delete(Delete::new(op.key.to_owned()))
            },
            _ => {
                self.0.insert(op.key.clone(), new_key_elements);
                let elt = self.get_by_key(&op.key).expect("execute obj");
                LocalOp::Put(Put::new(op.key.clone(), elt.value.clone()))
            },
        }
    }

    pub fn reverse_execute_remote(&mut self, op: &UpdateObject) -> LocalOp {
        let mut key_elements = {
            let mut empty_vec = Vec::new();
            let mut key_elements_ref = self.0.get_mut(&op.key).unwrap_or(&mut empty_vec);
            mem::replace(key_elements_ref, vec![])
        };

        // remove op.new_element
        key_elements = match op.new_element {
            Some(ref e) => key_elements.into_iter().filter(|elt| elt != e).collect(),
            None => key_elements,
        };

        // add op.deleted_elements
        for elt in &op.deleted_elements {
            key_elements.push(elt.clone())
        }

        match key_elements.len() {
            0 => {
                self.0.remove(&op.key);
                LocalOp::Delete(Delete::new(op.key.to_string()))
            },
            _ => {
                self.0.insert(op.key.clone(), key_elements);
                let elt = self.get_by_key(&op.key).expect("reverse execute obj");
                LocalOp::Put(Put::new(op.key.clone(), elt.value.clone()))
            },
        }
    }

    pub fn elements(&self) -> &HashMap<String,Vec<Element>> {
        &self.0
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

        assert!(op.key == "bar".to_string());
        assert!(op.new_element == None);
        assert!(op.deleted_uids.len() == 1);
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
        assert!(remote_op.deleted_elements.len() == 0);
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
        assert!(remote_op3.deleted_elements[0].value == Value::Bool(false));
    }

    #[test]
    fn test_reverse_execute_remote_put_new() {
        let mut object = Object::new();
        let remote_op  = object.put("foo", Value::Num(1.0), &Replica::new(34,43));
        let local_op   = object.reverse_execute_remote(&remote_op);

        assert!(object.get_by_key("foo") == Err(Error::KeyDoesNotExist));
        assert!(local_op.delete().unwrap().key == "foo");
    }

    #[test]
    fn test_reverse_execute_remote_put_replace() {
        let mut object = Object::new();
        let _          = object.put("foo", Value::Num(1.0), &Replica::new(1,1));
        let remote_op2 = object.put("foo", Value::Num(2.0), &Replica::new(1,2));
        let local_op   = object.reverse_execute_remote(&remote_op2);

        assert!(object.get_by_key("foo").ok().unwrap().value == Value::Num(1.0));
        assert!(local_op.put().unwrap().key == "foo");
        assert!(local_op.put().unwrap().value == Value::Num(1.0));
    }

    #[test]
    fn test_reverse_execute_remote_delete_some() {
        let mut object = Object::new();
        let element1 = Element::new("foo", Value::Num(1.0), &Replica::new(1,1));
        let element2 = Element::new("foo", Value::Num(2.0), &Replica::new(2,1));
        let _ = object.execute_remote(&mut UpdateObject::new("foo".to_owned(), Some(element2.clone()), vec![]));
        let remote_op = UpdateObject::new("foo".to_owned(), None, vec![element1]);

        assert!(object.get_by_key("foo").ok().unwrap().value == Value::Num(2.0));
        let local_op = object.reverse_execute_remote(&remote_op);
        assert!(object.get_by_key("foo").ok().unwrap().value == Value::Num(1.0));
        assert!(local_op.put().unwrap().key == "foo");
        assert!(local_op.put().unwrap().value == Value::Num(1.0));
    }

    #[test]
    fn test_reverse_execute_remote_delete_all() {
        let mut object = Object::new();
        let element = Element::new("foo", Value::Num(1.0), &Replica::new(1,1));
        let remote_op = UpdateObject::new("foo".to_owned(), None, vec![element]);

        assert!(object.get_by_key("foo") == Err(Error::KeyDoesNotExist));
        let local_op = object.reverse_execute_remote(&remote_op);
        assert!(object.get_by_key("foo").ok().unwrap().value == Value::Num(1.0));
        assert!(local_op.put().unwrap().key == "foo");
        assert!(local_op.put().unwrap().value == Value::Num(1.0));
    }
}
