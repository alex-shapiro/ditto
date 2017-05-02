//! A `Set` stores a collection of distinct elements.
//! The elements themselves are immutable.

use Error;
use Replica;
use traits::*;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Set<T: Debug + Clone + Eq + Hash> {
    value: SetValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetValue<T: Debug + Clone + Eq + Hash>(HashMap<T, Vec<Replica>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteOp<T> {
    Insert{value: T, replica: Replica},
    Remove{value: T, replicas: Vec<Replica>},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<T: Debug + Clone + Eq + Hash> {
    Insert(T),
    Remove(T),
}

impl<T> Set<T> where T: Debug + Clone + Eq + Hash {

    /// Constructs and returns a new set CRDT.
    /// The set has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1, 0);
        let value = SetValue(HashMap::new());
        Set{replica, value, awaiting_site: vec![]}
    }

    /// Returns true iff the set contains the value.
    pub fn contains(&self, value: &T) -> bool {
        self.value.0.contains_key(value)
    }

    /// Inserts a value into the set and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the set does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert(&mut self, value: T) -> Result<RemoteOp<T>, Error> {
        let op = self.value.insert(value, &self.replica)?;
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }

    /// Removes a value from the set and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the set does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, value: &T) -> Result<RemoteOp<T>, Error> {
        let op = self.value.remove(value)?;
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl<T> Crdt for Set<T> where T: Debug + Clone + Eq + Hash {
    type Value = SetValue<T>;

    fn site(&self) -> u32 {
        self.replica.site
    }

    fn value(&self) -> &SetValue<T> {
        &self.value
    }

    fn clone_value(&self) -> SetValue<T> {
        self.value.clone()
    }

    fn from_value(value: SetValue<T>, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        Set{value, replica, awaiting_site: vec![]}
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

impl<T> SetValue<T> where T: Debug + Clone + Eq + Hash {

    /// Constructs and returns a new set.
    pub fn new() -> Self {
        SetValue(HashMap::new())
    }

    /// Returns true if the set contains the value.
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains_key(value)
    }

    /// Inserts a value into the set and returns an op that can
    /// be sent to remote sites for replication. If the set already
    /// contains the value, it returns an AlreadyExists error.
    pub fn insert(&mut self, value: T, replica: &Replica) -> Result<RemoteOp<T>, Error> {
        if self.0.contains_key(&value) { return Err(Error::AlreadyExists) }
        self.0.insert(value.clone(), vec![replica.clone()]);
        Ok(RemoteOp::Insert{value, replica: replica.clone()})
    }

    /// Removes a value from the set and returns an op that can
    /// be sent to remote sites for replication. If the set does
    /// not contain the value, it returns a DoesNotExist error.
    pub fn remove(&mut self, value: &T) -> Result<RemoteOp<T>, Error> {
        let replicas = self.0.remove(value).ok_or(Error::DoesNotExist)?;
        Ok(RemoteOp::Remove{value: value.clone(), replicas})
    }

    /// Updates the set and returns the equivalent local op.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        match *op {
            RemoteOp::Insert{ref value, ref replica} => {
                let replicas = self.0.entry(value.clone()).or_insert(vec![]);
                match replicas.binary_search_by(|r| r.cmp(replica)) {
                    Ok(_) => None,
                    Err(_) => {
                        replicas.push(replica.clone());
                        Some(LocalOp::Insert(value.clone()))
                    }
                }
            }
            RemoteOp::Remove{ref value, ref replicas} => {
                let should_remove_value = {
                    let existing_replicas = try_opt!(self.0.get_mut(value));
                    for replica in replicas {
                        if let Ok(index) = existing_replicas.binary_search_by(|r| r.cmp(replica)) {
                            existing_replicas.remove(index);
                        }
                    }
                    existing_replicas.is_empty()
                };

                if should_remove_value {
                    self.0.remove(value);
                    Some(LocalOp::Remove(value.clone()))
                } else {
                    None
                }
            }
        }
    }
}

impl<T> CrdtValue for SetValue<T> where T: Debug + Clone + Eq + Hash {
    type LocalValue = HashSet<T>;
    type RemoteOp = RemoteOp<T>;
    type LocalOp = LocalOp<T>;

    fn local_value(&self) -> HashSet<T> {
        let mut hash_set = HashSet::new();
        for key in self.0.keys() {
            hash_set.insert(key.clone());
        }
        hash_set
    }

    fn add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        if let RemoteOp::Insert{ref value, ref replica} = *op {
            if let Some(ref mut replicas) = self.0.get_mut(value) {
                if let Ok(index) = replicas.binary_search_by(|r| r.cmp(replica)) {
                    replicas[index].site = site;
                }
            }
        }
    }
}


impl<T> CrdtRemoteOp for RemoteOp<T> where T: Debug + Clone + Eq + Hash {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert{ref mut replica, ..} => {
                if replica.site == 0 { replica.site == site; }
            }
            RemoteOp::Remove{ref mut replicas, ..} => {
                for replica in replicas {
                    if replica.site == 0 { replica.site == site; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into() {
        let mut set: Set<i64> = Set::new();
        let _ = set.insert(123, &Replica{site: 1, counter: 0});
        let _ = set.insert(456, &Replica{site: 2, counter: 0});
        let _ = set.insert(789, &Replica{site: 1, counter: 1});
        let hash_set: HashSet<i64> = set.into();
        assert!(hash_set.len() == 3);
        assert!(hash_set.contains(&123));
        assert!(hash_set.contains(&456));
        assert!(hash_set.contains(&789));
    }

    #[test]
    fn test_insert() {
        let mut set: Set<String> = Set::new();
        let remote_op = set.insert("Bob", &Replica{site: 1, counter: 0});
        assert!(set.contains(&"Bob".to_owned()));
        assert!(remote_op.value == "Bob");
        assert!(remote_op.remove.is_empty());
        assert!(remote_op.insert == Some(Replica{site: 1, counter: 0}));
    }

    #[test]
    fn test_insert_overwrite() {
        let mut set1: Set<i64> = Set::new();
        let mut set2: Set<i64> = Set::new();
        let remote_op1 = set1.insert(123, &Replica{site: 1, counter: 0});
        let _          = set2.execute_remote(&remote_op1);
        let remote_op2 = set2.insert(123, &Replica{site: 2, counter: 0});

        assert!(set2.contains(&123));
        assert!(remote_op2.value == 123);
        assert!(remote_op2.remove == vec![Replica{site: 1, counter: 0}]);
        assert!(remote_op2.insert == Some(Replica{site: 2, counter: 0}));
    }

    #[test]
    fn test_remove() {
        let mut set: Set<bool> = Set::new();
        let _ = set.insert(true, &Replica{site: 1, counter: 0});
        let remote_op = set.remove(&true).unwrap();

        assert!(!set.contains(&true));
        assert!(remote_op.value == true);
        assert!(remote_op.remove == vec![Replica{site: 1, counter: 0}]);
        assert!(remote_op.insert == None);
    }

    #[test]
    fn test_remove_does_not_exist() {
        let mut set: Set<bool> = Set::new();
        assert!(set.remove(&true).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_execute_remote() {
        let mut set1: Set<u8> = Set::new();
        let mut set2: Set<u8> = Set::new();
        let remote_op = set1.insert(12, &Replica{site: 1, counter: 0});
        let local_op = set2.execute_remote(&remote_op);

        assert!(set2.contains(&12));
        assert!(local_op == LocalOp::Insert(12));
    }

    #[test]
    fn test_execute_remote_concurrent() {
        let mut set1: Set<u8> = Set::new();
        let mut set2: Set<u8> = Set::new();
        let mut set3: Set<u8> = Set::new();

        let remote_op1 = set1.insert(99, &Replica{site: 1, counter: 0});
        let remote_op2 = set2.insert(99, &Replica{site: 2, counter: 0});
        let local_op1  = set3.execute_remote(&remote_op1);
        let local_op2  = set3.execute_remote(&remote_op2);

        assert!(set3.contains(&99));
        assert!(local_op1 == LocalOp::Insert(99));
        assert!(local_op2 == LocalOp::Insert(99));

        let remote_op3 = set3.insert(99, &Replica{site: 3, counter: 0});
        assert!(remote_op3.value == 99);
        assert!(remote_op3.remove == vec![Replica{site: 1, counter: 0}, Replica{site: 2, counter: 0}]);
        assert!(remote_op3.insert == Some(Replica{site: 3, counter: 0}));
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut set1: Set<u8> = Set::new();
        let mut set2: Set<u8> = Set::new();
        let remote_op = set1.insert(12, &Replica{site: 1, counter: 0});
        let local_op1 = set2.execute_remote(&remote_op);
        let local_op2 = set2.execute_remote(&remote_op);

        assert!(set2.contains(&12));
        assert!(local_op1 == LocalOp::Insert(12));
        assert!(local_op2 == LocalOp::Insert(12));
    }
}
