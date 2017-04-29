use Error;
use Replica;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

#[derive(Debug, Clone)]
pub struct Set<T: Debug + Clone + Eq + Hash>(HashMap<T, Vec<Replica>>);

#[derive(Debug, Clone)]
pub struct RemoteOp<T> {
    value:  T,
    remove: Vec<Replica>,
    insert: Option<Replica>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalOp<T: Debug + Clone + Eq + Hash> {
    Insert(T),
    Remove(T),
}

impl<T> Set<T> where T: Debug + Clone + Eq + Hash {

    /// Constructs and returns a new set.
    pub fn new() -> Self {
        Set(HashMap::new())
    }

    /// Returns true if the set contains the value.
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains_key(value)
    }

    /// Consumes the set and returns a HashSet of its values.
    pub fn into(self) -> HashSet<T> {
        let mut hash_set: HashSet<T> = HashSet::new();
        for (key,_) in self.0.into_iter() {
            hash_set.insert(key);
        }
        hash_set
    }

    /// Inserts a value into the set and returns an op that can
    /// be sent to remote sites for replication. If the set
    /// already contains the value, nothing changes except for
    /// the CRDT metadta.
    pub fn insert<V: Into<T>>(&mut self, value: V, replica: &Replica) -> RemoteOp<T> {
        let value = value.into();
        let replicas = self.0.entry(value.clone()).or_insert(vec![]);
        let new_replicas = vec![replica.clone()];
        let remove = mem::replace(replicas, new_replicas);
        let insert = Some(replica.clone());
        RemoteOp{value, remove, insert}
    }

    /// Removes a value from the set and returns an op that can
    /// be sent to remote sites for replication. If the set does
    /// not contain the value, it returns a DoesNotExist error.
    pub fn remove(&mut self, value: &T) -> Result<RemoteOp<T>, Error> {
        let remove = self.0.remove(value).ok_or(Error::DoesNotExist)?;
        let value = value.clone();
        Ok(RemoteOp{value, remove, insert: None})
    }

    /// Updates the set and returns the equivalent local op.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> LocalOp<T> {
        let value_should_be_removed = {
            let replicas = self.0.entry(op.value.clone()).or_insert(vec![]);

            for replica in &op.remove {
                if let Ok(index) = replicas.binary_search_by(|r| r.cmp(replica)) {
                    replicas.remove(index);
                }
            }

            if let Some(ref insert) = op.insert {
                if let Err(index) = replicas.binary_search_by(|r| r.cmp(insert)) {
                    replicas.insert(index, insert.clone());
                }
            }

            replicas.is_empty()
        };

        if value_should_be_removed {
            let _ = self.0.remove(&op.value);
            LocalOp::Remove(op.value.clone())
        } else {
            LocalOp::Insert(op.value.clone())
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
