use Replica;
use op::local::{LocalOp, Increment};
use op::remote::IncrementCounter;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Counter {
    pub value: f64,
    pub site_counters: HashMap<u32, u32>,
}

impl Counter {
    pub fn new(value: f64) -> Self {
        Counter{value: value, site_counters: HashMap::new()}
    }

    pub fn increment(&mut self, amount: f64, replica: &Replica) -> IncrementCounter {
        self.value += amount;
        let mut site_counter = self.site_counters.entry(replica.site).or_insert(replica.counter);
        *site_counter = replica.counter;
        IncrementCounter{amount: amount, replica: replica.clone()}
    }

    pub fn execute_remote(&mut self, op: &IncrementCounter) -> Option<LocalOp> {
        let is_duplicate =
            match self.site_counters.get(&op.replica.site) {
                Some(counter) => *counter >= op.replica.counter,
                None => false,
            };

        if is_duplicate {
            None
        } else {
            self.value += op.amount;
            self.site_counters.insert(op.replica.site, op.replica.counter);
            Some(LocalOp::Increment(Increment::new(op.amount)))
        }
    }

    pub fn replicas_vec(&self) -> Vec<Replica> {
        let mut replicas = Vec::with_capacity(self.site_counters.len());
        for (site, counter) in &self.site_counters {
            replicas.push(Replica::new(*site, *counter))
        }
        replicas
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
}
