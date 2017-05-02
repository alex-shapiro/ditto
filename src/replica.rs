#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Replica {
    pub site: u32,
    pub counter: u32,
}

impl Replica {
    pub fn new(site: u32, counter: u32) -> Self {
        Replica{site: site, counter: counter}
    }
}
