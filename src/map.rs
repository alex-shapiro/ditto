//! A `Map` stores a collection of key-value pairs.
//! The values in the map are immutable.

use {Error, Replica, Tombstones};
use map_tuple_vec;
use traits::*;

use serde::ser::Serialize;
use serde::de::DeserializeOwned;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::hash_map::{self, HashMap};
use std::hash::Hash;
use std::mem;

pub trait Key: Clone + Eq + Hash + Serialize + DeserializeOwned {}
impl<T: Clone + Eq + Hash + Serialize + DeserializeOwned> Key for T {}

pub trait Value: Clone + PartialEq + Serialize + DeserializeOwned {}
impl<T: Clone + PartialEq + Serialize + DeserializeOwned> Value for T {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Map<K: Key, V: Value> {
    value: MapValue<K, V>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<K, V>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapValue<K: Key, V: Value> {
    #[serde(with = "map_tuple_vec")]
    pub elements: HashMap<K, Vec<Element<V>>>,
    tombstones: Tombstones,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteOp<K, V> {
    Insert{key: K, element: Element<V>, removed: Vec<Replica>},
    Remove{key: K, removed: Vec<Replica>},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<K, V> {
    Insert{key: K, value: V},
    Remove{key: K},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element<V>(pub Replica, pub V);

impl<V> PartialEq for Element<V> {
    fn eq(&self, other: &Element<V>) -> bool {
        self.0 == other.0
    }
}

impl<V> Eq for Element<V> {}

impl<V> PartialOrd for Element<V> {
    fn partial_cmp(&self, other: &Element<V>) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<V> Ord for Element<V> {
    fn cmp(&self, other: &Element<V>) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<K: Key, V: Value> Map<K, V> {

    crdt_impl!(Map, MapValue<K,V>);

    /// Constructs and returns a new map.
    /// The map has site 1 and counter 0.
    pub fn new() -> Self {
        let replica = Replica::new(1, 0);
        let value = MapValue::new();
        Map{replica, value, awaiting_site: vec![]}
    }

    /// Returns true iff the map has the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.value.elements.contains_key(key)
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.value.elements.get(key).and_then(|elements| Some(&elements[0].1))
    }

    /// Inserts a key-value pair into the map and returns a remote
    /// op that can be sent to remote sites for replication. If the
    /// map does not have a site allocated, it caches the op and
    /// returns an `AwaitingSite` error.
    pub fn insert(&mut self, key: K, value: V) -> Result<RemoteOp<K, V>, Error> {
        let op = self.value.insert(key, value, &self.replica)?;
        self.after_op(op)
    }

    /// Removes a key from the map and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the map does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, key: &K) -> Result<RemoteOp<K,V>, Error> {
        let op = self.value.remove(key)?;
        self.after_op(op)
    }

    /// Merges two Maps into one.
    pub fn merge(&mut self, other: Map<K,V>) {
        self.value.merge(other.value)
    }
}

impl<K: Key, V: Value> MapValue<K, V> {

    /// Constructs and returns a new map value.
    pub fn new() -> Self {
        MapValue{elements: HashMap::new(), tombstones: Tombstones::new()}
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns an iterator over the key-value pairs in the map.
    pub fn iter(&self) -> hash_map::Iter<K,Vec<Element<V>>> {
        self.elements.iter()
    }

    /// Returns a mutable reference to the first element for the key.
    /// For internal use only.
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut Element<V>>
        where Q: Hash + Eq,
              K: Borrow<Q>,
    {
        let elements = try_opt!(self.elements.get_mut(key));
        Some(&mut elements[0])
    }

    /// Returns a mutable reference to the element corresponding to the
    /// given key and replica. For internal use only.
    pub fn get_mut_element<Q: ?Sized>(&mut self, key: &Q, replica: &Replica) -> Option<&mut Element<V>>
        where Q: Hash + Eq,
              K: Borrow<Q>,
    {
        let elements = try_opt!(self.elements.get_mut(key));
        let index = try_opt!(elements.binary_search_by(|e| e.0.cmp(replica)).ok());
        Some(&mut elements[index])
    }

    /// Inserts a key-value pair into the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map had this key present, the value is updated. If the
    /// value was already identical, it returns an AlreadyExists
    /// error.
    pub fn insert(&mut self, key: K, value: V, replica: &Replica) -> Result<RemoteOp<K, V>, Error> {
        if let Some(values) = self.elements.get(&key) {
            if values[0].1 == value { return Err(Error::AlreadyExists) }
        }

        let element = Element(replica.clone(), value.clone());
        let old_elements = self.elements.entry(key.clone()).or_insert(vec![]);
        let new_elements = vec![element.clone()];
        let removed_elements = mem::replace(old_elements, new_elements);
        let removed = removed_elements.into_iter().map(|e| e.0).collect();
        Ok(RemoteOp::Insert{key, element, removed})
    }

    /// Removes a key-value pair from the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map did not contain the key, it returns a DoesNotExist error.
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Result<RemoteOp<K, V>, Error>
        where Q: Hash + Eq + ToOwned<Owned = K>,
              K: Borrow<Q>,
    {
        let removed_elements = self.elements.remove(key).ok_or(Error::DoesNotExist)?;
        let removed = removed_elements.into_iter().map(|e| e.0).collect();

        for replica in &removed {
            self.tombstones.insert(replica);
        }

        Ok(RemoteOp::Remove{key: key.to_owned(), removed})
    }

    /// Updates the map and returns the equivalent local op.
    pub fn execute_remote(&mut self, op: &RemoteOp<K, V>) -> Option<LocalOp<K, V>> {
        match *op {
            RemoteOp::Insert{ref key, ref element, ref removed} => {
                let elements = self.elements.entry(key.clone()).or_insert(vec![]);
                remove_replicas(elements, removed);

                for replica in removed {
                    self.tombstones.insert(replica);
                }

                let index = try_opt!(elements.binary_search_by(|e| e.0.cmp(&element.0)).err());
                elements.insert(index, element.clone());
                if index == 0 {
                    Some(LocalOp::Insert{key: key.clone(), value: element.1.clone()})
                } else {
                    None
                }
            }
            RemoteOp::Remove{ref key, ref removed} => {
                let first_remaining_element = {
                    let elements = try_opt!(self.elements.get_mut(key));
                    remove_replicas(elements, removed);
                    elements.first().and_then(|e| Some(e.1.clone()))
                };

                for replica in removed {
                    self.tombstones.insert(replica);
                }

                if let Some(value) = first_remaining_element {
                    Some(LocalOp::Insert{key: key.clone(), value: value})
                } else {
                    let _ = self.remove(key);
                    Some(LocalOp::Remove{key: key.clone()})
                }
            }
        }
    }

    /// Merges two MapValues into one.
    pub fn merge(&mut self, mut other: MapValue<K,V>) {
        let self_elements = mem::replace(&mut self.elements, HashMap::new());

        for (key, elements) in self_elements {
            let elements =
                if let Some(other_elements) = other.elements.remove(&key) {
                    let mut self_elements: Vec<Element<V>> =
                        elements.into_iter()
                        .filter(|e| other_elements.contains(&e) || !other.tombstones.contains(&e.0))
                        .collect();

                    let mut other_elements =
                        other_elements.into_iter()
                        .filter(|e| !self_elements.contains(&e) && !self.tombstones.contains(&e.0))
                        .collect();

                    self_elements.append(&mut other_elements);
                    self_elements.sort();
                    self_elements
                } else {
                    elements.into_iter().filter(|e| !other.tombstones.contains(&e.0)).collect()
                };

            if !elements.is_empty() {
                self.elements.insert(key, elements);
            }
        }

        for (key, elements) in other.elements.into_iter() {
            let elements: Vec<Element<V>> = elements.into_iter()
                .filter(|e| !self.tombstones.contains(&e.0)).collect();

            if !elements.is_empty() {
                self.elements.insert(key, elements);
            }
        }

        self.tombstones.merge(other.tombstones);
    }
}

fn remove_replicas<V: Value>(elements: &mut Vec<Element<V>>, replicas: &[Replica]) {
    for replica in replicas {
        if let Ok(index) = elements.binary_search_by(|e| e.0.cmp(&replica)) {
            elements.remove(index);
        }
    }
}

impl<K: Key, V: Value> CrdtValue for MapValue<K, V> {
    type LocalValue = HashMap<K, V>;
    type RemoteOp = RemoteOp<K, V>;
    type LocalOp = LocalOp<K, V>;

    fn local_value(&self) -> HashMap<K, V> {
        let mut hash_map = HashMap::new();
        for (key, elements) in self.elements.iter() {
            hash_map.insert(key.clone(), elements[0].1.clone());
        }
        hash_map
    }

    fn add_site(&mut self, op: &RemoteOp<K,V>, site: u32) {
        if let RemoteOp::Insert{ref key, ref element, ..} = *op {
            let elements = some!(self.elements.get_mut(key));
            let index = some!(elements.binary_search_by(|e| e.0.cmp(&element.0)).ok());
            elements[index].0.site = site;
        }
    }
}

impl<K: Key, V: Value + AddSiteToAll> AddSiteToAll for MapValue<K,V> {
    fn add_site_to_all(&mut self, site: u32) {
        for elements in self.elements.values_mut() {
            for element in elements.iter_mut() {
                element.0.site = site;
                element.1.add_site_to_all(site);
            }
        }
    }

    fn validate_site_for_all(&self, site: u32) -> Result<(), Error> {
        for elements in self.elements.values() {
            for element in elements.iter() {
                try_assert!(element.0.site == site, Error::InvalidRemoteOp);
                try!(element.1.validate_site_for_all(site));
            }
        }
        Ok(())
    }
}

impl<K: Key, V: Value> CrdtRemoteOp for RemoteOp<K, V> {
    fn add_site(&mut self, site: u32) {
        match *self {
            RemoteOp::Insert{ref mut element, ref mut removed, ..} => {
                element.0.site = site;
                for replica in removed {
                    if replica.site == 0 { replica.site = site; }
                }
            }
            RemoteOp::Remove{ref mut removed, ..} => {
                for replica in removed {
                    if replica.site == 0 { replica.site = site; }
                }
            }
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        match *self {
            RemoteOp::Remove{..} => Ok(()),
            RemoteOp::Insert{ref element, ..} => {
                try_assert!(element.0.site == site, Error::InvalidRemoteOp);
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
        let map: Map<bool, i64> = Map::new();
        assert!(map.site() == 1);
    }

    #[test]
    fn test_contains_key() {
        let mut map: Map<usize, isize> = Map::new();
        let _ = map.insert(123, -123);
        assert!(map.contains_key(&123));
    }

    #[test]
    fn test_insert() {
        let mut map: Map<u32, String> = Map::new();
        let remote_op = map.insert(123, "abc".to_owned()).unwrap();
        let (key, element, removed) = insert_fields(remote_op);
        assert!(map.get(&123).unwrap() == "abc");
        assert!(key == 123);
        assert!(element.0 == Replica::new(1,0));
        assert!(element.1 == "abc");
        assert!(removed.is_empty());
    }

    #[test]
    fn test_insert_replaces_value() {
        let mut map: Map<u32, String> = Map::new();
        let _ = map.insert(123, "abc".to_owned()).unwrap();
        let remote_op2 = map.insert(123, "def".to_owned()).unwrap();
        let (key, element, removed) = insert_fields(remote_op2);

        assert!(map.get(&123).unwrap() == "def");
        assert!(key == 123);
        assert!(element.0 == Replica::new(1,1));
        assert!(element.1 == "def");
        assert!(removed[0] == Replica::new(1,0));
    }

    #[test]
    fn test_insert_same_value() {
        let mut map: Map<u32, String> = Map::new();
        let _ = map.insert(123, "abc".to_owned()).unwrap();
        assert!(map.insert(123, "abc".to_owned()).unwrap_err() == Error::AlreadyExists);
    }

    #[test]
    fn test_insert_awaiting_site() {
        let mut map: Map<u32, String> = Map::from_value(MapValue::new(), 0);
        assert!(map.insert(123, "abc".to_owned()).unwrap_err() == Error::AwaitingSite);
        assert!(map.get(&123).unwrap() == "abc");
        assert!(map.awaiting_site.len() == 1);
    }

    #[test]
    fn test_remove() {
        let mut map: Map<bool, i8> = Map::new();
        let _ = map.insert(true, 3).unwrap();
        let remote_op = map.remove(&true).unwrap();
        let (key, removed) = remove_fields(remote_op);

        assert!(map.get(&true).is_none());
        assert!(key == true);
        assert!(removed[0] == Replica::new(1,0));
    }

    #[test]
    fn test_remove_does_not_exist() {
        let mut map: Map<bool, i8> = Map::new();
        assert!(map.remove(&true).unwrap_err() == Error::DoesNotExist);
    }

    #[test]
    fn test_remove_awaiting_site() {
        let mut map: Map<bool, i8> = Map::from_value(MapValue::new(), 0);
        let _ = map.insert(true, 3);
        assert!(map.remove(&true).unwrap_err() == Error::AwaitingSite);
        assert!(map.get(&true).is_none());
    }

    #[test]
    fn test_execute_remote_insert() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let remote_op = map1.insert(123, 1010).unwrap();
        let local_op  = map2.execute_remote(&remote_op).unwrap();

        assert!(map2.get(&123).unwrap() == &1010);
        assert_matches!(local_op, LocalOp::Insert{key: 123, value: 1010});
    }

    #[test]
    fn test_execute_remote_insert_concurrent() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let remote_op1 = map1.insert(123, 2222).unwrap();
        let remote_op2 = map2.insert(123, 1111).unwrap();
        let local_op1  = map1.execute_remote(&remote_op2);
        let local_op2  = map2.execute_remote(&remote_op1);

        assert!(map1.get(&123).unwrap() == &2222);
        assert!(map2.get(&123).unwrap() == &2222);
        assert_matches!(local_op1, None);
        assert_matches!(local_op2, Some(LocalOp::Insert{key: 123, value: 2222}));
    }

    #[test]
    fn test_execute_remote_insert_dupe() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let remote_op = map1.insert(123, 2222).unwrap();
        let _ = map2.execute_remote(&remote_op);
        assert!(map2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_execute_remote_remove() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let remote_op1 = map1.insert(123, 2222).unwrap();
        let remote_op2 = map1.remove(&123).unwrap();
        let _ = map2.execute_remote(&remote_op1).unwrap();
        let local_op = map2.execute_remote(&remote_op2).unwrap();

        assert!(map2.get(&123).is_none());
        assert_matches!(local_op, LocalOp::Remove{key: 123});
    }

    #[test]
    fn test_execute_remote_remove_does_not_exist() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let _ = map1.insert(123, 2222);
        let remote_op = map1.remove(&123).unwrap();
        assert!(map2.execute_remote(&remote_op).is_none());
    }

    #[test]
    fn test_execute_remote_remove_some_replicas_remain() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let mut map3: Map<i32, u64> = Map::from_value(MapValue::new(), 3);
        let remote_op1 = map2.insert(123, 1111).unwrap();
        let remote_op2 = map1.insert(123, 2222).unwrap();
        let remote_op3 = map1.remove(&123).unwrap();

        let _ = map3.execute_remote(&remote_op1).unwrap();
        let _ = map3.execute_remote(&remote_op2).unwrap();
        let local_op3 = map3.execute_remote(&remote_op3).unwrap();

        assert!(map3.get(&123).unwrap() == &1111);
        assert_matches!(local_op3, LocalOp::Insert{key: 123, value: 1111});
    }

    #[test]
    fn test_execute_remote_remove_dupe() {
        let mut map1: Map<i32, u64> = Map::new();
        let mut map2: Map<i32, u64> = Map::from_value(MapValue::new(), 2);
        let remote_op1 = map1.insert(123, 2222).unwrap();
        let remote_op2 = map1.remove(&123).unwrap();

        let _ = map2.execute_remote(&remote_op1).unwrap();
        let _ = map2.execute_remote(&remote_op2).unwrap();
        assert!(map2.execute_remote(&remote_op2).is_none());
    }

    #[test]
    fn test_merge() {
        let mut map1: Map<u32, bool> = Map::new();
        let _ = map1.insert(1, true);
        let _ = map1.insert(2, true);
        let _ = map1.remove(&2);
        let _ = map1.insert(3, true);

        let mut map2 = Map::from_value(map1.clone_value(), 2);
        let _ = map2.remove(&3);
        let _ = map2.insert(4, true);
        let _ = map2.remove(&4);
        let _ = map2.insert(5, true);
        let _ = map1.insert(4, true);
        let _ = map1.insert(5, true);
        let map3 = map1.clone();

        map1.merge(map2.clone());
        map2.merge(map3.clone());

        assert!(map1.value == map2.value);
        assert!(map1.contains_key(&1));
        assert!(!map1.contains_key(&2));
        assert!(!map1.contains_key(&3));
        assert!(map1.contains_key(&4));
        assert!(map1.contains_key(&5));

        assert!(map1.value.elements[&1][0].0 == Replica{site: 1, counter: 0});
        assert!(map1.value.elements[&4][0].0 == Replica{site: 1, counter: 4});
        assert!(map1.value.elements[&5][0].0 == Replica{site: 1, counter: 5});
        assert!(map1.value.elements[&5][1].0 == Replica{site: 2, counter: 3});

        assert!(map1.value.tombstones.contains_pair(1, 3));
        assert!(map1.value.tombstones.contains_pair(2, 1));
    }

    #[test]
    fn test_add_site() {
        let mut map: Map<i32, u64> = Map::from_value(MapValue::new(), 0);
        let _ = map.insert(10, 56);
        let _ = map.insert(20, 57);
        let _ = map.remove(&10);
        let mut remote_ops = map.add_site(5).unwrap().into_iter();

        let remote_op1 = remote_ops.next().unwrap();
        let remote_op2 = remote_ops.next().unwrap();
        let remote_op3 = remote_ops.next().unwrap();
        let (key1, elt1, removed1) = insert_fields(remote_op1);
        let (key2, elt2, removed2) = insert_fields(remote_op2);
        let (key3, removed3) = remove_fields(remote_op3);

        assert!(key1 == 10 && elt1.0 == Replica::new(5,0) && elt1.1 == 56 && removed1.is_empty());
        assert!(key2 == 20 && elt2.0 == Replica::new(5,1) && elt2.1 == 57 && removed2.is_empty());
        assert!(key3 == 10 && removed3.len() == 1 && removed3[0] == Replica::new(5,0));
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut map: Map<i32, u64> = Map::from_value(MapValue::new(), 123);
        let _ = map.insert(10, 56).unwrap();
        let _ = map.insert(20, 57).unwrap();
        let _ = map.remove(&10).unwrap();
        assert!(map.add_site(3).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let mut map1: Map<i32, i64> = Map::new();
        let _ = map1.insert(1, 100);
        let _ = map1.insert(2, 200);

        let s_json = serde_json::to_string(&map1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&map1).unwrap();
        let map2: Map<i32, i64> = serde_json::from_str(&s_json).unwrap();
        let map3: Map<i32, i64> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(map1 == map2);
        assert!(map1 == map3);
    }

    #[test]
    fn test_serialize_value() {
        let mut map1: Map<i32, i64> = Map::new();
        let _ = map1.insert(1, 100);
        let _ = map1.insert(2, 200);

        let s_json = serde_json::to_string(&map1.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(&map1.value()).unwrap();
        let value2: MapValue<i32, i64> = serde_json::from_str(&s_json).unwrap();
        let value3: MapValue<i32, i64> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*map1.value() == value2);
        assert!(*map1.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut map: Map<bool, String> = Map::new();
        let remote_op1 = map.insert(true, "abc".to_owned()).unwrap();

        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp<bool, String> = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp<bool, String> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let local_op1: LocalOp<String, (i32, i32)> = LocalOp::Insert{key: "abc".to_owned(), value: (32, 102)};

        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp<String, (i32, i32)> = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp<String, (i32, i32)> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }

    fn insert_fields<K, V>(remote_op: RemoteOp<K, V>) -> (K, Element<V>, Vec<Replica>) {
        match remote_op {
            RemoteOp::Insert{key, element, removed} => (key, element, removed),
            _ => panic!(),
        }
    }

    fn remove_fields<K, V>(remote_op: RemoteOp<K, V>) -> (K, Vec<Replica>) {
        match remote_op {
            RemoteOp::Remove{key, removed} => (key, removed),
            _ => panic!(),
        }
    }
}
