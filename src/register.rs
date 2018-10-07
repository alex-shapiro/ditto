//! A CRDT that stores a replaceable value

use Error;
use dot::{Dot, SiteId, Counter, Summary};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::mem;

/// A Register is a replaceable value that can be updated
/// via the [`update`](#method.update) function.
///
/// Register allows op-based replication via [`execute_op`](#method.execute_op)
/// and state-based replication via [`merge`](#method.merge).
/// Both replication methods are idempotent and can handle
/// out-of-order delivery.
///
/// `Register` has a spatial complexity of *O(N + S)*, where
/// *N* is the number of values concurrently held in the `Register` and
/// *S* is the number of sites that have updated the `Register`.
/// It has the following performance characteristics:
///
///   * [`update`](#method.update): *O(1)*
///   * [`execute_op`](#method.execute_op): *O(N)*, where *N* is
///     the number of values concurrently held in the `Register`.
///   * [`merge`](#method.merge): *O(N + M)*, where *N* and *M* are
///     the number of values concurrently held in the `Register` being
///     merged into and the `RegisterState` being merged, respectively.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register<T: Clone> {
    elements:  BTreeMap<SiteId, SiteValue<T>>,
    summary:   Summary,
    site_id:   SiteId,
    cached_op: Option<Op<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterState<'a, T: Clone + 'a> {
    elements: Cow<'a, BTreeMap<SiteId, SiteValue<T>>>,
    summary:  Cow<'a, Summary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op<T: Clone> {
    site_id: SiteId,
    counter: Counter,
    value: T,
    removed_dots: Vec<Dot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SiteValue<T: Clone> {
    value:   T,
    counter: Counter,
}

impl<T: Clone> Register<T> {

    /// Constructs and returns a new `Register` with site id 1.
    pub fn new(value: T) -> Self {
        Self::new_with_id(value,1)
    }

    /// Constructs and returns a new `Register` with the given site id.
    pub fn new_with_id(value: T, site_id: u32) -> Self {
        let counter = 1;
        let mut elements = BTreeMap::new();
        let mut summary = Summary::default();
        let _ = elements.insert(site_id, SiteValue{value, counter});
        summary.insert_pair(site_id, counter);
        Register{elements, summary, site_id, cached_op: None}
    }

    /// Returns a reference to the `Register`'s value.
    pub fn get(&self) -> &T {
        &self.elements.values().next().as_ref().unwrap().value
    }

    /// Updates the `Register`'s value and returns an op
    /// that can be replciated to other sites.
    /// If the register does not have a site id allocated, it
    /// caches the op and returns an `AwaitingSiteId` error.
    pub fn update(&mut self, value: T) -> Result<Op<T>, Error> {
        let counter = self.summary.increment(self.site_id);

        let mut new_elements = BTreeMap::new();
        new_elements.insert(self.site_id, SiteValue{value: value.clone(), counter});

        let removed_dots = mem::replace(&mut self.elements, new_elements)
            .into_iter()
            .filter_map(|(site_id, site_value)|
                if site_id == self.site_id {
                    None
                } else {
                    Some(Dot::new(site_id, site_value.counter))
                })
            .collect();

        let op = Op{site_id: self.site_id, value, counter, removed_dots};

        if self.site_id == 0 {
            self.cached_op = Some(op);
            Err(Error::AwaitingSiteId)
        } else {
            Ok(op)
        }
    }

    /// Executes an Op and returns a reference to the new value if
    /// the value has changed. If the op has already been executed
    /// or superceded, nothing is done.
    pub fn execute_op(&mut self, op: Op<T>) -> &T {
        for Dot{site_id, counter} in op.removed_dots {
            // remove any elements that were removed by the op.
            if let Some(site_value) = self.elements.remove(&site_id) {
                if site_value.counter > counter {
                    self.elements.insert(site_id, site_value);
                }
            }
        }

        // insert the element that is inserted by the op
        self.summary.insert_pair(op.site_id, op.counter);
        let sv_other = SiteValue{value: op.value, counter: op.counter};
        if let Some(sv_self) = self.elements.insert(op.site_id, sv_other) {
            if sv_self.counter > op.counter {
                self.elements.insert(op.site_id, sv_self);
            }
        }

        self.get()
    }

    /// Merges remote state into the Register
    pub fn merge(&mut self, other: RegisterState<T>) {
        let mut other_elements = other.elements.into_owned();
        let self_elements = mem::replace(&mut self.elements, BTreeMap::new());

        // retain any element that is either:
        // - in both self and other
        // - in self and not yet inserted into other
        // - in other and not yet inserted into self
        for (site_id, sv_self) in self_elements {
            if let Some(sv_other) = other_elements.remove(&site_id) {
                let sv = if sv_self.counter > sv_other.counter { sv_self } else { sv_other };
                self.summary.insert_pair(site_id, sv.counter);
                self.elements.insert(site_id, sv);
            } else if !other.summary.contains_pair(site_id, sv_self.counter) {
                self.summary.insert_pair(site_id, sv_self.counter);
                self.elements.insert(site_id, sv_self);
            }
        }

        // insert any element that has been inserted into other but not self
        for (site_id, sv) in other_elements {
            if !self.summary.contains_pair(site_id, sv.counter) {
                self.summary.insert_pair(site_id, sv.counter);
                self.elements.insert(site_id, sv);
            }
        }
    }

    /// Assigns a site id and returns a cached op if it exists.
    pub fn add_site_id(&mut self, site_id: SiteId) -> Result<Option<Op<T>>, Error> {
        if self.site_id != 0 {
            return Err(Error::AlreadyHasSiteId)
        }

        self.site_id = site_id;
        self.summary.add_site_id(site_id);

        if let Some(site_value) = self.elements.remove(&0) {
            self.elements.insert(site_id, site_value);
        }

        if let Some(mut op) = self.cached_op.take() {
            op.add_site_id(site_id);
            Ok(Some(op))
        } else {
            Ok(None)
        }
    }

    /// Returns the `Register`'s site id.
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// Returns a reference to the `Register`'s summary.
    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    /// Returns a borrowed RegisterState.
    pub fn state(&self) -> RegisterState<T> {
        RegisterState{
            elements: Cow::Borrowed(&self.elements),
            summary: Cow::Borrowed(&self.summary),
        }
    }

    /// Returns an owned RegisterState of cloned values.
    pub fn clone_state(&self) -> RegisterState<'static, T> {
        RegisterState {
            elements: Cow::Owned(self.elements.clone()),
            summary: Cow::Owned(self.summary.clone()),
        }
    }

    /// Consumes the Register and returns its RegisterState
    pub fn into_state(self) -> RegisterState<'static, T> {
        RegisterState {
            elements: Cow::Owned(self.elements),
            summary: Cow::Owned(self.summary),
        }
    }

    /// Constructs a new Register from a RegisterState and an
    /// optional site id. If the site id is given, it must be nonzero.
    pub fn from_state(state: RegisterState<T>, site_id: Option<SiteId>) -> Result<Self, Error> {
        let site_id = match site_id {
            None => 0,
            Some(0) => return Err(Error::InvalidSiteId),
            Some(s) => s,
        };

        Ok(Register{
            elements: state.elements.into_owned(),
            summary: state.summary.into_owned(),
            site_id,
            cached_op: None,
        })
    }

    /// Validates that an op comes from a specific site id,
    /// then executes the op.
    pub fn validate_and_execute_op(&mut self, op: Op<T>, site_id: SiteId) -> Result<&T, Error> {
        op.validate(site_id)?;
        Ok(self.execute_op(op))
    }
}

impl<T: Clone> Op<T> {
    /// Returns the `Op`'s site_id
    pub fn site_id(&self) -> SiteId { self.site_id }

    /// Returns the `Op`'s counter
    pub fn counter(&self) -> Counter { self.counter }

    /// Returns a reference to the `Op`'s value
    pub fn value(&self) -> &T { &self.value }

    /// Returns a reference to the `Op`'s removed dots
    pub fn removed_dots(&self) -> &[Dot] {
        &self.removed_dots
    }

    /// Assigns a new site id to the `Op`
    pub fn add_site_id(&mut self, site_id: SiteId) {
        self.site_id = site_id;
    }

    /// Validates that the Op's site id is equal to the given site id.
    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if self.site_id == site_id { Ok(()) } else { Err(Error::InvalidOp) }
    }
}
