#[derive(Clone)]
pub struct UID {
    key: String,
    site: u32,
    counter: u32,
}

impl UID {
    pub fn new(key: &str, site: u32, counter: u32) -> UID {
        UID{key: key.to_string(), site: site, counter: counter}
    }
}

impl PartialEq for UID {
    fn eq(&self, other: &UID) -> bool {
        self.site == other.site && self.counter == other.counter
    }
}

#[test]
fn test_new() {
    let uid = UID::new("foo", 1, 23);
    assert!(uid.key == String::from("foo"));
    assert!(uid.site == 1);
    assert!(uid.counter == 23);
}

#[test]
fn test_equality() {
    let uid1 = UID::new("foo", 1, 23);
    let uid2 = UID::new("bar", 1, 23);
    let uid3 = UID::new("foo", 2, 13);
    assert!(uid1 == uid2);
    assert!(uid1 != uid3);
}
