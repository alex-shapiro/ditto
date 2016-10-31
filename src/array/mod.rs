pub mod element;

use self::element::Element;
use Error;
use op::local::{DeleteItem, InsertItem, LocalOp};
use op::remote::UpdateArray;
use Replica;
use sequence::uid::UID;
use std::mem;
use Value;

#[derive(Debug,Clone,PartialEq)]
pub struct Array(Vec<Element>);

impl Array {
    pub fn new() -> Array {
        Array(vec![Element::start_marker(), Element::end_marker()])
    }

    pub fn assemble(elements: Vec<Element>) -> Array {
        Array(elements)
    }

    pub fn len(&self) -> usize {
        self.0.len() - 2
    }

    pub fn insert(&mut self, index: usize, value: Value, replica: &Replica) -> Result<UpdateArray, Error> {
        if index <= self.len() {
            let ref mut elements = self.0;
            let uid = {
                let ref uid1 = elements[index].uid;
                let ref uid2 = elements[index+1].uid;
                UID::between(uid1, uid2, replica)
            };

            let element = Element::new(value, uid);
            elements.insert(index+1, element.clone());
            Ok(UpdateArray::insert(element))
        } else {
            Err(Error::OutOfBounds)
        }
    }

    pub fn delete(&mut self, index: usize) -> Result<UpdateArray, Error> {
        if index < self.len() {
            let element = self.0.remove(index+1);
            Ok(UpdateArray::delete(element))
        } else {
            Err(Error::OutOfBounds)
        }
    }

    pub fn get_by_index(&mut self, index: usize) -> Result<&mut Element, Error> {
        if index >= self.len() { return Err(Error::OutOfBounds) }
        let ref mut element = self.0[index+1];
        Ok(element)
    }

    pub fn get_by_uid(&mut self, uid: &UID) -> Result<(&mut Element, usize), Error> {
        match self.0.binary_search_by(|elt| elt.uid.cmp(uid)) {
            Ok(index) => Ok((&mut self.0[index], index - 1)),
            Err(_) => Err(Error::UIDDoesNotExist),
        }
    }

    pub fn execute_remote(&mut self, op: &UpdateArray) -> Vec<LocalOp> {
        let delete_ops: Vec<DeleteItem> =
            op.deletes.iter()
            .map(|uid| self.delete_remote(&uid))
            .filter(|op| op.is_some())
            .map(|op| op.unwrap())
            .collect();

        let insert_ops: Vec<InsertItem> =
            op.inserts.iter()
            .map(|elt| self.insert_remote(elt.clone()))
            .filter(|op| op.is_some())
            .map(|op| op.unwrap())
            .collect();

        let mut local_ops: Vec<LocalOp> = Vec::with_capacity(delete_ops.len() + insert_ops.len());
        for op in delete_ops { local_ops.push(LocalOp::DeleteItem(op)); }
        for op in insert_ops { local_ops.push(LocalOp::InsertItem(op)); }
        local_ops
    }

    pub fn reverse_execute_remote(&mut self, op: &UpdateArray) -> Vec<LocalOp> {
        let elements = mem::replace(&mut self.0, Vec::new());
        let mut inserted_uids = op.inserts.iter().map(|e| &e.uid).peekable();
        let mut deleted_elements = op.deleted_elements.iter().peekable();
        let mut local_ops: Vec<LocalOp> = Vec::new();

        let max_elt = Element::end_marker();
        let max_uid = UID::max();

        for (index, elt) in elements.into_iter().enumerate() {
            let should_delete_elt = {
                let uid = *inserted_uids.peek().unwrap_or(&&max_uid);
                uid < &max_uid && uid == &elt.uid
            };
            // if the current element was inserted by the UpdatedArray op,
            // remove it
            if should_delete_elt {
                inserted_uids.next();
                let op = DeleteItem::new(index - 1);
                local_ops.push(LocalOp::DeleteItem(op));
            // otherwise, insert all deleted element that precede the current
            // element and then re-insert the current element
            } else {
                while *deleted_elements.peek().unwrap_or(&&max_elt) < &elt {
                    let deleted_elt = deleted_elements.next().unwrap().clone();
                    let op = InsertItem::new(index - 1, deleted_elt.value.clone());
                    local_ops.push(LocalOp::InsertItem(op));
                    self.0.push(deleted_elt);
                }
                self.0.push(elt);
            }
        }
        local_ops
    }

    fn insert_remote(&mut self, element: Element) -> Option<InsertItem> {
        let uid = element.uid.clone();
        let ref mut elements = self.0;
        match elements.iter().position(|e| uid < e.uid) {
            Some(index) => {
                elements.insert(index, element.clone());
                Some(InsertItem::new(index-1, element.value.clone()))},
            None =>
                None,
        }
    }

    fn delete_remote(&mut self, uid: &UID) -> Option<DeleteItem> {
        let ref mut elements = self.0;
        match elements.iter().position(|e| *uid == e.uid) {
            Some(index) => {
                elements.remove(index);
                Some(DeleteItem::new(index-1))},
            None =>
                None,
        }
    }

    pub fn elements(&self) -> &[Element] {
        let lower = 1;
        let upper = self.len() + 1;
        &self.0[lower..upper]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::element::Element;
    use Error;
    use Replica;
    use Value;

    const REPLICA: Replica = Replica{site: 1, counter: 1};

    #[test]
    fn test_new() {
        let array = Array::new();
        assert!(array.len() == 0);
        assert!(array.0[0] == Element::start_marker());
        assert!(array.0[1] == Element::end_marker());
    }

    #[test]
    fn test_insert() {
        let mut array = Array::new();
        let _  = array.insert(0, Value::Str("a".to_string()), &REPLICA);
        let _  = array.insert(1, Value::Str("b".to_string()), &REPLICA);
        let op = array.insert(1, Value::Str("c".to_string()), &REPLICA).unwrap();

        assert!(array.len() == 3);
        assert!(array.0[1].value == Value::Str("a".to_string()));
        assert!(array.0[2].value == Value::Str("c".to_string()));
        assert!(array.0[3].value == Value::Str("b".to_string()));

        assert!(op.inserts.len() == 1);
        assert!(op.inserts[0].value == Value::Str("c".to_string()));
        assert!(op.deletes.len() == 0);
    }

    #[test]
    fn test_insert_invalid_index() {
        let mut array = Array::new();
        assert!(array.insert(1, Value::Bool(true), &REPLICA) == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_delete() {
        let mut array = Array::new();
        let op1 = array.insert(0, Value::Num(1.0), &REPLICA).unwrap();
        let _   = array.insert(1, Value::Num(2.0), &REPLICA);
        let op2 = array.delete(0).unwrap();

        assert!(array.0[1].value == Value::Num(2.0));
        assert!(op2.deletes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_delete_invalid_index() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::Num(1.0), &REPLICA);
        assert!(array.delete(1) == Err(Error::OutOfBounds));
    }

    #[test]
    fn test_execute_remote() {
        let mut array1 = Array::new();
        let mut array2 = Array::new();

        let op1 = array1.insert(0, Value::Num(1.0), &REPLICA).unwrap();
        let op2 = array1.insert(1, Value::Num(2.0), &REPLICA).unwrap();
        let op3 = array1.delete(0).unwrap();

        let lops1 = array2.execute_remote(&op1);
        let lops2 = array2.execute_remote(&op2);
        let lops3 = array2.execute_remote(&op3);

        assert!(array1 == array2);
        assert!(lops1.len() == 1);
        assert!(lops2.len() == 1);
        assert!(lops3.len() == 1);

        let lop1 = lops1[0].insert_item().unwrap();
        assert!(lop1.index == 0);
        assert!(lop1.value == Value::Num(1.0));

        let lop2 = lops2[0].insert_item().unwrap();
        assert!(lop2.index == 1);
        assert!(lop2.value == Value::Num(2.0));

        let lop3 = lops3[0].delete_item().unwrap();
        assert!(lop3.index == 0);
    }

    #[test]
    fn text_reverse_execute_remote_insert() {
        let mut array = Array::new();
        let remote_op = array.insert(0, Value::Num(1.0), &Replica::new(4,23)).expect("!");
        let local_ops = array.reverse_execute_remote(&remote_op);
        assert!(array.len() == 0);
        assert!(local_ops.len() == 1);
        assert!(local_ops[0].delete_item().expect("!").index == 0);
    }

    #[test]
    fn text_reverse_execute_remote_delete() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::Num(1.0), &Replica::new(4,23)).expect("!");
        let remote_op = array.delete(0).expect("!");
        let local_ops = array.reverse_execute_remote(&remote_op);
        assert!(array.len() == 1);
        assert!(local_ops.len() == 1);
        assert!(local_ops[0].insert_item().expect("!").index == 0);
        assert!(local_ops[0].insert_item().expect("!").value == Value::Num(1.0));
    }
}
