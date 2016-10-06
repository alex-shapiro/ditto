pub mod element;

use Value;
use Index;
use Site;
use Counter;
use sequence::path;
use sequence::uid::UID;
use self::element::Element;
use op::LocalOp;
use op::remote::UpdateArray;
use op::local::InsertItem;
use op::local::DeleteItem;

#[derive(Clone,PartialEq)]
pub struct Array(Vec<Element>);

impl Array {
    pub fn new() -> Array {
        Array(vec![Element::start_marker(), Element::end_marker()])
    }

    pub fn len(&self) -> usize {
        self.0.len() - 2
    }

    pub fn insert(&mut self, index: Index, value: Value, site: Site, counter: Counter) -> Option<UpdateArray> {
        if index <= self.len() {
            let ref mut elements = self.0;
            let path = {
                let ref path1 = elements[index].uid.path;
                let ref path2 = elements[index+1].uid.path;
                path::between(path1, path2, site)
            };
            let element = Element::new(value, path, counter);
            elements.insert(index+1, element.clone());
            Some(UpdateArray::insert(element))
        } else {
            None
        }
    }

    pub fn delete(&mut self, index: Index) -> Option<UpdateArray> {
        if index < self.len() {
            let element = self.0.remove(index+1);
            Some(UpdateArray::delete(element.uid))
        } else {
            None
        }
    }

    pub fn execute_remote(&mut self, op: UpdateArray) -> Vec<Box<LocalOp>> {
        let delete_ops: Vec<DeleteItem> =
            op.deletes.into_iter()
            .map(|uid| self.delete_remote(uid))
            .filter(|op| op.is_some())
            .map(|op| op.unwrap())
            .collect();

        let insert_ops: Vec<InsertItem> =
            op.inserts.into_iter()
            .map(|elt| self.insert_remote(elt))
            .filter(|op| op.is_some())
            .map(|op| op.unwrap())
            .collect();

        let mut local_ops: Vec<Box<LocalOp>> = vec![];
        for op in delete_ops { local_ops.push(Box::new(op)); }
        for op in insert_ops { local_ops.push(Box::new(op)); }
        local_ops
    }

    fn insert_remote(&mut self, element: Element) -> Option<InsertItem> {
        let path = element.uid.path.clone();
        let ref mut elements = self.0;
        match elements.iter().position(|e| path < e.uid.path) {
            Some(index) => {
                elements.insert(index, element.clone());
                Some(InsertItem::new(index-1, element.value.clone()))},
            None =>
                None,
        }
    }

    fn delete_remote(&mut self, uid: UID) -> Option<DeleteItem> {
        let ref mut elements = self.0;
        match elements.iter().position(|e| uid == e.uid) {
            Some(index) => {
                elements.remove(index);
                Some(DeleteItem::new(index-1))},
            None =>
                None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::element::Element;
    use Value;
    use op::LocalOp;
    use op::local::InsertItem;
    use op::local::DeleteItem;
    use std::any::Any;

    #[test]
    fn test_new() {
        let array = Array::new();
        assert!(array.len() == 0);
        assert!(array.0[0] == Element::start_marker());
        assert!(array.0[1] == Element::end_marker());
        assert!(array.len() == 0);
    }

    #[test]
    fn test_insert() {
        let mut array = Array::new();
        let _  = array.insert(0, Value::Str("a".to_string()), 1, 1);
        let _  = array.insert(1, Value::Str("b".to_string()), 1, 2);
        let op = array.insert(1, Value::Str("c".to_string()), 1, 3).unwrap();

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
        assert!(array.insert(1, Value::Bool(true), 1, 1).is_none());
    }

    #[test]
    fn test_delete() {
        let mut array = Array::new();
        let op1 = array.insert(0, Value::Num(1.0), 1, 1).unwrap();
        let _   = array.insert(1, Value::Num(2.0), 1, 1);
        let op2 = array.delete(0).unwrap();

        assert!(array.0[1].value == Value::Num(2.0));
        assert!(op2.deletes[0] == op1.inserts[0].uid);
    }

    #[test]
    fn test_delete_invalid_index() {
        let mut array = Array::new();
        array.insert(0, Value::Num(1.0), 1, 1);
        assert!(array.delete(1).is_none());
    }

    #[test]
    fn test_execute_remote() {
        let mut array1 = Array::new();
        let mut array2 = Array::new();

        let op1 = array1.insert(0, Value::Num(1.0), 1, 1).unwrap();
        let op2 = array1.insert(1, Value::Num(2.0), 1, 2).unwrap();
        let op3 = array1.delete(0).unwrap();
        let lops1 = array2.execute_remote(op1);
        let lops2 = array2.execute_remote(op2);
        let lops3 = array2.execute_remote(op3);

        assert!(array1 == array2);
        assert!(lops1.len() == 1);
        assert!(lops2.len() == 1);
        assert!(lops3.len() == 1);

        let lop1 = to::<InsertItem>(&lops1);
        assert!(lop1.index == 0);
        assert!(lop1.value == Value::Num(1.0));

        let lop2 = to::<InsertItem>(&lops2);
        assert!(lop2.index == 1);
        assert!(lop2.value == Value::Num(2.0));

        let lop3 = to::<DeleteItem>(&lops3);
        assert!(lop3.index == 0);
    }

    fn to<'a, T: Any>(ops: &'a [Box<LocalOp>]) -> &'a T {
        let op = &ops[0];
        op.as_any().downcast_ref::<T>().unwrap()
    }
}
