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
