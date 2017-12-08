//! A CRDT that stores an ordered sequence of elements

use {Error, Replica, Tombstones};
use order_statistic_tree::{self, Tree};
use sequence::uid::{self, UID};
use traits::*;
use std::borrow::Cow;

/// A List is a `Vec`-like ordered sequence of elements.
/// To allow for CRDT replication, List elements must implement
/// the `Clone`, `Serialize`, and `Deserialize` traits.
///
/// An *N*-element List's performance characteristics are:
///
///   * [`insert`](#method.insert) is approximately *O(log N)*
///   * [`remove`](#method.remove) is approximately *O(log N)*
///   * [`execute_remote`](#method.remove) is approximately *O(log N)*
///   * [`get`](#method.get) is approximately *O(1)*
///   * [`len`](#method.insert) is *O(1)*
///
/// Internally, List is based on LSEQ. It can be used as a CmRDT or a CvRDT,
/// providing eventual consistency via both op execution and state merges.
/// This flexibility comes with tradeoffs:
///
///   * Unlike a pure CmRDT, it requires tombstones, which increase size.
///   * Unlike a pure CvRDT, it requires each site to replicate its ops
///     in their order of generation.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List<T: 'static> {
    value: ListValue<T>,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListState<'a, T: Clone + 'a + 'static> {
    value: Cow<'a, ListValue<T>>,
    tombstones: Cow<'a, Tombstones>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListValue<T: 'static>(pub Tree<Element<T>>);

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

    /// Constructs and returns a new list CRDT with site 1.
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
        let element = try_opt!(self.value.0.get_elt(index).ok()).0;
        Some(&element.1)
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

    crdt_impl!(List, ListState, ListState<T>, ListState<'static, T>, ListValue<T>);
}

impl<T: Clone> From<Vec<T>> for List<T> {
    fn from(local_value: Vec<T>) -> Self {
        let replica = Replica::new(1,0);
        let mut value = ListValue::new();

        for element in local_value {
            let _ = value.push(element, &replica);
        }

        let tombstones = Tombstones::new();
        List{replica, value, tombstones, awaiting_site: vec![]}
    }
}

impl<T: Clone> ListValue<T> {
    pub fn new() -> Self {
        ListValue(Tree::new())
    }

    pub fn iter(&self) -> order_statistic_tree::Iter<Element<T>> {
        self.0.iter()
    }

    pub fn push(&mut self, value: T, replica: &Replica) -> Result<RemoteOp<T>, Error> {
        let len = self.0.len();
        self.insert(len, value, replica)
    }

    pub fn insert(&mut self, index: usize, value: T, replica: &Replica) -> Result<RemoteOp<T>, Error> {
        let max_index = self.0.len();
        if index > max_index { return Err(Error::OutOfBounds) }

        let uid = {
            let uid1 = if index == 0 { &*uid::MIN } else { &(self.0.get_elt(index-1)?.0).0 };
            let uid2 = if index == max_index { &*uid::MAX } else { &(self.0.get_elt(index)?.0).0 };
            UID::between(uid1, uid2, replica)
        };

        let element = Element(uid, value);
        self.0.insert(element.clone())?;
        Ok(RemoteOp::Insert(element))
    }

    pub fn remove(&mut self, index: usize) -> Result<RemoteOp<T>, Error> {
        let uid = (self.0.get_elt(index)?.0).0.clone();
        let element = self.0.remove(&uid).unwrap();
        Ok(RemoteOp::Remove(element.0))
    }

    /// Updates the list and returns the equivalent local op.
    /// If the remote op is a duplicate, returns `None`.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        match *op {
            RemoteOp::Insert(ref element) => {
                try_opt!(self.0.insert(element.clone()).ok());
                let index = self.0.get_idx(&element.0).unwrap();
                let value = element.1.clone();
                Some(LocalOp::Insert{index, value})
            }
            RemoteOp::Remove(ref uid) => {
                let index = try_opt!(self.0.get_idx(uid));
                let _ = self.0.remove(&uid).unwrap();
                Some(LocalOp::Remove{index})
            }
        }
    }
}

impl<T: Clone> CrdtValue for ListValue<T> {
    type LocalValue = Vec<T>;
    type RemoteOp = RemoteOp<T>;
    type LocalOp = LocalOp<T>;

    fn local_value(&self) -> Vec<T> {
        self.0.iter().map(|element| element.1.clone()).collect()
    }

    fn add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        if let RemoteOp::Insert(Element(ref uid, _)) = *op {
            let mut element = some!(self.0.remove(uid));
            element.0.site = site;
            let _ = self.0.insert(element);
        }
    }

    fn add_site_to_all(&mut self, site: u32) {
        let old_tree = ::std::mem::replace(&mut self.0, Tree::new());
        for mut element in old_tree {
            element.0.site = site;
            let _ = self.0.insert(element);
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        for element in &self.0 {
            try_assert!(element.0.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }

    fn merge(&mut self, other: ListValue<T>, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        let removed_uids: Vec<UID> = self.0.iter()
            .filter(|e| other.0.get_idx(&e.0).is_none() && other_tombstones.contains_pair(e.0.site, e.0.counter))
            .map(|e| e.0.clone())
            .collect();

        let new_elements: Vec<Element<T>> = other.0.iter()
            .filter(|e| self.0.get_idx(&e.0).is_none() && !self_tombstones.contains_pair(e.0.site, e.0.counter))
            .map(|e| e.clone())
            .collect();

        for uid in removed_uids {
            let _ = self.0.remove(&uid);
        }

        for element in new_elements {
            let _ = self.0.insert(element);
        }
    }
}

impl<T: Clone + NestedCrdtValue> NestedCrdtValue for ListValue<T> {
    fn nested_add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        if let RemoteOp::Insert(Element(ref uid, _)) = *op {
            let mut element = some!(self.0.remove(uid));
            element.0.site = site;
            element.1.add_site_to_all(site);
            self.0.insert(element).unwrap();
        }
    }

    fn nested_add_site_to_all(&mut self, site: u32) {
        let old_tree = ::std::mem::replace(&mut self.0, Tree::new());
        for mut element in old_tree {
            element.0.site = site;
            element.1.add_site_to_all(site);
            let _ = self.0.insert(element);
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        for element in &self.0 {
            try_assert!(element.0.site == site, Error::InvalidRemoteOp);
            try!(element.1.nested_validate_site(site));
        }
        Ok(())
    }

    fn nested_merge(&mut self, other: ListValue<T>, self_tombstones: &Tombstones, other_tombstones: &Tombstones) -> Result<(), Error> {
        {
            let removed_uids: Vec<UID> = self.0.iter()
                .filter(|e| other.0.get_idx(&e.0).is_none() && other_tombstones.contains_pair(e.0.site, e.0.counter))
                .map(|e| e.0.clone())
                .collect();

            for uid in removed_uids {
                let _ = self.0.remove(&uid);
            }
        }

        for element in other.0.into_iter() {
            if self.0.lookup(&element.0).is_some() {
                let self_element = self.0.lookup_mut(&element.0).unwrap();
                self_element.1.nested_merge(element.1, self_tombstones, other_tombstones)?;
            } else if !self_tombstones.contains_pair(element.0.site, element.0.counter) {
                let _ = self.0.insert(element);
            }
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

impl<T: NestedCrdtValue> NestedCrdtRemoteOp for RemoteOp<T> {
    fn nested_add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert(ref mut element) => {
                element.0.site = site;
                element.1.add_site_to_all(site);
            }
            RemoteOp::Remove(ref mut uid) => {
                if uid.site == 0 { uid.site = site };
            }
        }
    }

    fn nested_validate_site(&self, site: u32) -> Result<(), Error> {
        match *self {
            RemoteOp::Remove(_) => Ok(()),
            RemoteOp::Insert(ref element) => {
                try_assert!(element.0.site == site, Error::InvalidRemoteOp);
                element.1.nested_validate_site(site)
            }
        }
    }
}

impl<T> order_statistic_tree::Element for Element<T> {
    type Id = UID;

    fn id(&self) -> &UID {
        &self.0
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
        assert!(list.value.0.get_idx(&uid) == Some(1));
        assert!(list.value.0.get_idx(&*uid::MIN) == None);
        assert!(list.value.0.get_idx(&*uid::MAX) == None);
    }

    #[test]
    fn test_insert_prepend() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123).unwrap();
        let op2 = list.insert(0, 456).unwrap();
        let op3 = list.insert(0, 789).unwrap();

        assert!(list.len() == 3);
        assert!(element_at(&list, 0).1 == 789);
        assert!(element_at(&list, 1).1 == 456);
        assert!(element_at(&list, 2).1 == 123);

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

        assert!(element_at(&list, 0).1 == 123);
        assert!(element_at(&list, 1).1 == 456);
        assert!(element_at(&list, 2).1 == 789);

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

        assert!(element_at(&list, 0).1 == 123);
        assert!(element_at(&list, 1).1 == 789);
        assert!(element_at(&list, 2).1 == 456);

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
        assert!(element_at(&list, 0).1 == 123);
        assert!(element_at(&list, 1).1 == 789);
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
        let mut list: List<i64> = List::from_state(List::new().clone_state(), None).unwrap();
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
        assert!(element_at(&list2, 0).1 == "a");
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

        let mut list2 = List::from_state(list1.clone_state(), Some(2)).unwrap();
        let _ = list2.remove(0);
        let _ = list2.insert(1, 12);
        let _ = list2.insert(2, 15);
        let _ = list1.remove(1);
        let _ = list1.insert(1, 12);

        let list1_state = list1.clone_state();
        list1.merge(list2.clone_state());
        list2.merge(list1_state);

        assert_eq!(list1.value, list2.value);
        assert_eq!(list1.tombstones, list2.tombstones);
        assert_eq!(list1.local_value(), [12, 12, 15]);
        assert_eq!(element_at(&list1, 2).0.site,    2);
        assert_eq!(element_at(&list1, 2).0.counter, 2);
        assert!(list1.tombstones.contains_pair(1,2));
    }

    #[test]
    fn test_add_site() {
        let mut list: List<u32> = List::from_state(List::new().clone_state(), None).unwrap();
        let _ = list.insert(0, 51);
        let _ = list.insert(1, 52);
        let _ = list.remove(0);
        let mut remote_ops = list.add_site(12).unwrap().into_iter();

        let element1 = get_insert_elt(remote_ops.next().unwrap());
        let element2 = get_insert_elt(remote_ops.next().unwrap());
        let uid3     = get_remove_uid(remote_ops.next().unwrap());

        assert!(element_at(&list, 0).0.site == 12);
        assert!(element1.0.site == 12);
        assert!(element2.0.site == 12);
        assert!(uid3.site == 12);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut list: List<u32> = List::from_state(List::new().clone_state(), Some(12)).unwrap();
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

    fn element_at<T>(list: &List<T>, idx: usize) -> &Element<T> {
        list.value.0.get_elt(idx).unwrap().0
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
