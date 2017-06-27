//! A `List` stores an ordered sequence of elements.
//! Elements in the list are immutable.

use {Error, Replica, Tombstones};
use sequence::uid::{self, UID};
use traits::*;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::mem;
use std::slice;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List<T> {
    value: ListValue<T>,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListState<'a, T: Clone + 'a> {
    value: Cow<'a, ListValue<T>>,
    tombstones: Cow<'a, Tombstones>,
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

    crdt_impl!(List, ListState, ListState<T>, ListState<'static, T>, ListValue<T>);

    /// Constructs and returns a new list.
    /// Th list has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1,0);
        let value = ListValue::new();
        let tombstones = Tombstones::new();
        List{replica, value, tombstones, awaiting_site: vec![]}
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
        let op = self.value.insert(index, value, &self.replica)?;
        self.after_op(op)
    }

    /// Removes the element at position `index` from the list,
    /// shifting all elements after it to the left. Returns an
    /// error if the index is out-of-bounds. If the list does not
    /// have a site allocated, it caches the op and returns an
    /// `AwaitingSite` error.
    pub fn remove(&mut self, index: usize) -> Result<RemoteOp<T>, Error> {
        let op = self.value.remove(index)?;
        self.after_op(op)
    }
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

    pub fn merge(&mut self, other: ListValue<T>, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        let mut self_iter = mem::replace(&mut self.0, vec![]).into_iter();
        let mut other_iter = other.0.into_iter();
        let mut s_element = self_iter.next();
        let mut o_element = other_iter.next();

        while s_element.is_some() || o_element.is_some() {
            match compare(s_element.as_ref(), o_element.as_ref()) {
                Ordering::Equal => {
                    self.0.push(mem::replace(&mut s_element, self_iter.next()).unwrap());
                    o_element = other_iter.next();
                }
                Ordering::Less => {
                    let element = mem::replace(&mut s_element, self_iter.next()).unwrap();
                    if !other_tombstones.contains_pair(element.0.site, element.0.counter) {
                        self.0.push(element);
                    }
                }
                Ordering::Greater => {
                    let element = mem::replace(&mut o_element, other_iter.next()).unwrap();
                    if !self_tombstones.contains_pair(element.0.site, element.0.counter) {
                        self.0.push(element);
                    }
                }
            }
        }
    }
}

impl<T: Clone + NestedValue> NestedValue for ListValue<T> {
    fn nested_merge(&mut self, other: ListValue<T>, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        let mut self_iter = mem::replace(&mut self.0, vec![]).into_iter();
        let mut other_iter = other.0.into_iter();
        let mut s_element = self_iter.next();
        let mut o_element = other_iter.next();

        while s_element.is_some() || o_element.is_some() {
            match compare(s_element.as_ref(), o_element.as_ref()) {
                Ordering::Equal => {
                    let mut elt1 = mem::replace(&mut s_element, self_iter.next()).unwrap();
                    let elt2 = mem::replace(&mut o_element, other_iter.next()).unwrap();
                    elt1.1.nested_merge(elt2.1, self_tombstones, other_tombstones);
                    self.0.push(elt1);
                }
                Ordering::Less => {
                    let element = mem::replace(&mut s_element, self_iter.next()).unwrap();
                    if !other_tombstones.contains_pair(element.0.site, element.0.counter) {
                        self.0.push(element);
                    }
                }
                Ordering::Greater => {
                    let element = mem::replace(&mut o_element, other_iter.next()).unwrap();
                    if !self_tombstones.contains_pair(element.0.site, element.0.counter) {
                        self.0.push(element);
                    }
                }
            }
        }
    }
}

fn compare<T>(e1: Option<&Element<T>>, e2: Option<&Element<T>>) -> Ordering {
    let e1 = unwrap_or!(e1, Ordering::Greater);
    let e2 = unwrap_or!(e2, Ordering::Less);
    e1.0.cmp(&e2.0)
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

    fn validate_site_for_all(&self, site: u32) -> Result<(), Error> {
        for element in &self.0 {
            try_assert!(element.0.site == site, Error::InvalidRemoteOp);
            try!(element.1.validate_site_for_all(site));
        }
        Ok(())
    }
}

impl<T> CrdtRemoteOp for RemoteOp<T> {
    fn deleted_replicas(&self) -> Vec<Replica> {
        match *self {
            RemoteOp::Remove(ref uid) => vec![Replica{site: uid.site, counter: uid.counter}],
            _ => vec![],
        }
    }

    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert(Element(ref mut uid, _)) => uid.site = site,
            RemoteOp::Remove(ref mut uid) => {
                if uid.site == 0 { uid.site = site };
            }
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        match *self {
            RemoteOp::Remove(_) => Ok(()),
            RemoteOp::Insert(Element(ref uid, _)) => {
                try_assert!(uid.site == site, Error::InvalidRemoteOp);
                Ok(())
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

        let Element(uid, _) = get_insert_elt(op);
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

        let element1 = get_insert_elt(op1);
        let element2 = get_insert_elt(op2);
        let element3 = get_insert_elt(op3);

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

        let element1 = get_insert_elt(op1);
        let element2 = get_insert_elt(op2);
        let element3 = get_insert_elt(op3);

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

        let element1 = get_insert_elt(op1);
        let element2 = get_insert_elt(op2);
        let element3 = get_insert_elt(op3);

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

        let element = get_insert_elt(op1);
        let uid     = get_remove_uid(op2);

        assert!(list.len() == 2);
        assert!(list.value.0[0].1 == 123);
        assert!(list.value.0[1].1 == 789);
        assert!(element.0 == uid);
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut list: List<i64> = List::new();
        let result = list.remove(0);
        assert!(result.unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_insert_remove_awaiting_site() {
        let mut list: List<i64> = List::from_state(List::new().clone_state(), 0);
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
    fn test_merge() {
        let mut list1 = List::new();
        let _ = list1.insert(0, 3);
        let _ = list1.insert(1, 6);
        let _ = list1.insert(2, 9);
        let _ = list1.remove(1);

        let mut list2 = List::from_state(list1.clone_state(), 2);
        let _ = list2.remove(0);
        let _ = list2.insert(1, 12);
        let _ = list2.insert(2, 15);
        let _ = list1.remove(1);
        let _ = list1.insert(1, 12);

        let list1_state = list1.clone_state();
        list1.merge(list2.clone_state());
        list2.merge(list1_state);

        assert!(list1.value == list2.value);
        assert!(list1.tombstones == list2.tombstones);
        assert!(list1.local_value() == vec![12, 12, 15]);

        assert!(list1.value.0[0].0.site    == 1);
        assert!(list1.value.0[0].0.counter == 5);
        assert!(list1.value.0[1].0.site    == 2);
        assert!(list1.value.0[1].0.counter == 1);
        assert!(list1.value.0[2].0.site    == 2);
        assert!(list1.value.0[2].0.counter == 2);
        assert!(list1.tombstones.contains_pair(1,2));
    }

    #[test]
    fn test_add_site() {
        let mut list: List<u32> = List::from_state(List::new().clone_state(), 0);
        let _ = list.insert(0, 51);
        let _ = list.insert(1, 52);
        let _ = list.remove(0);
        let mut remote_ops = list.add_site(12).unwrap().into_iter();

        let element1 = get_insert_elt(remote_ops.next().unwrap());
        let element2 = get_insert_elt(remote_ops.next().unwrap());
        let uid3     = get_remove_uid(remote_ops.next().unwrap());

        assert!(list.value.0[0].0.site == 12);
        assert!(element1.0.site == 12);
        assert!(element2.0.site == 12);
        assert!(uid3.site == 12);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut list: List<u32> = List::from_state(List::new().clone_state(), 12);
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

    fn get_insert_elt<T>(remote_op: RemoteOp<T>) -> Element<T> {
        match remote_op {
            RemoteOp::Insert(element) => element,
            RemoteOp::Remove(_) => panic!(),
        }
    }

    fn get_remove_uid<T>(remote_op: RemoteOp<T>) -> UID {
        match remote_op {
            RemoteOp::Remove(uid) => uid,
            RemoteOp::Insert(_) => panic!(),
        }
    }
}
