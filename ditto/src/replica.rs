use map_tuple_vec;
use std::cmp::max;
use std::collections::HashMap;

pub type SiteId = u32;
pub type Counter = u32;
pub type Summary = Tombstones;


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Replica {
    pub site: u32,
    pub counter: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tombstones(#[serde(with = "map_tuple_vec")] HashMap<u32,u32>);

impl Replica {
    pub fn new(site: u32, counter: u32) -> Self {
        Replica{site: site, counter: counter}
    }
}

impl Tombstones {
    pub fn new() -> Self {
        Tombstones(HashMap::new())
    }

    pub fn get(&self, site_id: SiteId) -> Counter {
        *self.0.get(&site_id).unwrap_or(&0)
    }

    pub fn increment(&mut self, site_id: SiteId) -> Counter {
        let entry = self.0.entry(site_id).or_insert(0);
        *entry += 1;
        return *entry;
    }

    pub fn contains(&self, replica: &Replica) -> bool {
        match self.0.get(&replica.site) {
            Some(counter) => *counter >= replica.counter,
            None => false,
        }
    }

    pub fn contains_pair(&self, site: u32, counter: u32) -> bool {
        match self.0.get(&site) {
            Some(site_counter) => *site_counter >= counter,
            None => false,
        }
    }

    pub fn insert(&mut self, replica: &Replica) {
        let entry = self.0.entry(replica.site).or_insert(replica.counter);
        *entry = max(*entry, replica.counter);
    }

    pub fn insert_pair(&mut self, site: u32, counter: u32) {
        let entry = self.0.entry(site).or_insert(counter);
        *entry = max(*entry, counter);
    }

    pub fn merge(&mut self, other: &Tombstones) {
        for (site, counter) in &other.0 {
            let site = *site;
            let counter = *counter;
            let entry = self.0.entry(site).or_insert(counter);
            *entry = max(*entry, counter);
        }
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        let counter = some!(self.0.remove(&0));
        self.0.insert(site_id, counter);
    }
}
