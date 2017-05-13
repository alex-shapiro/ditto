//! A `List` stores an ordered sequence of elements.
//! Elements in the list are immutable.

use Error;
use Replica;
use sequence::uid::{self, UID};
use traits::*;
use std::slice;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List<T> {
    value: ListValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListValue<T>(pub Vec<Element<T>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteOp<T> {
    Insert(Element<T>),
    Remove(UID),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<T> {
    Insert { index: usize, value: T },
    Remove { index: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element<T>(pub UID, pub T);

impl<T: Clone> List<T> {

    /// Constructs and returns a new list.
    /// Th list has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1,0);
        let value = ListValue::new();
        List{replica, value, awaiting_site: vec![]}
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.value.0.len()
    }

    /// Returns a reference to the element at position `index`.
    /// Returns None if the index is out-of-bounds.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.value.0.get(index).and_then(|element| Some(&element.1))
    }

    /// Inserts an element at position `index` within the list,
    /// shifting all elements after it to the right. Returns an
    /// error if the index is out-of-bounds. If the list does not
    /// have a site allocated, it caches the op and returns an
    /// `AwaitingSite` error.
    pub fn insert(&mut self, index: usize, value: T) -> Result<RemoteOp<T>, Error> {
        let remote_op = self.value.insert(index, value, &self.replica)?;
        self.after_op(remote_op)
    }

    /// Removes the element at position `index` from the list,
    /// shifting all elements after it to the left. Returns an
    /// error if the index is out-of-bounds. If the list does not
    /// have a site allocated, it caches the op and returns an
    /// `AwaitingSite` error.
    pub fn remove(&mut self, index: usize) -> Result<RemoteOp<T>, Error> {
        let remote_op = self.value.remove(index)?;
        self.after_op(remote_op)
    }
}

impl<T: Clone> Crdt for List<T> {
    crdt_impl!(List, ListValue<T>);
}

impl<T: Clone> ListValue<T> {
    pub fn new() -> Self {
        ListValue(vec![])
    }

    pub fn iter(&self) -> slice::Iter<Element<T>> {
        self.0.iter()
    }

    pub fn find_index(&self, uid: &UID) -> Result<usize, usize> {
        self.0.binary_search_by(|element| element.0.cmp(uid))
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Element<T>> {
        self.0.get_mut(index)
    }

    pub fn insert(&mut self, index: usize, value: T, replica: &Replica) -> Result<RemoteOp<T>, Error> {
        let max_index = self.0.len();
        if index > max_index { return Err(Error::OutOfBounds) }

        let uid = {
            let uid1 = if index == 0 { &*uid::MIN } else { &self.0[index-1].0 };
            let uid2 = if index == max_index { &*uid::MAX } else { &self.0[index].0 };
            UID::between(uid1, uid2, replica)
        };

        let element = Element(uid, value);
        self.0.insert(index, element.clone());
        Ok(RemoteOp::Insert(element))
    }

    pub fn remove(&mut self, index: usize) -> Result<RemoteOp<T>, Error> {
        if index >= self.0.len() { return Err(Error::OutOfBounds) }
        let element = self.0.remove(index);
        Ok(RemoteOp::Remove(element.0))
    }

    /// Updates the list and returns the equivalent local op.
    /// If the remote op is a duplicate, returns `None`.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        match *op {
            RemoteOp::Insert(ref element) => {
                let index = try_opt!(self.find_index(&element.0).err());
                self.0.insert(index, element.clone());
                let value = element.1.clone();
                Some(LocalOp::Insert{index: index, value: value})
            }
            RemoteOp::Remove(ref uid) => {
                let index = try_opt!(self.find_index(&uid).ok());
                self.0.remove(index);
                Some(LocalOp::Remove{index: index})
            }
        }
    }
}

impl<T: Clone> CrdtValue for ListValue<T> {
    type LocalValue = Vec<T>;
    type RemoteOp = RemoteOp<T>;
    type LocalOp = LocalOp<T>;

    fn local_value(&self) -> Vec<T> {
        let mut vec = vec![];
        for element in self.0.iter() {
            vec.push(element.1.clone())
        }
        vec
    }

    fn add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        if let RemoteOp::Insert(Element(ref uid, _)) = *op {
            let index = some!(self.find_index(uid).ok());
            self.0[index].0.site = site;
        }
    }
}

impl<T: Clone + AddSiteToAll> AddSiteToAll for ListValue<T> {
    fn add_site_to_all(&mut self, site: u32) {
        for element in self.0.iter_mut() {
            element.0.site = site;
            element.1.add_site_to_all(site);
        }
    }
}

impl<T> CrdtRemoteOp for RemoteOp<T> {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert(Element(ref mut uid, _)) => uid.site = site,
            RemoteOp::Remove(ref mut uid) => {
                if uid.site == 0 { uid.site = site };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use rmp_serde;

    #[test]
    fn test_new() {
        let list: List<i64> = List::new();
        assert!(list.len() == 0);
    }

    #[test]
    fn test_get() {
        let mut list: List<i64> = List::new();
        let _ = list.insert(0, 123).unwrap();
        assert!(list.get(0).unwrap() == &123);
    }

    #[test]
    fn test_find_index() {
        let mut list: List<i64> = List::new();
        let _  = list.insert(0, 123).unwrap();
        let op = list.insert(1, 456).unwrap();
        let _  = list.insert(2, 789).unwrap();

        let Element(uid, _) = get_elt(op);
        assert!(list.value.find_index(&uid) == Ok(1));
        assert!(list.value.find_index(&*uid::MIN) == Err(0));
        assert!(list.value.find_index(&*uid::MAX) == Err(3));
    }

    #[test]
    fn test_insert_prepend() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123).unwrap();
        let op2 = list.insert(0, 456).unwrap();
        let op3 = list.insert(0, 789).unwrap();

        assert!(list.len() == 3);
        assert!(list.value.0[0].1 == 789);
        assert!(list.value.0[1].1 == 456);
        assert!(list.value.0[2].1 == 123);

        let element1 = get_elt(op1);
        let element2 = get_elt(op2);
        let element3 = get_elt(op3);

        assert!(element1.1 == 123 && element2.1 == 456 && element3.1 == 789);
        assert!(*uid::MAX > element1.0);
        assert!(element1.0 > element2.0);
        assert!(element2.0 > element3.0);
        assert!(element3.0 > *uid::MIN);
    }

    #[test]
    fn test_insert_append() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123).unwrap();
        let op2 = list.insert(1, 456).unwrap();
        let op3 = list.insert(2, 789).unwrap();

        assert!(list.value.0[0].1 == 123);
        assert!(list.value.0[1].1 == 456);
        assert!(list.value.0[2].1 == 789);

        let element1 = get_elt(op1);
        let element2 = get_elt(op2);
        let element3 = get_elt(op3);

        assert!(*uid::MIN < element1.0);
        assert!(element1.0 < element2.0);
        assert!(element2.0 < element3.0);
        assert!(element3.0 < *uid::MAX);
    }

    #[test]
    fn test_insert_middle() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123).unwrap();
        let op2 = list.insert(1, 456).unwrap();
        let op3 = list.insert(1, 789).unwrap();

        assert!(list.value.0[0].1 == 123);
        assert!(list.value.0[1].1 == 789);
        assert!(list.value.0[2].1 == 456);

        let element1 = get_elt(op1);
        let element2 = get_elt(op2);
        let element3 = get_elt(op3);

        assert!(*uid::MIN < element1.0);
        assert!(element1.0 < element3.0);
        assert!(element3.0 < element2.0);
        assert!(element2.0 < *uid::MAX);
    }

    #[test]
    fn test_insert_out_of_bounds() {
        let mut list: List<i64> = List::new();
        let result = list.insert(1, 123);
        assert!(result.unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_remove() {
        let mut list: List<i64> = List::new();
        let _   = list.insert(0, 123).unwrap();
        let op1 = list.insert(1, 456).unwrap();
        let _   = list.insert(2, 789).unwrap();
        let op2 = list.remove(1).unwrap();

        let element1 = get_elt(op1);
        let element2 = get_elt(op2);

        assert!(list.len() == 2);
        assert!(list.value.0[0].1 == 123);
        assert!(list.value.0[1].1 == 789);
        assert!(element1 == element2);
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut list: List<i64> = List::new();
        let result = list.remove(0);
        assert!(result.unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_insert_remove_awaiting_site() {
        let mut list: List<i64> = List::from_value(ListValue::new(), 0);
        assert!(list.insert(0, 123).unwrap_err() == Error::AwaitingSite);
        assert!(list.len() == 1);
        assert!(list.awaiting_site.len() == 1);
        assert!(list.remove(0).unwrap_err() == Error::AwaitingSite);
        assert!(list.len() == 0);
        assert!(list.awaiting_site.len() == 2);
    }

    #[test]
    fn test_execute_remote_insert() {
        let mut list1: List<&'static str> = List::new();
        let mut list2: List<&'static str> = List::new();
        let remote_op = list1.insert(0, "a").unwrap();
        let local_op = list2.execute_remote(&remote_op).unwrap();

        assert!(list2.len() == 1);
        assert!(list2.value.0[0].1 == "a");
        assert_matches!(local_op, LocalOp::Insert{index: 0, value: "a"});
    }

    #[test]
    fn test_execute_remote_insert_dupe() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op = list1.insert(0, "a".to_owned()).unwrap();

        let _ = list2.execute_remote(&remote_op).unwrap();
        assert!(list2.execute_remote(&remote_op).is_none());
        assert!(list2.len() == 1);
    }

    #[test]
    fn test_execute_remote_remove() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op1 = list1.insert(0, "a".to_owned()).unwrap();
        let remote_op2 = list1.remove(0).unwrap();
        let _ = list2.execute_remote(&remote_op1).unwrap();
        let local_op = list2.execute_remote(&remote_op2).unwrap();
        assert!(list2.len() == 0);
        assert_matches!(local_op, LocalOp::Remove{index: 0});
    }

    #[test]
    fn test_execute_remote_remove_dupe() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op1 = list1.insert(0, "a".to_owned()).unwrap();
        let remote_op2 = list1.remove(0).unwrap();

        let _ = list2.execute_remote(&remote_op1).unwrap();
        let _ = list2.execute_remote(&remote_op2).unwrap();
        assert!(list2.execute_remote(&remote_op2).is_none());
        assert!(list2.len() == 0);
    }

    #[test]
    fn test_add_site() {
        let mut list: List<u32> = List::from_value(ListValue::new(), 0);
        let _ = list.insert(0, 51);
        let _ = list.insert(1, 52);
        let _ = list.remove(0);
        let mut remote_ops = list.add_site(12).unwrap().into_iter();

        let element1 = get_elt(remote_ops.next().unwrap());
        let element2 = get_elt(remote_ops.next().unwrap());
        let element3 = get_elt(remote_ops.next().unwrap());

        assert!(list.value.0[0].0.site == 12);
        assert!(element1.0.site == 12);
        assert!(element2.0.site == 12);
        assert!(element3.0.site == 12);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut list: List<u32> = List::from_value(ListValue::new(), 12);
        let _ = list.insert(0, 51);
        let _ = list.insert(1, 52);
        let _ = list.remove(0);
        assert!(list.add_site(13).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let mut list1: List<u32> = List::new();
        let _ = list1.insert(0, 502);
        let _ = list1.insert(1, 48);

        let s_json = serde_json::to_string(&list1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&list1).unwrap();
        let list2: List<u32> = serde_json::from_str(&s_json).unwrap();
        let list3: List<u32> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(list1 == list2);
        assert!(list1 == list3);
    }

    #[test]
    fn test_serialize_value() {
        let mut list1: List<String> = List::new();
        let _ = list1.insert(0, "Bob".to_owned());
        let _ = list1.insert(1, "Sue".to_owned());

        let s_json = serde_json::to_string(list1.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(list1.value()).unwrap();
        let value2: ListValue<String> = serde_json::from_str(&s_json).unwrap();
        let value3: ListValue<String> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*list1.value() == value2);
        assert!(*list1.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut list: List<i8> = List::new();
        let remote_op1 = list.insert(0, 24).unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp<i8> = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp<i8> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let local_op1: LocalOp<String> = LocalOp::Insert{index: 0, value: "Bob".to_string()};

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp<String> = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp<String> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }

    fn get_elt<T>(remote_op: RemoteOp<T>) -> Element<T> {
        match remote_op {
            RemoteOp::Insert(element) => element,
            RemoteOp::Remove(element) => element,
        }
    }
}
