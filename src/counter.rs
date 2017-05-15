//! A `Counter` stores an incrementable float value.

use Error;
use Replica;
use map_tuple_vec;
use traits::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    value: CounterValue,
    replica: Replica,
    awaiting_site: Vec<RemoteOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterValue {
    value: f64,
    #[serde(with = "map_tuple_vec")]
    site_counters: HashMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp {
    amount: f64,
    replica: Replica,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalOp {
    amount: f64,
}

impl Counter {

    crdt_impl!(Counter, CounterValue);

    pub fn new(value: f64) -> Self {
        let replica = Replica::new(1, 0);
        let value = CounterValue{value: value, site_counters: HashMap::new()};
        Counter{value, replica, awaiting_site: vec![]}
    }

    pub fn increment(&mut self, amount: f64) -> Result<RemoteOp, Error> {
        let op = self.value.increment(amount, &self.replica);
        self.after_op(op)
    }
}

impl CounterValue {
    pub fn increment(&mut self, amount: f64, replica: &Replica) -> RemoteOp {
        self.value += amount;
        let _ = self.site_counters.insert(replica.site, replica.counter);
        RemoteOp{amount: amount, replica: replica.clone()}
    }

    pub fn execute_remote(&mut self, op: &RemoteOp) -> Option<LocalOp> {
        let _ = try_opt!(
            self.site_counters
            .get(&op.replica.site)
            .and_then(|counter| if *counter >= op.replica.counter { Some(()) } else { None }));

        self.value += op.amount;
        let _ = self.site_counters.insert(op.replica.site, op.replica.counter);
        Some(LocalOp{amount: op.amount})
    }
}

impl CrdtValue for CounterValue {
    type LocalValue = f64;
    type RemoteOp = RemoteOp;
    type LocalOp = LocalOp;

    fn local_value(&self) -> f64 {
        self.value
    }

    fn add_site(&mut self, _: &RemoteOp, site: u32) {
        if let Some(counter) = self.site_counters.remove(&0) {
            self.site_counters.insert(site, counter);
        }
    }
}

impl CrdtRemoteOp for RemoteOp {
    fn add_site(&mut self, site: u32) {
        self.replica.site = site;
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        try_assert!(self.replica.site == site, Error::InvalidRemoteOp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let counter = Counter::new(8434.54);
        assert!(counter.value == 8434.54);
        assert!(counter.site_counters.is_empty());
    }

    #[test]
    fn test_increment() {
        let mut counter = Counter::new(843.3);
        let r1 = Replica::new(1,2);
        let r2 = Replica::new(1,3);
        let op1 = counter.increment(4.2, &r1);
        let op2 = counter.increment(-3.4, &r2);

        let mut sc = HashMap::new();
        sc.insert(1, 3);

        assert!(counter.value == 844.1);
        assert!(counter.site_counters == sc);

        assert!(op1.amount == 4.2);
        assert!(op1.replica == r1);
        assert!(op2.amount == -3.4);
        assert!(op2.replica == r2);
    }

    #[test]
    fn test_execute_remote() {
        let mut counter1 = Counter::new(3.0);
        let mut counter2 = Counter::new(3.0);

        let op1 = counter1.increment(1.0, &Replica{site: 1, counter: 2});
        let op2 = counter1.increment(5.0, &Replica{site: 1, counter: 3});
        let op3 = counter1.increment(7.0, &Replica{site: 2, counter: 1});

        let lop1 = counter2.execute_remote(&op1).unwrap();
        let lop2 = counter2.execute_remote(&op2).unwrap();
        let lop3 = counter2.execute_remote(&op3).unwrap();

        assert!(counter1 == counter2);
        assert!(lop1.increment().unwrap().amount == 1.0);
        assert!(lop2.increment().unwrap().amount == 5.0);
        assert!(lop3.increment().unwrap().amount == 7.0);
    }

    #[test]
    fn test_execute_remote_ignore_duplicate() {
        let mut counter = Counter::new(0.0);
        let op1 = counter.increment(2.0, &Replica{site: 1, counter: 2});
        let op2 = counter.increment(5.0, &Replica{site: 1, counter: 3});

        assert!(counter.execute_remote(&op1).is_none());
        assert!(counter.execute_remote(&op2).is_none());
        assert!(counter.value == 7.0);
    }

    #[test]
    fn test_update_site() {
        let mut counter = Counter::new(0.0);
        let op = counter.increment(1.0, &Replica::new(0, 84));
        let _  = counter.increment(1.0, &Replica::new(0, 85));

        counter.update_site(&op, 111);
        assert!(counter.site_counters.get(&111) == Some(&85));
    }
}
