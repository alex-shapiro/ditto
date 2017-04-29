use Error;
use Replica;

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;

#[derive(Debug, Clone)]
pub struct Map<K: Debug + Clone + Eq + Hash, V: Clone>(HashMap<K, Vec<Element<V>>>);

// #[derive(Debug, Clone)]
type Element<V> = (Replica, V);

#[derive(Debug, Clone)]
pub struct RemoteOp<K, V: Clone> {
    key:    K,
    remove: Vec<Element<V>>,
    insert: Option<Element<V>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalOp<K, V: Clone> {
    Insert(K, V),
    Remove(K),
}

impl<K,V> Map<K, V> where K: Debug + Clone + Eq + Hash, V: Clone {

    /// Constructs and returns a new map.
    pub fn new() -> Self {
        Map(HashMap::new())
    }

    /// Consumes the map and returns a HashMap of its values.
    pub fn into(self) -> HashMap<K,V> {
        let mut hashmap = HashMap::new();
        for (key, mut elements) in self.0.into_iter() {
            let element = elements.swap_remove(0);
            hashmap.insert(key, element.1);
        }
        hashmap
    }

    /// Returns true if the map has the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key).and_then(|elements| Some(&elements[0].1))
    }

    /// Returns a mutable reference to the value corresponding to
    /// the key. This function must not be exposed to the end-user
    /// because mutating a value without creating an op will cause
    /// the CRDT to lose sync with remote replicas.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.0.get_mut(key).and_then(|elements| Some(&mut elements[0].1))
    }

    /// Inserts a key-value pair into the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map had this key present, the value is updated.
    pub fn insert(&mut self, key: K, value: V, replica: &Replica) -> RemoteOp<K, V> {
        let old_elements = self.0.entry(key.clone()).or_insert(vec![]);
        let new_elements = vec![(replica.clone(), value.clone())];
        let remove = mem::replace(old_elements, new_elements);
        let insert = Some((replica.clone(), value));
        RemoteOp{key, remove, insert}
    }

    /// Removes a key-value pair from the map and returns an op
    /// that can be sent to remote sites for replication. If the
    /// map did not contain the key, it returns a DoesNotExist error.
    pub fn remove(&mut self, key: &K) -> Result<RemoteOp<K, V>, Error> {
        let remove = self.0.remove(key).ok_or(Error::DoesNotExist)?;
        Ok(RemoteOp{key: key.clone(), remove, insert: None})
    }

    /// Updates the map and returns the equivalent local op.
    pub fn execute_remote(&mut self, op: &RemoteOp<K, V>) -> LocalOp<K, V> {
        let key_should_be_removed = {
            let elements = self.0.entry(op.key.clone()).or_insert(vec![]);

            for element in &op.remove {
                if let Ok(index) = elements.binary_search_by(|e| e.0.cmp(&element.0)) {
                    elements.remove(index);
                }
            }

            if let Some(ref element) = op.insert {
                if let Err(index) = elements.binary_search_by(|e| e.0.cmp(&element.0)) {
                    elements.insert(index, element.clone());
                }
            }

            elements.is_empty()
        };

        if key_should_be_removed {
            self.0.remove(&op.key);
            LocalOp::Remove(op.key.clone())
        } else {
            let key = op.key.clone();
            let value = self.get(&key).unwrap().clone();
            LocalOp::Insert(key, value)
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
