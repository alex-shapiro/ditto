use map_tuple_vec;
use std::cmp::max;
use std::collections::HashMap;

pub type Replica = Dot;
pub type SiteId = u32;
pub type Counter = u32;
pub type Tombstones = Summary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Dot {
    pub site_id: SiteId,
    pub counter: Counter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary(#[serde(with = "map_tuple_vec")] HashMap<u32,u32>);

impl Dot {
    pub fn new(site_id: SiteId, counter: Counter) -> Self {
        Dot{site_id, counter}
    }
}

impl Summary {
    pub fn new() -> Self {
        Summary(HashMap::new())
    }

    pub fn get(&self, site_id: SiteId) -> Counter {
        *self.0.get(&site_id).unwrap_or(&0)
    }

    pub fn get_dot(&mut self, site_id: SiteId) -> Dot {
        let counter = self.increment(site_id);
        Dot{site_id, counter}
    }

    pub fn increment(&mut self, site_id: SiteId) -> Counter {
        let entry = self.0.entry(site_id).or_insert(0);
        *entry += 1;
        return *entry;
    }

    pub fn contains(&self, dot: &Dot) -> bool {
        match self.0.get(&dot.site_id) {
            Some(counter) => *counter >= dot.counter,
            None => false,
        }
    }

    pub fn contains_pair(&self, site_id: u32, counter: u32) -> bool {
        match self.0.get(&site_id) {
            Some(site_counter) => *site_counter >= counter,
            None => false,
        }
    }

    pub fn insert(&mut self, dot: &Dot) {
        let entry = self.0.entry(dot.site_id).or_insert(dot.counter);
        *entry = max(*entry, dot.counter);
    }

    pub fn insert_pair(&mut self, site_id: u32, counter: u32) {
        let entry = self.0.entry(site_id).or_insert(counter);
        *entry = max(*entry, counter);
    }

    pub fn merge(&mut self, other: &Summary) {
        for (site_id, counter) in &other.0 {
            let site_id = *site_id;
            let counter = *counter;
            let entry = self.0.entry(site_id).or_insert(counter);
            *entry = max(*entry, counter);
        }
    }

    pub fn add_site_id(&mut self, site_id: SiteId) {
        if let Some(counter) = self.0.remove(&0) {
            self.0.insert(site_id, counter);
        }
    }

    pub fn validate_no_unassigned_sites(&self) -> Result<(), ::Error> {
        if self.0.contains_key(&0) { Err(::Error::InvalidSiteId) } else { Ok(()) }
    }
}
