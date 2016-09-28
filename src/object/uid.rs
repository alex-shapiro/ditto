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

#[test]
fn test_new() {
    let uid = UID::new("foo", 1, 23);
    assert!(uid.key == String::from("foo"));
    assert!(uid.site == 1);
    assert!(uid.counter == 23);
}
