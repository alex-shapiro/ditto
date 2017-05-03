//! A `Set` stores a collection of distinct elements.
//! The elements themselves are immutable.

use Error;
use Replica;
use traits::*;

use serde::{Serialize, Deserialize, Serializer, Deserializer};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

pub trait SetElement: Debug + Clone + Eq + Hash {}
impl<T: Debug + Clone + Eq + Hash> SetElement for T {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Set<T: SetElement> {
    value: SetValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetValue<T: SetElement>(HashMap<T, Vec<Replica>>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RemoteOp<T> {
    Insert{value: T, replica: Replica},
    Remove{value: T, replicas: Vec<Replica>},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<T> {
    Insert(T),
    Remove(T),
}

impl<T: SetElement> Set<T> {

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

impl<T: SetElement> Crdt for Set<T> {
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

impl<T: SetElement> SetValue<T> {

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
                        if replicas.len() == 1 {
                            Some(LocalOp::Insert(value.clone()))
                        } else {
                            None
                        }
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

impl<T: SetElement> CrdtValue for SetValue<T> {
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


impl<T: SetElement> CrdtRemoteOp for RemoteOp<T> {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert{ref mut replica, ..} => {
                if replica.site == 0 { replica.site = site; }
            }
            RemoteOp::Remove{ref mut replicas, ..} => {
                for replica in replicas {
                    if replica.site == 0 { replica.site = site; }
                }
            }
        }
    }
}

impl<T: SetElement + Serialize> Serialize for SetValue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for value_replicas in self.0.iter() {
            seq.serialize_element(&value_replicas)?;
        }
        seq.end()
    }
}

impl<'de, T> Deserialize<'de> for SetValue<T> where T: SetElement + Deserialize<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        use serde::de::{Visitor, SeqAccess};
        use std::fmt;
        use std::marker::PhantomData;

        struct SetValueVisitor<T: SetElement> {
            marker: PhantomData<SetValue<T>>,
        }

        impl<T: SetElement> SetValueVisitor<T> {
            fn new() -> Self {
                SetValueVisitor{marker: PhantomData}
            }
        }

        impl<'de, T> Visitor<'de> for SetValueVisitor<T> where T: SetElement + Deserialize<'de> {
            type Value = SetValue<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a list of (T, Replica) tuples")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqAccess<'de> {
                let mut hash_map = HashMap::with_capacity(visitor.size_hint().unwrap_or(0));
                while let Some((value, replica)) = visitor.next_element()? {
                    hash_map.insert(value, replica);
                }
                Ok(SetValue(hash_map))
            }
        }

        deserializer.deserialize_seq(SetValueVisitor::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use rmp_serde as rmps;

    #[test]
    fn test_new() {
        let set: Set<u8> = Set::new();
        assert!(set.site() == 1);
    }

    #[test]
    fn test_contains() {
        let mut set: Set<u8> = Set::new();
        let _ = set.insert(123);
        assert!(set.contains(&123));
        assert!(!set.contains(&56));
    }

    #[test]
    fn test_insert() {
        let mut set: Set<u32> = Set::new();
        let remote_op = set.insert(123).unwrap();
        let (value, replica) = insert_fields(remote_op);
        assert!(value == 123);
        assert!(replica.site == 1);
    }

    #[test]
    fn test_insert_already_exists() {
        let mut set: Set<u32> = Set::new();
        assert!(set.insert(123).is_ok());
        assert!(set.insert(123).unwrap_err() == Error::AlreadyExists);
        assert!(set.contains(&123));
    }

    #[test]
    fn test_insert_awaiting_site() {
        let set1: Set<u32> = Set::new();
        let mut set2: Set<u32> = Set::from_value(set1.clone_value(), 0);
        assert!(set2.insert(123).unwrap_err() == Error::AwaitingSite);
        assert!(set2.contains(&123));
    }

    #[test]
    fn test_remove() {
        let mut set: Set<u32> = Set::new();
        let remote_op1 = set.insert(123).unwrap();
        let remote_op2 = set.remove(&123).unwrap();
        let (_, replica) = insert_fields(remote_op1);
        let (value, replicas) = remove_fields(remote_op2);

        assert!(value == 123);
        assert!(replicas.len() == 1);
        assert!(replica == replicas[0]);
    }

    #[test]
    fn test_remove_does_not_exist() {
        let mut set: Set<u32> = Set::new();
        assert!(set.remove(&123).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_remove_awaiting_site() {
        let set1: Set<u32> = Set::new();
        let mut set2: Set<u32> = Set::from_value(set1.clone_value(), 0);
        let _ = set2.insert(123);
        assert!(set2.remove(&123).unwrap_err() == Error::AwaitingSite);
        assert!(!set2.contains(&123));
    }

    #[test]
    fn test_site() {
        let set1: Set<u64> = Set::new();
        let set2: Set<u64> = Set::from_value(set1.clone_value(), 8403);
        assert!(set1.site() == 1);
        assert!(set2.site() == 8403);
    }

    #[test]
    fn execute_remote_insert() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 8403);
        let remote_op = set1.insert(22).unwrap();
        let local_op = set2.execute_remote(&remote_op).unwrap();
        assert_matches!(local_op, LocalOp::Insert(22));
    }

    #[test]
    fn execute_remote_insert_value_already_exists() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let remote_op = set1.insert(22).unwrap();
        let _         = set2.insert(22).unwrap();

        assert!(set2.execute_remote(&remote_op).is_none());
        assert!(set2.value.0.get(&22).unwrap().len() == 2);
    }

    #[test]
    fn execute_remote_insert_dupe() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let remote_op = set1.insert(22).unwrap();

        assert!(set2.execute_remote(&remote_op).is_some());
        assert!(set2.execute_remote(&remote_op).is_none());
        assert!(set2.value.0.get(&22).unwrap().len() == 1);
    }

    #[test]
    fn execute_remote_remove() {
        let mut set1: Set<u64> = Set::new();
        let _ = set1.insert(10).unwrap();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let remote_op = set1.remove(&10).unwrap();
        let local_op = set2.execute_remote(&remote_op).unwrap();

        assert!(!set2.contains(&10));
        assert_matches!(local_op, LocalOp::Remove(10));
    }

    #[test]
    fn execute_remote_remove_does_not_exist() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let _ = set1.insert(10).unwrap();
        let remote_op = set1.remove(&10).unwrap();
        assert!(set2.execute_remote(&remote_op).is_none());
        assert!(!set2.contains(&10));
    }

    #[test]
    fn execute_remote_remove_some_replicas_remain() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let _ = set1.insert(10).unwrap();
        let _ = set2.insert(10).unwrap();
        let remote_op = set1.remove(&10).unwrap();
        assert!(set2.execute_remote(&remote_op).is_none());
        assert!(set2.contains(&10));
    }

    #[test]
    fn execute_remote_remove_dupe() {
        let mut set1: Set<u64> = Set::new();
        let mut set2: Set<u64> = Set::from_value(set1.clone_value(), 2);
        let remote_op1 = set1.insert(10).unwrap();
        let remote_op2 = set1.remove(&10).unwrap();
        assert!(set2.execute_remote(&remote_op1).is_some());
        assert!(set2.execute_remote(&remote_op2).is_some());
        assert!(set2.execute_remote(&remote_op2).is_none());
        assert!(!set2.contains(&10));
    }

    #[test]
    fn test_add_site() {
        let mut set: Set<u64> = Set::from_value(Set::new().clone_value(), 0);
        let _ = set.insert(10);
        let _ = set.insert(20);
        let _ = set.remove(&10);
        let mut remote_ops = set.add_site(5).unwrap().into_iter();

        let (value1, replica1) = insert_fields(remote_ops.next().unwrap());
        let (value2, replica2) = insert_fields(remote_ops.next().unwrap());
        let (value3, replicas) = remove_fields(remote_ops.next().unwrap());

        assert!(set.value.0.get(&20).unwrap()[0].site == 5);
        assert!(value1 == 10 && replica1 == Replica::new(5, 0));
        assert!(value2 == 20 && replica2 == Replica::new(5, 1));
        assert!(value3 == 10 && replicas == vec![Replica::new(5, 0)]);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut set: Set<u64> = Set::from_value(Set::new().clone_value(), 123);
        let _ = set.insert(10);
        let _ = set.insert(20);
        let _ = set.remove(&10);
        assert!(set.add_site(42).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let mut set1: Set<i64> = Set::new();
        let _ = set1.insert(182);
        let _ = set1.insert(-41);

        let s_json = serde_json::to_string(&set1).unwrap();
        let s_msgpack = rmps::to_vec(&set1).unwrap();
        let set2: Set<i64> = serde_json::from_str(&s_json).unwrap();
        let set3: Set<i64> = rmps::from_slice(&s_msgpack).unwrap();

        assert!(set1 == set2);
        assert!(set1 == set3);
    }

    #[test]
    fn test_serialize_value() {
        let mut set: Set<Vec<bool>> = Set::new();
        let _ = set.insert(vec![true, false, true]);
        let _ = set.insert(vec![false, false, true, false]);

        let s_json = serde_json::to_string(set.value()).unwrap();
        let s_msgpack = rmps::to_vec(&set.value()).unwrap();
        let value2: SetValue<Vec<bool>> = serde_json::from_str(&s_json).unwrap();
        let value3: SetValue<Vec<bool>> = rmps::from_slice(&s_msgpack).unwrap();

        assert!(*set.value() == value2);
        assert!(*set.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut set: Set<(i64, String)> = Set::new();
        let remote_op1 = set.insert((123,"abc".to_owned())).unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmps::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp<(i64, String)> = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp<(i64, String)> = rmps::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let local_op1: LocalOp<u8> = LocalOp::Insert(142);

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmps::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp<u8> = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp<u8> = rmps::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }

    fn insert_fields<T>(remote_op: RemoteOp<T>) -> (T, Replica) {
        match remote_op {
            RemoteOp::Insert{value, replica} => (value, replica),
            _ => panic!(),
        }
    }

    fn remove_fields<T>(remote_op: RemoteOp<T>) -> (T, Vec<Replica>) {
        match remote_op {
            RemoteOp::Remove{value, replicas} => (value, replicas),
            _ => panic!(),
        }
    }
}
