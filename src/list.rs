//! A `List` stores an ordered sequence of elements.
//! Elements in the list are immutable.

use Error;
use Replica;
use sequence::uid::{self, UID};
use traits::*;

use std::mem;

#[derive(Debug, Clone, PartialEq)]
pub struct List<T> {
    value: ListValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListValue<T>(Vec<Element<T>>);

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteOp<T> {
    Insert(Element<T>),
    Remove(Element<T>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalOp<T> {
    Insert { index: usize, value: T },
    Remove { index: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element<T>(UID, T);

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

    /// Returns the index of the element with `uid`. If a matching
    /// element is found, it returns `Ok` containing the element index.
    /// If no matching element is found, it returns `Err` containing
    /// the index where an element with the UID may be inserted.
    pub fn find_index(&self, uid: &UID) -> Result<usize, usize> {
        self.value.find_index(uid)
    }

    /// Inserts an element at position `index` within the list,
    /// shifting all elements after it to the right. Returns an
    /// error if the index is out-of-bounds. If the list does not
    /// have a site allocated, it caches the op and returns an
    /// `AwaitingSite` error.
    pub fn insert(&mut self, index: usize, value: T) -> Result<RemoteOp<T>, Error> {
        let remote_op = self.value.insert(index, value, &self.replica)?;
        self.manage_op(remote_op)
    }

    /// Removes the element at position `index` from the list,
    /// shifting all elements after it to the left. Returns an
    /// error if the index is out-of-bounds. If the list does not
    /// have a site allocated, it caches the op and returns an
    /// `AwaitingSite` error.
    pub fn remove(&mut self, index: usize) -> Result<RemoteOp<T>, Error> {
        let remote_op = self.value.remove(index)?;
        self.manage_op(remote_op)
    }

    fn manage_op(&mut self, op: RemoteOp<T>) -> Result<RemoteOp<T>, Error> {
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl<T: Clone> Crdt for List<T> {
    type Value = ListValue<T>;

    fn site(&self) -> u32 {
        self.replica.site
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn clone_value(&self) -> Self::Value {
        self.value.clone()
    }

    fn from_value(value: Self::Value, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        List{value, replica, awaiting_site: vec![]}
    }

    fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        self.value.execute_remote(op)
    }

    fn add_site(&mut self, site: u32) -> Result<Vec<RemoteOp<T>>, Error> {
        if self.replica.site != 0 { return Err(Error::AlreadyHasSite) }
        let mut ops = mem::replace(&mut self.awaiting_site, vec![]);
        for op in &mut ops {
            self.value.add_site(op, site);
            op.add_site(site);
        }
        Ok(ops)
    }
}

impl<T: Clone> ListValue<T> {
    pub fn new() -> Self {
        ListValue(vec![])
    }

    pub fn find_index(&self, uid: &UID) -> Result<usize, usize> {
        self.0.binary_search_by(|element| element.0.cmp(uid))
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
        Ok(RemoteOp::Remove(element))
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
            RemoteOp::Remove(ref element) => {
                let index = try_opt!(self.find_index(&element.0).ok());
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
            if let Ok(index) = self.find_index(uid) {
                self.0[index].0.site = site;
            }
        }
    }
}

impl<T> CrdtRemoteOp for RemoteOp<T> {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert(Element(ref mut uid, _)) => uid.site = site,
            RemoteOp::Remove(Element(ref mut uid, _)) => uid.site = site,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let list: List<i64> = List::new();
        assert!(list.len() == 0);
    }

    #[test]
    fn test_get() {
        let mut list: List<i64> = List::new();
        let _ = list.insert(0, 123, &Replica::new(1, 0));
        assert!(list.get(0) == &123);
    }

    #[test]
    fn test_find_index() {
        let mut list: List<i64> = List::new();
        let _  = list.insert(0, 123, &Replica::new(1, 0)).unwrap();
        let op = list.insert(1, 456, &Replica::new(1, 0)).unwrap();
        let _  = list.insert(2, 789, &Replica::new(1, 0)).unwrap();

        let (uid, _) = get_elt(op);
        assert!(list.find_index(&uid) == Ok(1));
        assert!(list.find_index(&*uid::MIN) == Err(0));
        assert!(list.find_index(&*uid::MAX) == Err(3));
    }

    #[test]
    fn test_insert_prepend() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123, &Replica::new(1, 0)).unwrap();
        let op2 = list.insert(0, 456, &Replica::new(1, 0)).unwrap();
        let op3 = list.insert(0, 789, &Replica::new(1, 0)).unwrap();

        assert!(list.len() == 3);
        assert!(list.0[0].1 == 789);
        assert!(list.0[1].1 == 456);
        assert!(list.0[2].1 == 123);

        let (uid1, v1) = get_elt(op1);
        let (uid2, v2) = get_elt(op2);
        let (uid3, v3) = get_elt(op3);

        assert!(v1 == 123 && v2 == 456 && v3 == 789);
        assert!(*uid::MAX > uid1);
        assert!(uid1 > uid2);
        assert!(uid2 > uid3);
        assert!(uid3 > *uid::MIN);
    }

    #[test]
    fn test_insert_append() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123, &Replica::new(1, 0)).unwrap();
        let op2 = list.insert(1, 456, &Replica::new(1, 0)).unwrap();
        let op3 = list.insert(2, 789, &Replica::new(1, 0)).unwrap();

        assert!(list.0[0].1 == 123);
        assert!(list.0[1].1 == 456);
        assert!(list.0[2].1 == 789);

        let (uid1, _) = get_elt(op1);
        let (uid2, _) = get_elt(op2);
        let (uid3, _) = get_elt(op3);

        assert!(*uid::MIN < uid1);
        assert!(uid1 < uid2);
        assert!(uid2 < uid3);
        assert!(uid3 < *uid::MAX);
    }

    #[test]
    fn test_insert_middle() {
        let mut list: List<i64> = List::new();
        let op1 = list.insert(0, 123, &Replica::new(1, 0)).unwrap();
        let op2 = list.insert(1, 456, &Replica::new(1, 0)).unwrap();
        let op3 = list.insert(1, 789, &Replica::new(1, 0)).unwrap();

        assert!(list.0[0].1 == 123);
        assert!(list.0[1].1 == 789);
        assert!(list.0[2].1 == 456);

        let (uid1, _) = get_elt(op1);
        let (uid2, _) = get_elt(op2);
        let (uid3, _) = get_elt(op3);

        assert!(*uid::MIN < uid1);
        assert!(uid1 < uid3);
        assert!(uid3 < uid2);
        assert!(uid2 < *uid::MAX);
    }

    #[test]
    fn test_insert_out_of_bounds() {
        let mut list: List<i64> = List::new();
        let result = list.insert(1, 123, &Replica::new(1, 0));
        assert!(result.unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_remove() {
        let mut list: List<i64> = List::new();
        let _   = list.insert(0, 123, &Replica::new(1, 0)).unwrap();
        let op1 = list.insert(1, 456, &Replica::new(1, 0)).unwrap();
        let _   = list.insert(2, 789, &Replica::new(1, 0)).unwrap();
        let op2 = list.remove(1).unwrap();

        let (uid1, _) = get_elt(op1);
        let (uid2, _) = get_elt(op2);

        assert!(list.len() == 2);
        assert!(list.0[0].1 == 123);
        assert!(list.0[1].1 == 789);
        assert!(uid1 == uid2);
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut list: List<i64> = List::new();
        let result = list.remove(0);
        assert!(result.unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn test_execute_remote_insert() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op = list1.insert(0, "a".to_owned(), &Replica::new(1,0)).unwrap();
        let local_op = list2.execute_remote(&remote_op).unwrap();

        assert!(list2.len() == 1);
        assert!(list2.0[0].1 == "a");
        let (i, v) = if let LocalOp::Insert{index: i, value: v} = local_op { (i, v) } else { panic!() };
        assert!(i == 0);
        assert!(v == "a");
    }

    #[test]
    fn test_execute_remote_insert_dupe() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op = list1.insert(0, "a".to_owned(), &Replica::new(1,0)).unwrap();

        let _ = list2.execute_remote(&remote_op).unwrap();
        assert!(list2.execute_remote(&remote_op).is_none());
        assert!(list2.len() == 1);
    }

    #[test]
    fn test_execute_remote_remove() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op1 = list1.insert(0, "a".to_owned(), &Replica::new(1,0)).unwrap();
        let remote_op2 = list1.remove(0).unwrap();
        let _ = list2.execute_remote(&remote_op1).unwrap();
        let local_op = list2.execute_remote(&remote_op2).unwrap();

        let i = if let LocalOp::Remove{index: i} = local_op { i } else { panic!() };

        assert!(list2.len() == 0);
        assert!(i == 0);
    }

    #[test]
    fn test_execute_remote_remove_dupe() {
        let mut list1: List<String> = List::new();
        let mut list2: List<String> = List::new();
        let remote_op1 = list1.insert(0, "a".to_owned(), &Replica::new(1,0)).unwrap();
        let remote_op2 = list1.remove(0).unwrap();

        let _ = list2.execute_remote(&remote_op1).unwrap();
        let _ = list2.execute_remote(&remote_op2).unwrap();
        assert!(list2.execute_remote(&remote_op2).is_none());
        assert!(list2.len() == 0);
    }

    fn get_elt<T>(remote_op: RemoteOp<T>) -> Element<T> {
        match remote_op {
            RemoteOp::Insert(element) => element,
            RemoteOp::Remove(element) => element,
        }
    }
}
