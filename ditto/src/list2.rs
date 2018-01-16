//! A CRDT that stores an ordered sequence of elements

use Error;
use dot::{Dot, Summary, SiteId};
use sequence::uid::{self, UID};
use traits2::*;
use std::borrow::Cow;
use std::mem;
use std::cmp::Ordering;

/// A List is a `Vec`-like ordered sequence of elements.
/// To allow for CRDT replication, List elements must implement
/// the `Clone`, `Serialize`, and `Deserialize` traits.
///
/// Internally, List is based on LSEQ. It allows op-based replication
/// via [`execute_op`](#method.execute_op) and state-based replication
/// via [`merge`](#method.merge). State-based replication allows
/// out-of-order delivery but op-based replication does not.
///
/// An *N*-element List's performance characteristics are:
///
///   * [`insert`](#method.insert): *O(log N)*
///   * [`remove`](#method.remove): *O(log N)*
///   * [`get`](#method.get): *O(1)*
///   * [`len`](#method.insert): *O(1)*
///   * [`execute_op`](#method.execute_op): *O(log N)*
///   * [`merge`](#method.merge): *O(N1 + N2 + S1 + S2)*, where *N1* and
///     *N2* are the number of values in each list being merged,
///     and *S1* and *S2* are the number of sites that have edited
///     each list being merged.
///
///
///   * Unlike a pure CmRDT, it requires tombstones, which increase size.
///   * Unlike a pure CvRDT, it requires each site to replicate its ops
///     in their order of generation.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List<T: 'static> {
    inner:      Inner<T>,
    summary:    Summary,
    site_id:    SiteId,
    cached_ops: Vec<Op<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListState<'a, T: Clone + 'a + 'static> {
    inner: Cow<'a, Inner<T>>,
    summary: Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inner<T: 'static>(pub Vec<Element<T>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op<T> {
    Insert(Element<T>),
    Remove(UID),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LocalOp<T> {
    Insert { idx: usize, value: T },
    Remove { idx: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element<T> {
    pub uid: UID,
    pub value: T,
}

impl<T> PartialEq for Element<T> {
    fn eq(&self, other: &Element<T>) -> bool {
        self.uid == other.uid
    }
}

impl<T> Eq for Element<T> {}

impl<T> PartialOrd for Element<T> {
    fn partial_cmp(&self, other: &Element<T>) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<T> Ord for Element<T> {
    fn cmp(&self, other: &Element<T>) -> Ordering {
        self.uid.cmp(&other.uid)
    }
}

impl<T: Clone> List<T> {

    /// Constructs and returns a new List with site id 1.
    pub fn new() -> Self {
        let inner   = Inner::new();
        let summary = Summary::new();
        let site_id = 1;
        List{inner, summary, site_id, cached_ops: vec![]}
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.inner.0.len()
    }

    /// Returns a reference to the element at position `idx`.
    /// Returns None if idx is out-of-bounds.
    pub fn get(&self, idx: usize) -> Option<&T> {
        Some(&self.inner.0.get(idx)?.value)
    }

    /// Pushes a value onto the end of the list. If the list does
    /// not have a site id, it caches the resulting op and returns an
    /// `AwaitingSiteId` error.
    pub fn push(&mut self, value: T) -> Result<Op<T>, Error> {
        let dot = self.summary.get_dot(self.site_id);
        let op = self.inner.push(value, dot);
        self.after_op(op)
    }

    /// Removes the value at the end of the list, if the list is nonempty.
    /// If the list is empty, it returns None. If the pop succeeds but
    /// the list does not have a site id, it caches the resulting op
    /// and returns an `AwaitingSiteId` error.
    pub fn pop(&mut self) -> Option<(T, Result<Op<T>, Error>)> {
        let (value, op) = self.inner.pop()?;
        Some((value, self.after_op(op)))
    }

    /// Inserts a value at position `idx` in the list, shifting all
    /// elements after it to the right. Panics if the idx is out of
    /// bounds. If the insert succeeds but the list does not have a
    /// site id, it caches the resulting op and returns an
    /// `AwaitingSiteId` error.
    pub fn insert(&mut self, idx: usize, value: T) -> Result<Op<T>, Error> {
        let dot = self.summary.get_dot(self.site_id);
        let op = self.inner.insert(idx, value, dot);
        self.after_op(op)
    }

    /// Removes the element at position `idx` from the list,
    /// shifting all elements after it to the left. Panics
    /// if the idx is out-of-bounds. If the remove succeeds but
    /// the list does not have a site id, it caches the resulting
    /// op and returns an `AwaitingSiteId` error.
    pub fn remove(&mut self, idx: usize) -> (T, Result<Op<T>, Error>) {
        let (value, op) = self.inner.remove(idx);
        (value, self.after_op(op))
    }

    crdt_impl2! {
        List,
        ListState<T>,
        ListState<'static, T>,
        ListState,
        Inner<T>,
        Op<T>,
        Option<LocalOp<T>>,
        Vec<T>,
    }
}

impl<T: Clone> From<Vec<T>> for List<T> {
    fn from(local_value: Vec<T>) -> Self {
        let mut list = List::new();
        for element in local_value {
            let _ = list.push(element);
        }
        list
    }
}

impl<T: Clone> Inner<T> {
    pub fn new() -> Self {
        Inner(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Inner(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, value: T, dot: Dot) -> Op<T> {
        let uid = {
            let len = self.0.len();
            let uid1 = if len == 0 { &*uid::MIN } else { &self.0[len-1].uid };
            let uid2 = &*uid::MAX;
            UID::between(uid1, uid2, &dot)
        };

        let element = Element{uid, value};
        self.0.push(element.clone());
        Op::Insert(element)
    }

    pub fn insert(&mut self, idx: usize, value: T, dot: Dot) -> Op<T> {
        let uid = {
            let len = self.0.len();
            let uid1 = if idx == 0 { &*uid::MIN } else { &self.0[idx-1].uid };
            let uid2 = if idx == len { &*uid::MAX } else { &self.0[idx].uid };
            UID::between(uid1, uid2, &dot)
        };

        let element = Element{uid, value};
        self.0.insert(idx, element.clone());
        Op::Insert(element)
    }

    pub fn pop(&mut self) -> Option<(T, Op<T>)> {
        let element = self.0.pop()?;
        let value = element.value;
        let op = Op::Remove(element.uid);
        Some((value, op))
    }

    pub fn remove(&mut self, idx: usize) -> (T, Op<T>) {
        let element = self.0.remove(idx);
        let value = element.value;
        let op = Op::Remove(element.uid);
        (value, op)
    }

    pub fn execute_op(&mut self, op: Op<T>) -> Option<LocalOp<T>> {
        match op {
            Op::Insert(elt) => {
                let idx = self.0.binary_search_by(|e| e.uid.cmp(&elt.uid)).err()?;
                let value = elt.value.clone();
                self.0.insert(idx, elt);
                Some(LocalOp::Insert{idx, value})
            }
            Op::Remove(uid) => {
                let idx = self.0.binary_search_by(|e| e.uid.cmp(&uid)).ok()?;
                let _   = self.0.remove(idx).value;
                Some(LocalOp::Remove{idx})
            }
        }
    }

    pub fn merge(&mut self, other: Inner<T>, summary: &Summary, other_summary: &Summary) {
        let capacity = self.0.capacity();
        let elements = mem::replace(&mut self.0, Vec::with_capacity(capacity));
        let mut iter = elements.into_iter().peekable();
        let mut other_iter = other.0.into_iter().peekable();

        while iter.peek().is_some() || other_iter.peek().is_some() {
            let ordering = {
                let uid1 = iter.peek().and_then(|e| Some(&e.uid)).unwrap_or(&uid::MAX);
                let uid2 = other_iter.peek().and_then(|e| Some(&e.uid)).unwrap_or(&uid::MAX);
                uid1.cmp(uid2)
            };

            match ordering {
                Ordering::Less => {
                    let element = iter.next().unwrap();
                    if !other_summary.contains_pair(element.uid.site_id, element.uid.counter) {
                        self.0.push(element);
                    }
                }
                Ordering::Equal => {
                    let element = iter.next().unwrap();
                    let _ = other_iter.next().unwrap();
                    self.0.push(element);
                }
                Ordering::Greater => {
                    let element = other_iter.next().unwrap();
                    if !summary.contains_pair(element.uid.site_id, element.uid.counter) {
                        self.0.push(element);
                    }
                }
            }
        }
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        for element in &mut self.0 {
            if element.uid.site_id == 0 { element.uid.site_id = site_id; }
        }
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), Error> {
        if self.0.iter().any(|e| e.uid.site_id == 0) { return Err(Error::InvalidSiteId) }
        Ok(())
    }

    pub fn local_value(&self) -> Vec<T> {
        self.0.iter().map(|e| e.value.clone()).collect()
    }
}

impl<T: Clone + NestedInner> NestedInner for Inner<T> {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        for element in &mut self.0 {
            element.value.nested_add_site_id(site_id);
            if element.uid.site_id == 0 { element.uid.site_id = site_id; }
        }
    }

    fn nested_validate_no_unassigned_sites(&self) -> Result<(), Error> {
        for element in &self.0 {
            element.value.nested_validate_no_unassigned_sites()?;
            if element.uid.site_id == 0 { return Err(Error::InvalidSiteId) };
        }
        Ok(())
    }

    fn nested_validate_all(&self, site_id: SiteId) -> Result<(), Error> {
        for element in &self.0 {
            element.value.nested_validate_all(site_id)?;
            if element.uid.site_id != site_id { return Err(Error::InvalidSiteId) };
        }
        Ok(())
    }

    fn nested_merge(&mut self, other: Inner<T>, summary: &Summary, other_summary: &Summary) {
        let capacity = self.0.capacity();
        let elements = mem::replace(&mut self.0, Vec::with_capacity(capacity));
        let mut iter = elements.into_iter().peekable();
        let mut other_iter = other.0.into_iter().peekable();

        while !(iter.peek().is_some() && other_iter.peek().is_some()) {
            let ordering = {
                let uid1 = iter.peek().and_then(|e| Some(&e.uid)).unwrap_or(&uid::MAX);
                let uid2 = other_iter.peek().and_then(|e| Some(&e.uid)).unwrap_or(&uid::MAX);
                uid1.cmp(uid2)
            };

            match ordering {
                Ordering::Less => {
                    let element = iter.next().unwrap();
                    if !other_summary.contains_pair(element.uid.site_id, element.uid.counter) {
                        self.0.push(element);
                    }
                }
                Ordering::Equal => {
                    let mut element = iter.next().unwrap();
                    let other_element = other_iter.next().unwrap();
                    element.value.nested_merge(other_element.value, summary, other_summary);
                    self.0.push(element);
                }
                Ordering::Greater => {
                    let element = other_iter.next().unwrap();
                    if !summary.contains_pair(element.uid.site_id, element.uid.counter) {
                        self.0.push(element);
                    }
                }
            }
        }
    }
}

impl<T> Op<T> {
    pub fn inserted_element(&self) -> Option<&Element<T>> {
        if let Op::Insert(ref elt) = *self { Some(elt) } else { None }
    }

    pub fn removed_uid(&self) -> Option<&UID> {
        if let Op::Remove(ref uid) = *self { Some(uid) } else { None }
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        match *self {
            Op::Insert(ref mut elt) => {
                if elt.uid.site_id == 0 { elt.uid.site_id = site_id; }
            }
            Op::Remove(ref mut uid) => {
                if uid.site_id == 0 { uid.site_id = site_id; }
            }
        }
    }

    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if let Op::Insert(ref elt) = *self {
            if elt.uid.site_id != site_id { return Err(Error::InvalidOp) };
        }
        Ok(())
    }

    pub(crate) fn inserted_dots(&self) -> Vec<Dot> {
        if let Op::Insert(ref elt) = *self {
            vec![Dot::new(elt.uid.site_id, elt.uid.counter)]
        } else {
            vec![]
        }
    }
}

impl<T: NestedInner> NestedOp for Op<T> {
    fn nested_add_site_id(&mut self, site_id: SiteId) {
        match *self {
            Op::Insert(ref mut elt) => {
                elt.value.nested_add_site_id(site_id);
                if elt.uid.site_id == 0 { elt.uid.site_id = site_id; }
            }
            Op::Remove(ref mut uid) => {
                if uid.site_id == 0 { uid.site_id = site_id; }
            }
        }
    }

    fn nested_validate(&self, site_id: SiteId) -> Result<(), Error> {
        if let Op::Insert(ref elt) = *self {
            if elt.uid.site_id != site_id { return Err(Error::InvalidOp) };
            elt.value.nested_validate_all(site_id)?;
        }
        Ok(())
    }
}
