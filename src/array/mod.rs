pub mod element;

use Value;
use Index;
use Site;
use Counter;
use sequence::path;
use self::element::Element;
use op::remote::UpdateArray;

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
}
