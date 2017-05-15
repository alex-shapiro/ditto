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
        if let Some(counter) = self.site_counters.get(&op.replica.site).clone() {
            if *counter >= op.replica.counter { return None }
        }

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
    use serde_json;
    use rmp_serde;

    #[test]
    fn test_new() {
        let counter = Counter::new(8434.54);
        assert!(counter.value.value == 8434.54);
        assert!(counter.value.site_counters.is_empty());
        assert!(counter.replica == Replica{site: 1, counter: 0});
    }

    #[test]
    fn test_increment() {
        let mut counter = Counter::new(843.3);
        let remote_op1 = counter.increment(4.2).unwrap();
        let remote_op2 = counter.increment(-3.4).unwrap();

        assert!(counter.replica.counter == 2);
        assert!(counter.value.value == 844.1);
        assert!(*counter.value.site_counters.get(&1).unwrap() == 1);

        assert!(remote_op1.replica == Replica{site: 1, counter: 0});
        assert!(remote_op1.amount == 4.2);
        assert!(remote_op2.replica == Replica{site: 1, counter: 1});
        assert!(remote_op2.amount == -3.4);
    }

    #[test]
    fn test_increment_awaiting_site() {
        let counter1 = Counter::new(843.3);
        let mut counter2 = Counter::from_value(counter1.clone_value(), 0);
        assert!(counter2.increment(-0.3).unwrap_err() == Error::AwaitingSite);
        assert!(counter2.value.value == 843.0);
        assert!(counter2.awaiting_site.len() == 1);
    }

    #[test]
    fn test_execute_remote() {
        let mut counter1 = Counter::new(3.0);
        let mut counter2 = Counter::from_value(counter1.clone_value(), 2);
        let remote_op = counter1.increment(1.0).unwrap();
        let local_op = counter2.execute_remote(&remote_op).unwrap();

        assert!(counter1.value() == counter2.value());
        assert!(local_op.amount == 1.0);
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut counter1 = Counter::new(3.0);
        let mut counter2 = Counter::from_value(counter1.clone_value(), 2);
        let remote_op = counter1.increment(1.0).unwrap();
        assert!(counter2.execute_remote(&remote_op).is_some());
        assert!(counter2.execute_remote(&remote_op).is_none());
        assert!(counter1.value() == counter2.value());
    }

    #[test]
    fn test_add_site() {
        let counter1 = Counter::new(123.0);
        let mut counter2 = Counter::from_value(counter1.clone_value(), 0);
        let _ = counter2.increment(3.0);
        let _ = counter2.increment(-5.0);
        let _ = counter2.increment(7.0);
        let remote_ops = counter2.add_site(17).unwrap();
        assert!(remote_ops.len() == 3);
        assert!(counter2.replica.site == 17);
        assert!(*counter2.value.site_counters.get(&17).unwrap() == 2);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let counter1 = Counter::new(123.0);
        let mut counter2 = Counter::from_value(counter1.clone_value(), 88);
        let _ = counter2.increment(3.0);
        let _ = counter2.increment(-5.0);
        let _ = counter2.increment(7.0);
        assert!(counter2.add_site(17).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let counter1 = Counter::new(123.4);
        let s_json = serde_json::to_string(&counter1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&counter1).unwrap();
        let counter2: Counter = serde_json::from_str(&s_json).unwrap();
        let counter3: Counter = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(counter1 == counter2);
        assert!(counter1 == counter3);
    }

    #[test]
    fn test_serialize_value() {
        let counter1 = Counter::new(123.4);
        let s_json = serde_json::to_string(counter1.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(counter1.value()).unwrap();
        let value2: CounterValue = serde_json::from_str(&s_json).unwrap();
        let value3: CounterValue = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*counter1.value() == value2);
        assert!(*counter1.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let remote_op1 = RemoteOp{amount: -3.723, replica: Replica::new(23, 5)};
        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let local_op1 = LocalOp{amount: -3.723};
        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }
}
