//! A CRDT that stores an `i64` value that increments

use {Error, Replica, Tombstones};
use traits::*;
use std::borrow::Cow;
use std::collections::hash_map::{HashMap, Entry};

type SiteId = u32;

/// A Counter is an `i64` value that can be incremented and
/// decremented via the [`increment`](#method.increment) function.
///
/// Internally, Counter is both a CmRDT and a CvRDT - it can provide
/// eventual consistency via both operations and state merges.
/// This flexibility comes with a set of tradeoffs:
///
/// * It is larger than a pure CmRDT (which is just an `i64` value),
///   but it can perform stateful merges (which a pure CmRDT cannot do).
///
/// * Unlike a pure CvRDT (e.g. a G-counter or PN-counter), it requires
///   each site to replicate its ops in their order of generation.
///   However, it requires much less storage space than a CvRDT because
///   it stores the *net* increment value from each site rather than *all*
///   increment values from each site.
///
/// Counter is an excellent choice if you require a counter that
/// can perform stateful merges and you know that each site can
/// replicate its ops in order.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    value:         CounterValue,
    replica:       Replica,
    tombstones:    Tombstones,
    awaiting_site: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterState<'a> {
    value: Cow<'a, CounterValue>,
    tombstones: Cow<'a, Tombstones>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterValue(HashMap<SiteId, SiteCount>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SiteCount {
    inc: i64,
    counter: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    site:    SiteId,
    counter: u32,
    inc:     i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalOp(pub i64);

impl Counter {

    /// Constructs and returns a new Counter with site 1.
    pub fn new(value: i64) -> Self {
        let mut replica = Replica::new(1, 0);
        let value = CounterValue::new(value, &replica);
        let tombstones = Tombstones::new();
        replica.counter += 1;
        Counter{value, replica, tombstones, awaiting_site: vec![]}
    }

    /// Returns the counter's value.
    pub fn get(&self) -> i64 {
        self.value.get()
    }

    /// Increments the counter's value by the given amount and
    /// returns an op that can be replicated to other sites.
    /// If the counter does not have a site id, it caches the
    /// op and returns an `AwaitingSite` error.
    pub fn increment(&mut self, amount: i64) -> Result<Op, Error> {
        let op = self.value.increment(amount, &self.replica);
        self.after_op(op)
    }

    crdt_impl!(Counter, CounterState, CounterState, CounterState<'static>, CounterValue);
}

impl CounterValue {

    fn new(count: i64, replica: &Replica) -> Self {
        let site_count = SiteCount{inc: count, counter: replica.counter};
        let mut map = HashMap::new();
        map.insert(replica.site, site_count);
        CounterValue(map)
    }

    fn get(&self) -> i64 {
        self.0.values().fold(0, |sum, site_count| sum + site_count.inc)
    }

    fn increment(&mut self, amount: i64, replica: &Replica) -> Op {
        let site_count = self.0
            .entry(replica.site)
            .or_insert_with(|| SiteCount{inc: 0, counter: 0});

        site_count.inc += amount;
        site_count.counter = replica.counter;
        Op{site: replica.site, counter: replica.counter, inc: amount}
    }

    pub fn execute_remote(&mut self, op: &Op) -> Option<LocalOp> {
        let Op{site, inc, counter} = *op;
        match self.0.entry(site) {
            Entry::Vacant(entry) => {
                if counter == 0 {
                    entry.insert(SiteCount{inc, counter});
                    Some(LocalOp(inc))
                } else {
                    None
                }
            }
            Entry::Occupied(mut entry) => {
                let site_count = entry.get_mut();
                if site_count.counter + 1 == counter {
                    site_count.inc += inc;
                    site_count.counter = counter;
                    Some(LocalOp(inc))
                } else {
                    None
                }
            }
        }
    }
}

impl CrdtValue for CounterValue {
    type LocalValue = i64;
    type RemoteOp = Op;
    type LocalOp = LocalOp;

    fn local_value(&self) -> i64 {
        self.get()
    }

    fn add_site(&mut self, _: &Op, site: SiteId) {
        self.add_site_to_all(site)
    }

    fn add_site_to_all(&mut self, site: SiteId) {
        let site_count = some!(self.0.remove(&0));
        self.0.insert(site, site_count);
    }

    fn validate_site(&self, site: SiteId) -> Result<(), Error> {
        if self.0.contains_key(&site) { Ok(()) } else { Err(Error::InvalidRemoteOp) }
    }

    fn merge(&mut self, other: CounterValue, _: &Tombstones, _: &Tombstones) {
        for (site, other_site_count) in other.0 {
            let site_count = self.0
                .entry(site)
                .or_insert_with(|| other_site_count.clone());

            if site_count.counter < other_site_count.counter {
                site_count.inc = other_site_count.inc;
                site_count.counter = other_site_count.counter;
            }
        }
    }
}

impl CrdtRemoteOp for Op {
    fn deleted_replicas(&self) -> Vec<Replica> {
        vec![]
    }

    fn add_site(&mut self, site: SiteId) {
        self.site = site;
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        if self.site == site { Ok(()) } else { Err(Error::InvalidRemoteOp) }
    }
}

impl Op {
    pub fn site(&self) -> SiteId { self.site }
    pub fn counter(&self) -> u32 { self.counter }
    pub fn inc(&self) -> i64 { self.inc }
}
