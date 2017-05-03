//! A `Map` stores a collection of key-value pairs.
//! The values in the map are immutable.

use Error;
use Replica;
use map_tuple_vec;
use traits::*;
use util::remove_elements;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

pub trait Key: Debug + Clone + Eq + Hash + Serialize + DeserializeOwned {}
impl<T: Debug + Clone + Eq + Hash + Serialize + DeserializeOwned> Key for T {}

pub trait Value: Debug + Clone + PartialEq + Eq + PartialOrd + Ord + Serialize + DeserializeOwned {}
impl<T: Debug + Clone + PartialEq + Eq + PartialOrd + Ord + Serialize + DeserializeOwned> Value for T {}

#[derive(Debug, Clone, PartialEq)]
pub struct Map<K: Key, V: Value> {
    value: MapValue<K, V>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<K, V>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapValue<K: Key, V: Value> {
    // #[serde(with = "map_tuple_vec")]
    inner: HashMap<K, Vec<Element<V>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteOp<K: Key, V: Value> {
    Insert{key: K, element: Element<V>, removed: Vec<Element<V>>},
    Remove{key: K, removed: Vec<Element<V>>},
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalOp<K: Key, V: Value> {
    Insert(K, V),
    Remove(K),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Element<V: Value + DeserializeOwned>(Replica, V);

impl<K,V> Map<K, V> where K: Key, V: Value {

    /// Constructs and returns a new map.
    /// Th map has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1, 0);
        let value = MapValue::new();
        Map{replica, value, awaiting_site: vec![]}
    }

    /// Returns true iff the map has the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.value.inner.contains_key(key)
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.value.inner.get(key).and_then(|elements| Some(&elements[0].1))
    }

    /// Inserts a key-value pair into the map and returns a remote
    /// op that can be sent to remote sites for replication. If the
    /// map does not have a site allocated, it caches the op and
    /// returns an `AwaitingSite` error.
    pub fn insert(&mut self, key: K, value: V) -> Result<RemoteOp<K, V>, Error> {
        let op = self.value.insert(key, value, &self.replica)?;
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }

    /// Removes a key from the map and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the set does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, key: &K) -> Result<RemoteOp<K,V>, Error> {
        let op = self.value.remove(key)?;
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl<K, V> Crdt for Map<K,V> where K: Key, V: Value {
    type Value = MapValue<K,V>;

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
        Map{value, replica, awaiting_site: vec![]}
    }

    fn execute_remote(&mut self, op: &RemoteOp<K,V>) -> Option<LocalOp<K,V>> {
        self.value.execute_remote(op)
    }

    fn add_site(&mut self, site: u32) -> Result<Vec<RemoteOp<K,V>>, Error> {
        if self.replica.site != 0 { return Err(Error::AlreadyHasSite) };
        let mut ops = mem::replace(&mut self.awaiting_site, vec![]);
        for op in &mut ops {
            self.value.add_site(op, site);
            op.add_site(site);
        }
        Ok(ops)
    }
}

impl<K, V> MapValue<K, V> where K: Key, V: Value {

    /// Constructs and returns a new map value.
    pub fn new() -> Self {
        MapValue{inner: HashMap::new()}
    }

    /// Inserts a key-value pair into the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map had this key present, the value is updated. If the
    /// value was already identical, it returns an AlreadyExists
    /// error.
    pub fn insert(&mut self, key: K, value: V, replica: &Replica) -> Result<RemoteOp<K, V>, Error> {
        if let Some(values) = self.inner.get(&key) {
            if values[0].1 == value { return Err(Error::AlreadyExists) }
        }

        let element = Element(replica.clone(), value.clone());
        let old_elements = self.inner.entry(key.clone()).or_insert(vec![]);
        let new_elements = vec![element.clone()];
        let removed = mem::replace(old_elements, new_elements);
        Ok(RemoteOp::Insert{key, element, removed})
    }

    /// Removes a key-value pair from the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map did not contain the key, it returns a DoesNotExist error.
    pub fn remove(&mut self, key: &K) -> Result<RemoteOp<K, V>, Error> {
        let removed = self.inner.remove(key).ok_or(Error::DoesNotExist)?;
        Ok(RemoteOp::Remove{key: key.clone(), removed})
    }

    /// Updates the map and returns the equivalent local op.
    pub fn execute_remote(&mut self, op: &RemoteOp<K, V>) -> Option<LocalOp<K, V>> {
        match *op {
            RemoteOp::Insert{ref key, ref element, ref removed} => {
                let elements = self.inner.entry(key.clone()).or_insert(vec![]);
                remove_elements(elements, removed);

                let index = try_opt!(elements.binary_search_by(|e| e.0.cmp(&element.0)).err());
                elements.insert(index, element.clone());
                if index == 0 {
                    Some(LocalOp::Insert(key.clone(), element.1.clone()))
                } else {
                    None
                }
            }
            RemoteOp::Remove{ref key, ref removed} => {
                let should_remove_key = {
                    let existing_elements = try_opt!(self.inner.get_mut(key));
                    remove_elements(existing_elements, removed);
                    existing_elements.is_empty()
                };

                if !should_remove_key { return None }
                let _ = self.remove(key);
                Some(LocalOp::Remove(key.clone()))
            }
        }
    }
}

impl<K, V> CrdtValue for MapValue<K, V> where K: Key, V: Value {
    type LocalValue = HashMap<K, V>;
    type RemoteOp = RemoteOp<K, V>;
    type LocalOp = LocalOp<K, V>;

    fn local_value(&self) -> HashMap<K, V> {
        let mut hash_map = HashMap::new();
        for (key, elements) in self.inner.iter() {
            hash_map.insert(key.clone(), elements[0].1.clone());
        }
        hash_map
    }

    fn add_site(&mut self, op: &RemoteOp<K, V>, site: u32) {
        if let RemoteOp::Insert{ref key, ref element, ..} = *op {
            if let Some(ref mut elements) = self.inner.get_mut(key) {
                if let Ok(index) = elements.binary_search_by(|e| e.0.cmp(&element.0)) {
                    elements[index].0.site = site;
                }
            }
        }
    }
}

impl<K, V> CrdtRemoteOp for RemoteOp<K, V> where K: Key, V: Value {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert{ref mut element, ref mut removed, ..} => {
                if element.0.site == 0 { element.0.site = site; }
                for element in removed {
                    if element.0.site == 0 { element.0.site = site; }
                }
            }
            RemoteOp::Remove{ref mut removed, ..} => {
                for element in removed {
                    if element.0.site == 0 { element.0.site = site; }
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
        let mut map: Map<&'static str, u16> = Map::new();
        map.insert("foo", 2, &Replica{site: 1, counter: 0});
        map.insert("bar", 5, &Replica{site: 1, counter: 1});
        map.insert("baz", 7, &Replica{site: 1, counter: 2});

        let hashmap = map.into();
        assert!(hashmap.len() == 3);
        assert!(hashmap.get(&"foo") == Some(&2));
        assert!(hashmap.get(&"bar") == Some(&5));
        assert!(hashmap.get(&"baz") == Some(&7));
    }

    #[test]
    fn test_contains_key() {
        let mut map: Map<&'static str, u16> = Map::new();
        map.insert("foo", 2, &Replica{site: 1, counter: 0});
        assert!(map.contains_key(&"foo"));
        assert!(!map.contains_key(&"bar"));
    }

    #[test]
    fn test_get() {
        let mut map: Map<i32, u64> = Map::new();
        map.insert(26, 42, &Replica{site: 1, counter: 0});
        assert!(map.get(&26) == Some(&42));
        assert!(map.get(&56) == None);
    }

    #[test]
    fn test_get_mut() {
        let mut map: Map<i32, u64> = Map::new();
        map.insert(26, 42, &Replica{site: 1, counter: 0});
        assert!(map.get_mut(&26) == Some(&mut 42));
        assert!(map.get_mut(&56) == None);
    }

    #[test]
    fn test_insert() {
        let mut map: Map<bool, i8> = Map::new();
        let remote_op = map.insert(true, 1, &Replica{site: 20, counter: 30});
        assert!(map.0.get(&true) == Some(&vec![(Replica{site: 20, counter: 30}, 1)]));
        assert!(remote_op.key == true);
        assert!(remote_op.remove == vec![]);
        assert!(remote_op.insert == Some((Replica{site: 20, counter: 30}, 1)));
    }

    #[test]
    fn test_insert_overwrite() {
        let mut map: Map<bool, i8> = Map::new();
        let _         = map.insert(true, 3, &Replica::new(1, 0));
        let remote_op = map.insert(true, 8, &Replica::new(2, 0));

        assert!(map.0.get(&true) == Some(&vec![(Replica::new(2, 0), 8)]));
        assert!(remote_op.key == true);
        assert!(remote_op.remove == vec![(Replica::new(1, 0), 3)]);
        assert!(remote_op.insert == Some((Replica::new(2, 0), 8)));
    }

    #[test]
    fn test_remove() {
        let mut map: Map<bool, i8> = Map::new();
        let _ = map.insert(true, 3, &Replica::new(1,0));
        let remote_op = map.remove(&true).unwrap();

        assert!(map.0.get(&true) == None);
        assert!(remote_op.key == true);
        assert!(remote_op.remove == vec![(Replica::new(1,0), 3)]);
        assert!(remote_op.insert == None);
    }

    #[test]
    fn test_remove_does_not_exist() {
        let mut map: Map<bool, i8> = Map::new();
        assert!(map.remove(&true).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_execute_remote() {
        let mut map1: Map<&'static str, u32> = Map::new();
        let mut map2: Map<&'static str, u32> = Map::new();
        let remote_op1 = map1.insert("foo", 1, &Replica::new(1,0));
        let remote_op2 = map1.remove(&"foo").unwrap();

        let local_op1 = map2.execute_remote(&remote_op1);
        assert!(map2.get(&"foo") == Some(&1));
        assert!(local_op1 == LocalOp::Insert("foo", 1));

        let local_op2 = map2.execute_remote(&remote_op2);
        assert!(map2.get(&"foo") == None);
        assert!(local_op2 == LocalOp::Remove("foo"));
    }

    #[test]
    fn test_execute_remote_concurrent() {
        let mut map1: Map<&'static str, u32> = Map::new();
        let mut map2: Map<&'static str, u32> = Map::new();
        let mut map3: Map<&'static str, u32> = Map::new();

        let remote_op1 = map1.insert("foo", 1, &Replica::new(1,0));
        let remote_op2 = map2.insert("foo", 2, &Replica::new(2,0));
        let remote_op3 = map1.remove(&"foo").unwrap();

        let local_op1 = map3.execute_remote(&remote_op1);
        let local_op2 = map3.execute_remote(&remote_op2);
        let local_op3 = map3.execute_remote(&remote_op3);

        assert!(map3.get(&"foo") == Some(&2));
        assert!(local_op1 == LocalOp::Insert("foo", 1));
        assert!(local_op2 == LocalOp::Insert("foo", 1));
        assert!(local_op3 == LocalOp::Insert("foo", 2));
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut map1: Map<&'static str, u32> = Map::new();
        let mut map2: Map<&'static str, u32> = Map::new();
        let remote_op = map1.insert("foo", 1, &Replica::new(1,0));
        let local_op1 = map2.execute_remote(&remote_op);
        let local_op2 = map2.execute_remote(&remote_op);

        assert!(map2.get(&"foo") == Some(&1));
        assert!(local_op1 == LocalOp::Insert("foo", 1));
        assert!(local_op2 == LocalOp::Insert("foo", 1));
    }
}
