//! A CRDT that stores a collection of distinct elements.

use Error;
use replica::{Replica, SiteId, Counter, Summary};
use map_tuple_vec;

use serde::ser::Serialize;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::mem;

pub trait SetElement: Clone + Eq + Hash + Serialize + DeserializeOwned {}
impl<T: Clone + Eq + Hash + Serialize + DeserializeOwned> SetElement for T {}

/// A Set is a `HashSet`-like collection of distinct elements.
/// As with `HashSet`, `Set` requires that the elements implement
/// the `Eq` and `Hash` traits. To allow for CRDT replication, they
/// must also implement the `Clone`, `Serialize`, and `Deserialize`
/// traits.
///
/// Internally, Set is a variant of OR-Set. It allows op-based replication
/// via [`execute_op`](#method.execute_op) and state-based replication
/// via [`merge`](#method.merge). Both replication methods are idempotent
/// and can handle out-of-order delivery.
///
/// `Set` has a spatial complexity of *O(N + S)*, where
/// *N* is the number of values concurrently held in the `Set` and
/// *S* is the number of sites that have inserted values into the `Set`.
/// It has the following performance characteristics:
///
///   * [`insert`](#method.insert): *O(1)*
///   * [`remove`](#method.remove): *O(1)*
///   * [`contains`](#method.contains): *O(1)*
///   * [`execute_op`](#method.execute_op): *O(1)*
///   * [`merge`](#method.merge): *O(N1 + N2 + S1 + S2)*, where *N1* and
///     *N2* are the number of values in the sets being merged,
///     and *S1* and *S2* are the number of sites that have edited sets
///     being merged.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct Set<T: SetElement> {
    inner:      Inner<T>,
    summary:    Summary,
    site_id:    SiteId,
    cached_ops: Vec<Op<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct SetState<'a, T: SetElement + 'a>{
    inner: Cow<'a, Inner<T>>,
    summary: Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Inner<T: SetElement>(#[serde(with = "map_tuple_vec")] pub HashMap<T, Vec<Replica>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op<T> {
    value: T,
    inserted_replica: Option<Replica>,
    removed_replicas: Vec<Replica>,
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
        let inner   = Inner::new();
        let summary = Summary::new();
        let site_id = 1;
        Set{inner, summary, site_id, cached_ops: vec![]}
    }

    /// Returns true iff the set contains the value.
    pub fn contains(&self, value: &T) -> bool {
        self.inner.contains(value)
    }

    /// Inserts a value into the set and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the set does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn insert(&mut self, value: T) -> Result<Op<T>, Error> {
        let counter = self.summary.increment(self.site_id);
        let op = self.inner.insert(value, self.site_id, counter);
        self.after_op(op)
    }

    /// Removes a value from the set and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the set does not have a site allocated, it caches
    /// the op and returns an `AwaitingSite` error.
    pub fn remove(&mut self, value: &T) -> Option<Result<Op<T>, Error>> {
        let op = self.inner.remove(value)?;
        Some(self.after_op(op))
    }

    crdt_impl2! {
        Set,
        SetState<T>,
        SetState<'static, T>,
        SetState,
        Inner<T>,
        Op<T>,
        Option<LocalOp<T>>,
        HashSet<T>,
    }
}

impl<T: SetElement> Inner<T> {

    fn new() -> Self {
        Inner(HashMap::new())
    }

    fn contains(&self, value: &T) -> bool {
        self.0.contains_key(value)
    }

    fn insert(&mut self, value: T, site_id: SiteId, counter: Counter) -> Op<T> {
        let inserted_replica = Replica{site: site_id, counter};
        let removed_replicas = self.0.insert(value.clone(), vec![inserted_replica.clone()]).unwrap_or(vec![]);
        Op{value, inserted_replica: Some(inserted_replica), removed_replicas}
    }

    fn remove(&mut self, value: &T) -> Option<Op<T>> {
        let removed_replicas = self.0.remove(value)?;
        Some(Op{value: value.clone(), inserted_replica: None, removed_replicas})
    }

    fn execute_op(&mut self, op: Op<T>) -> Option<LocalOp<T>> {
        let mut replicas  = self.0.remove(&op.value).unwrap_or(vec![]);
        let exists_before = !replicas.is_empty();
        replicas.retain(|r| !op.removed_replicas.contains(r));

        if let Some(new_replica) = op.inserted_replica {
            if let Err(idx) = replicas.binary_search_by(|r| r.cmp(&new_replica)) {
                replicas.insert(idx, new_replica);
            }
        }

        let exists_after = !replicas.is_empty();
        if exists_before && exists_after {
            self.0.insert(op.value, replicas);
            None
        } else if exists_after {
            self.0.insert(op.value.clone(), replicas);
            Some(LocalOp::Insert(op.value))
        } else if exists_before {
            Some(LocalOp::Remove(op.value))
        } else {
            None
        }
    }

    fn merge(&mut self,  other: Inner<T>, summary: &Summary, other_summary: &Summary) {
        let mut other_elements = other.0;

        // retain an element in self iff:
        // - the element is in in both self and other, OR
        // - the element has not been inserted into other
        self.0.retain(|value, replicas| {
            let mut other_replicas = other_elements.remove(&value).unwrap_or(vec![]);
            replicas.retain(|r| other_replicas.contains(r) || !other_summary.contains(r));
            other_replicas.retain(|r| !replicas.contains(r) && !summary.contains(r));
            replicas.append(&mut other_replicas);
            replicas.sort();
            !replicas.is_empty()
        });

        // insert any element that is in other but not yet inserted into self
        for (value, mut replicas) in other_elements.to_owned() {
            replicas.retain(|r| !summary.contains(r));
            if !replicas.is_empty() {
                self.0.insert(value, replicas);
            }
        }
    }

    fn add_site_id(&mut self, site_id: SiteId) {
        for (_, replicas) in &mut self.0 {
            for replica in replicas {
                if replica.site == 0 { replica.site = site_id };
            }
        }
    }

    fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        for replicas in self.0.values() {
            for replica in replicas {
                if replica.site == 0 {
                    return Err(Error::InvalidSiteId);
                }
            }
        }
        Ok(())
    }


    fn local_value(&self) -> HashSet<T> {
        self.0.keys().map(|value| value.clone()).collect()
    }
}

impl<T: SetElement> Op<T> {
    /// Returns the `Op`'s value.
    pub fn value(&self) -> &T { &self.value }

    /// Returns a reference to the `Op`'s inserted replica.
    pub fn inserted_replica(&self) -> Option<&Replica> { self.inserted_replica.as_ref() }

    /// Returns a reference to the `Op`'s removed replicas.
    pub fn removed_replicas(&self) -> &[Replica] { &self.removed_replicas }

    /// Assigns a site id to any unassigned inserts and removes
    pub fn add_site_id(&mut self, site_id: SiteId) {
        if let Some(ref mut r) = self.inserted_replica {
            if r.site == 0 { r.site = site_id };
        }
        for r in &mut self.removed_replicas {
            if r.site == 0 { r.site = site_id };
        }
    }

    /// Validates that the `Op`'s site id is equal to the given site id.
    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if let Some(ref r) = self.inserted_replica {
            try_assert!(r.site == site_id, Error::InvalidOp);
        }
        Ok(())
    }

    pub(crate) fn inserted_replicas(&self) -> Vec<Replica> {
        match self.inserted_replica {
            Some(ref r) => vec![r.clone()],
            None => vec![],
        }
    }
}
