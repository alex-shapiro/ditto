use map_tuple_vec;
use std::cmp::max;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Replica {
    pub site: u32,
    pub counter: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tombstones {
    #[serde(with = "map_tuple_vec")]
    inner: HashMap<u32,u32>,
}

impl Replica {
    pub fn new(site: u32, counter: u32) -> Self {
        Replica{site: site, counter: counter}
    }
}


impl Tombstones {
    pub fn new() -> Self {
        Tombstones{inner: HashMap::new()}
    }

    pub fn includes(&self, replica: &Replica) -> bool {
        match self.inner.get(&replica.site) {
            Some(counter) => *counter >= replica.counter,
            None => false,
        }
    }

    pub fn insert(&mut self, replica: &Replica) {
        let entry = self.inner.entry(replica.site).or_insert(replica.counter);
        *entry = max(*entry, replica.counter);
    }

    pub fn merge(&mut self, other: Tombstones) {
        for (site, counter) in other.inner.into_iter() {
            let entry = self.inner.entry(site).or_insert(counter);
            *entry = max(*entry, counter);
        }
    }
}
