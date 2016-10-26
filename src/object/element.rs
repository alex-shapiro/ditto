use object::uid::UID;
use Replica;
use Value;

#[derive(Clone,PartialEq,Debug)]
pub struct Element {
    pub uid: UID,
    pub value: Value,
}

impl Element {
    pub fn new(key: &str, value: Value, replica: &Replica) -> Element {
        Element{uid: UID::new(key, replica), value: value}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;
    use Value;

    #[test]
    fn test_new() {
        let replica = Replica{site: 1, counter: 1};
        let val = Value::Str("bar".to_string());
        let elt = Element::new("foo", val, &replica);
        assert!(elt.value == Value::Str("bar".to_string()));
    }
}
