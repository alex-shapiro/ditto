use std::cmp::Ordering;
use Replica;

#[derive(Clone,Eq)]
pub struct UID {
    pub key: String,
    pub site: u32,
    counter: u32,
}

impl UID {
    pub fn new(key: &str, replica: &Replica) -> UID {
        UID{key: key.to_string(), site: replica.site, counter: replica.counter}
    }
}

impl PartialEq for UID {
    fn eq(&self, other: &UID) -> bool {
        self.site == other.site && self.counter == other.counter
    }
}

impl PartialOrd for UID {
    fn partial_cmp(&self, other: &UID) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UID {
    fn cmp(&self, other: &UID) -> Ordering {
        if self.site < other.site {
            Ordering::Less
        } else if self.site == other.site && self.counter < other.counter {
            Ordering::Less
        } else if self.site == other.site && self.counter == other.counter {
            Ordering::Equal
        } else {
            Ordering::Greater
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;

    #[test]
    fn test_new() {
        let replica = Replica{site: 1, counter: 23};
        let uid = UID::new("foo", &replica);
        assert!(uid.key == String::from("foo"));
        assert!(uid.site == 1);
        assert!(uid.counter == 23);
    }

    #[test]
    fn test_equality() {
        let replica1 = Replica::new(1, 23);
        let replica2 = Replica::new(2, 13);

        let uid1 = UID::new("foo", &replica1);
        let uid2 = UID::new("bar", &replica1);
        let uid3 = UID::new("foo", &replica2);
        assert!(uid1 == uid2);
        assert!(uid1 != uid3);
    }
}
