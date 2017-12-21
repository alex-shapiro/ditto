//! A CRDT that stores an `i64` value that increments

use Error;
use replica::{SiteId, Counter as RCounter};
use std::borrow::Cow;
use std::collections::HashMap;

/// A Counter is an `i64` value that can be incremented and
/// decremented via the [`increment`](#method.increment) function.
///
/// Internally, Counter is a variant of GCounter that allows
/// op-based replication via [`execute_op`](#method.execute_op)
/// and state-based replication via [`merge`](#method.merge). Both
/// replication methods are idempotent and can handle out-of-order
/// delivery.
///
/// Counter has the following performance characteristics:
///
///   * [`increment`](#method.increment): O(1)
///   * [`execute_op`](#method.execute_op): O(1)
///   * [`merge`](#method.merge): O(*N*), where *N* is the number of sites that have incremented the counter
///   * space: O(*N*), where *N* is the number of sites that have incremented the counter
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    inner:            CounterInner,
    site_id:          SiteId,
    awaiting_site_id: Option<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterState<'a>(Cow<'a, CounterInner>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterInner(HashMap<SiteId, SiteInc>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SiteInc {
    inc:     i64,
    counter: RCounter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    site_id: SiteId,
    counter: RCounter,
    inc:     i64,
}

impl Counter {

    /// Constructs and returns a new Counter with site id 1.
    pub fn new(value: i64) -> Self {
        let site_id = 1;
        let inner = CounterInner::new(value, site_id);
        Counter{inner, site_id, awaiting_site_id: None}
    }

    /// Returns the counter value.
    pub fn get(&self) -> i64 {
        self.inner.get()
    }

    /// Increments the counter by the given amount and
    /// returns an op that can be replicated to other sites.
    /// If the counter does not have a site id, it caches the
    /// op and returns an `AwaitingSiteId` error.
    pub fn increment(&mut self, amount: i64) -> Result<Op, Error> {
        let op = self.inner.increment(amount, self.site_id);
        if self.site_id == 0 {
            self.awaiting_site_id = Some(op);
            Err(Error::AwaitingSiteId)
        } else {
            Ok(op)
        }
    }

    /// Returns the Counter site id.
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// Returns a reference to the Counter state.
    pub fn state(&self) -> CounterState {
        CounterState(Cow::Borrowed(&self.inner))
    }

    /// Clones and returns the Counter state.
    pub fn clone_state(&self) -> CounterState<'static> {
        CounterState(Cow::Owned(self.inner.clone()))
    }

    /// Consumes the Counter and returns its state.
    pub fn into_state(self) -> CounterState<'static> {
        CounterState(Cow::Owned(self.inner))
    }

    /// Constructs a new Counter from a state and optional site id.
    /// If the site is given, it must be nonzero.
    pub fn from_state(state: CounterState, site_id: Option<SiteId>) -> Result<Self, Error> {
        let site_id = match site_id {
            None => 0,
            Some(0) => return Err(Error::InvalidSiteId),
            Some(s) => s,
        };

        Ok(Counter{
            inner: state.0.into_owned(),
            site_id: site_id,
            awaiting_site_id: None,
        })
    }

    /// Returns the counter's equivalent local value.
    pub fn local_value(&self) -> i64 {
        self.inner.get()
    }

    /// Executes an Op and returns the equivalent increment.
    /// If the op has already been executed or superceded,
    /// nothing is done.
    pub fn execute_op(&mut self, op: &Op) -> Option<i64> {
        self.inner.execute_op(op)
    }

    /// Validates that an op comes from a specific site id,
    /// then executes the op.
    pub fn validate_and_execute_op(&mut self, op: &Op, site_id: SiteId) -> Result<Option<i64>, Error> {
        op.validate(site_id)?;
        Ok(self.execute_op(op))
    }

    /// Merges remote state into the Counter.
    pub fn merge(&mut self, other: CounterState) {
        self.inner.merge(other.0.into_owned())
    }

    /// Assigns a site id and returns a cached op if it exists.
    pub fn add_site_id(&mut self, site_id: SiteId) -> Result<Option<Op>, Error> {
        if self.site_id != 0 { return Err(Error::AlreadyHasSiteId) }
        self.site_id = site_id;
        self.inner.add_site_id(site_id);

        if let Some(mut op) = self.awaiting_site_id.take() {
            op.add_site_id(site_id);
            Ok(Some(op))
        } else {
            Ok(None)
        }
    }
}

impl CounterInner {
    fn new(inc: i64, site_id: SiteId) -> Self {
        let mut map = HashMap::new();
        map.insert(site_id, SiteInc{inc, counter: 1});
        CounterInner(map)
    }

    fn get(&self) -> i64 {
        self.0.values().fold(0, |sum, site_count| sum + site_count.inc)
    }

    fn increment(&mut self, amount: i64, site_id: SiteId) -> Op {
        let site_inc = self.0
            .entry(site_id)
            .or_insert_with(|| SiteInc{inc: 0, counter: 0});

        site_inc.inc += amount;
        site_inc.counter += 1;
        Op{site_id, counter: site_inc.counter, inc: site_inc.inc}
    }

    fn execute_op(&mut self, op: &Op) -> Option<i64> {
        let Op{site_id, counter, inc} = *op;
        let site_inc = self.0
            .entry(site_id)
            .or_insert_with(|| SiteInc{inc: 0, counter: 0});

        if site_inc.counter >= counter {
            None
        } else {
            let diff = inc - site_inc.inc;
            site_inc.counter = counter;
            site_inc.inc = inc;
            Some(diff)
        }
    }

    fn merge(&mut self, other: CounterInner) {
        for (site_id, SiteInc{inc, counter}) in other.0 {
            let site_inc = self.0
                .entry(site_id)
                .or_insert_with(|| SiteInc{inc: 0, counter: 0});

            if counter > site_inc.counter {
                site_inc.inc = inc;
                site_inc.counter = counter;
            }
        }
    }

    fn add_site_id(&mut self, site_id: SiteId) {
        let site_inc = some!(self.0.remove(&site_id));
        self.0.insert(site_id, site_inc);
    }
}

impl Op {
    /// Returns the Op site id.
    pub fn site_id(&self) -> SiteId { self.site_id }

    /// returns the Op counter.
    pub fn counter(&self) -> RCounter { self.counter }

    /// Returns the Op inc.
    pub fn inc(&self) -> i64 { self.inc }

    /// Assigns a new site id to the op.
    pub fn add_site_id(&mut self, site_id: SiteId) {
        self.site_id = site_id;
    }

    /// Validates that the Op's site id is equal to the given site id.
    pub fn validate(&self, site_id: SiteId) -> Result<(), Error> {
        if self.site_id == site_id { Ok(()) } else { Err(Error::InvalidOp) }
    }
}
