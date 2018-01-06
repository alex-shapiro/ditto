//! A CRDT that stores a collection of key-value pairs.

use Error;
use replica::{Replica, Summary, SiteId};
use map_tuple_vec;
use traits2::*;

use serde::ser::Serialize;
use serde::de::DeserializeOwned;
use std::borrow::{Borrow, Cow};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

pub trait Key: Clone + Eq + Hash + Serialize + DeserializeOwned {}
impl<T: Clone + Eq + Hash + Serialize + DeserializeOwned> Key for T {}

pub trait Value: Clone + PartialEq + Serialize + DeserializeOwned {}
impl<T: Clone + PartialEq + Serialize + DeserializeOwned> Value for T {}

/// A Map is a `HashMap`-like collection of key-value pairs.
/// As with `HashMap`, `Map` requires that the elements implement
/// the `Eq` and `Hash` traits. To allow for CRDT replication, they
/// must also implement the `Clone`, `Serialize`, and `Deserialize`
/// traits.
///
/// Internally, Map is based on OR-Set. It allows op-based replication
/// via [`execute_op`](#method.execute_op) and state-based replication
/// via [`merge`](#method.merge). State-based replication allows
/// out-of-order delivery but op-based replication does not.
///
/// Map's performance characteristics are similar to HashMap:
///
///   * [`insert`](#method.insert): *O(1)*
///   * [`remove`](#method.remove): *O(1)*
///   * [`contains_key`](#method.contains_key): *O(1)*
///   * [`get`](#method.get): *O(1)*
///   * [`execute_op`](#method.execute_op): *O(1)*
///   * [`merge`](#method.merge): *O(N1 + N2 + S1 + S2)*, where *N1* and
///     *N2* are the number of values in the maps being merged,
///     and *S1* and *S2* are the number of sites that have edited maps
///     being merged.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Map<K: Key, V: Value> {
    inner:      Inner<K, V>,
    summary:    Summary,
    site_id:    SiteId,
    cached_ops: Vec<Op<K, V>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct MapState<'a, K: Key + 'a, V: Value + 'a> {
    inner: Cow<'a, Inner<K,V>>,
    summary: Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Inner<K: Key, V: Value>(#[serde(with = "map_tuple_vec")] pub HashMap<K, Vec<Element<V>>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op<K, V> {
    key: K,
    inserted_element: Option<Element<V>>,
    removed_replicas: Vec<Replica>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<K, V> {
    Insert{key: K, value: V},
    Remove{key: K},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[doc(hidden)]
pub struct Element<V> {
    pub value: V,
    pub replica: Replica,
}

impl<V> PartialEq for Element<V> {
    fn eq(&self, other: &Element<V>) -> bool {
        self.replica == other.replica
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
        self.replica.cmp(&other.replica)
    }
}

impl<K: Key, V: Value> Map<K, V> {

    /// Constructs and returns a new map.
    /// The map has site 1 and counter 0.
    pub fn new() -> Self {
        let inner   = Inner::new();
        let summary = Summary::new();
        let site_id = 1;
        Map{inner, summary, site_id, cached_ops: vec![]}
    }

    /// Returns true iff the map has the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.0.contains_key(key)
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        let elements = self.inner.0.get(key)?;
        Some(&elements[0].value)
    }

    /// Inserts a key-value pair into the map and returns a remote
    /// op that can be sent to remote sites for replication. If the
    /// map does not have a site allocated, it caches the op and
    /// returns an `AwaitingSite` error.
    pub fn insert(&mut self, key: K, value: V) -> Result<Op<K, V>, Error> {
        let counter = self.summary.increment(self.site_id);
        let op = self.inner.insert(key, value, Replica::new(self.site_id, counter));
        self.after_op(op)
    }

    /// Removes a key from the map and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the map does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, key: &K) -> Option<Result<Op<K,V>, Error>> {
        let op = self.inner.remove(key)?;
        Some(self.after_op(op))
    }

    crdt_impl2! {
        Map,
        MapState<K, V>,
        MapState<'static, K, V>,
        MapState,
        Inner<K, V>,
        Op<K, V>,
        LocalOp<K, V>,
        HashMap<K, V>,
    }
}

impl<K: Key, V: Value> From<HashMap<K, V>> for Map<K, V> {
    fn from(local_value: HashMap<K, V>) -> Self {
        let mut map = Map::new();
        for (k, v) in local_value { let _ = map.insert(k, v); }
        map
    }
}

impl<K: Key, V: Value> Inner<K, V> {

    pub fn new() -> Self {
        Inner(HashMap::new())
    }

    // pub fn len(&self) -> usize {
    //     self.0.len()
    // }

    // pub fn iter(&self) -> hash_map::Iter<K,Vec<Element<V>>> {
    //     self.0.iter()
    // }

    // pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut Element<V>>
    //     where Q: Hash + Eq,
    //           K: Borrow<Q>,
    // {
    //     let elements = self.0.get_mut(key)?;
    //     Some(&mut elements[0])
    // }

    // pub fn get_mut_element<Q: ?Sized>(&mut self, key: &Q, replica: &Replica) -> Option<&mut Element<V>>
    //     where Q: Hash + Eq,
    //           K: Borrow<Q>,
    // {
    //     let elements = self.0.get_mut(key)?;
    //     let idx = elements.binary_search_by(|e| e.replica.cmp(replica)).ok()?;
    //     Some(&mut elements[idx])
    // }

    pub fn insert(&mut self, key: K, value: V, replica: Replica) -> Op<K, V> {
        let inserted_element = Element{value, replica};
        let removed_elements = self.0.insert(key.clone(), vec![inserted_element.clone()]).unwrap_or(vec![]);
        let removed_replicas = removed_elements.into_iter().map(|e| e.replica).collect();
        Op{key, inserted_element: Some(inserted_element), removed_replicas}
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<Op<K, V>>
        where Q: Hash + Eq + ToOwned<Owned = K>,
              K: Borrow<Q>,
    {
        let removed_elements = self.0.remove(key)?;
        let removed_replicas = removed_elements.into_iter().map(|e| e.replica).collect();
        Some(Op{key: key.to_owned(), inserted_element: None, removed_replicas})
    }

    pub fn execute_op(&mut self, op: Op<K, V>) -> LocalOp<K, V> {
        let mut elements = self.0.remove(&op.key).unwrap_or(vec![]);
        elements.retain(|e| !op.removed_replicas.contains(&e.replica));

        if let Some(new_element) = op.inserted_element {
            if let Err(idx) = elements.binary_search_by(|e| e.cmp(&new_element)) {
                elements.insert(idx, new_element);
            }
        }

        if elements.is_empty() {
            LocalOp::Remove{key: op.key}
        } else {
            let value = elements[0].value.clone();
            self.0.insert(op.key.clone(), elements);
            LocalOp::Insert{key: op.key, value}
        }
    }

    pub fn merge(&mut self, other: Self, summary: &Summary, other_summary: &Summary) {
        let mut other_values = other.0;

        // retain an element in self iff
        // - the element is in both self and other, OR
        // - the element has not been inserted into other
        self.0.retain(|key, elements| {
            let mut other_elements = other_values.remove(&key).unwrap_or(vec![]);
            elements.retain(|e| other_elements.contains(e) || !other_summary.contains(&e.replica));
            other_elements.retain(|e| !elements.contains(e) && !summary.contains(&e.replica));
            elements.append(&mut other_elements);
            elements.sort();
            !elements.is_empty()
        });

        // insert any element that is in other but not yet inserted into self
        for (key, mut elements) in other_values {
            elements.retain(|e| !summary.contains(&e.replica));
            if !elements.is_empty() {
                self.0.insert(key, elements);
            }
        }
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        for (_, elements) in &mut self.0 {
            for element in elements {
                if element.replica.site == 0 { element.replica.site = site_id };
            }
        }
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        for elements in self.0.values() {
            for element in elements {
                if element.replica.site == 0 {
                    return Err(Error::InvalidSiteId);
                }
            }
        }
        Ok(())
    }

    pub fn local_value(&self) -> HashMap<K, V> {
        let mut hashmap = HashMap::with_capacity(self.0.len());
        for (key, elements) in &self.0 {
            hashmap.insert(key.clone(), elements[0].value.clone());
        }
        hashmap
    }
}

impl<K: Key, V: Value + NestedInner> NestedInner for Inner<K, V> {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        for (_, elements) in &mut self.0 {
            for element in elements {
                element.value.nested_add_site_id(site_id);
                if element.replica.site == 0 {
                    element.replica.site = site_id;
                }
            }
        }
    }

    fn nested_validate_no_unassigned_sites(&self) -> Result<(), Error> {
        for elements in self.0.values() {
            for element in elements {
                if element.replica.site == 0 {
                    return Err(Error::InvalidSiteId);
                }
                element.value.nested_validate_no_unassigned_sites()?;
            }
        }
        Ok(())
    }

    fn nested_validate_all(&self, site_id: SiteId) -> Result<(), Error> {
        for elements in self.0.values() {
            for element in elements {
                if element.replica.site == 0 {
                    return Err(Error::InvalidSiteId);
                }
                element.value.nested_validate_all(site_id)?;
            }
        }
        Ok(())
    }


    fn nested_merge(&mut self, other: Inner<K, V>, summary: &Summary, other_summary: &Summary) {
        let mut other_values = other.0;

        self.0.retain(|key, elements| {
            let mut other_elements = other_values.remove(&key).unwrap_or(vec![]);

            // remove elements that have been removed from other
            elements.retain(|e| other_elements.contains(e) || !other_summary.contains(&e.replica));
            other_elements.retain(|e| elements.contains(e) || !summary.contains(&e.replica));

            let (other_merge, mut other_insert) = other_elements.into_iter()
                .partition(|e| elements.contains(e));

            // merge elements that are in both self and other
            for element in other_merge {
                let idx = elements.binary_search_by(|e| e.cmp(&element)).expect("Element must be present");
                elements[idx].value.nested_merge(element.value, summary, other_summary);
            }

            // append elements from other that have not been inserted into self
            elements.append(&mut other_insert);
            elements.sort();
            !elements.is_empty()
        });

        // insert any element that is in other but not yet inserted into self
        for (key, mut elements) in other_values {
            elements.retain(|e| !summary.contains(&e.replica));
            if !elements.is_empty() {
                self.0.insert(key, elements);
            }
        }
    }
}

impl<K: Key, V: Value> Op<K, V> {
    /// Returns the `Op`'s key.
    pub fn key(&self) -> &K { &self.key }

    /// Returns a reference to the `Op`'s inserted element.
    pub fn inserted_element(&self) -> Option<&Element<V>> { self.inserted_element.as_ref() }

    /// Returns a reference to the `Op`'s removed replicas.
    pub fn removed_replicas(&self) -> &[Replica] { &self.removed_replicas }

    /// Assigns a site id to any unassigned inserts and removes
    pub fn add_site_id(&mut self, site_id: SiteId) {
        if let Some(ref mut e) = self.inserted_element {
            if e.replica.site == 0 { e.replica.site = site_id };
        }
        for r in &mut self.removed_replicas {
            if r.site == 0 { r.site = site_id };
        }
    }

    /// Validates that the `Op`'s site id is equal to the given site id.
    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if let Some(ref e) = self.inserted_element {
            if e.replica.site != site_id { return Err(Error::InvalidOp) };
        }
        Ok(())
    }

    pub(crate) fn inserted_replicas(&self) -> Vec<Replica> {
        match self.inserted_element {
            Some(ref e) => vec![e.replica.clone()],
            None => vec![],
        }
    }
}

impl<K: Key, V: Value + NestedInner> NestedOp for Op<K, V> {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        if let Some(ref mut e) = self.inserted_element {
            e.value.nested_add_site_id(site_id);
            if e.replica.site == 0 { e.replica.site = site_id };
        }
        for r in &mut self.removed_replicas {
            if r.site == 0 { r.site = site_id };
        }
    }

    fn nested_validate(&self, site_id: SiteId) -> Result<(), Error> {
        if let Some(ref e) = self.inserted_element {
            if e.replica.site != site_id { return Err(Error::InvalidOp) };
            e.value.nested_validate_all(site_id)?;
        }
        Ok(())
    }
}
