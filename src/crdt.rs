use Replica;
use Value;
use op::remote::{UpdateObject,UpdateArray,UpdateAttributedString};

pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    pub fn new(value: Value, site: u32) -> Self {
        CRDT{root_value: value, replica: Replica::new(site, 0)}
    }

    pub fn new_object(site: u32) -> Self {
        CRDT::new(Value::object(), site)
    }

    pub fn new_array(site: u32) -> Self {
        CRDT::new(Value::array(), site)
    }

    pub fn new_attrstr(site: u32) -> Self {
        CRDT::new(Value::attrstr(), site)
    }

    pub fn put(&mut self, pointer: &str, key: &str, value: Value) -> Option<UpdateObject> {
        self
        .root_value
        .get_nested(pointer)
        .and_then(|value| value.as_object())
        .and_then(|object| Some(object.put(key, value, &self.replica)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put() {
        let crdt = CRDT::new_object(1);
        crdt.put("", "foo".to_string(), Value::Num(1.0));
    }

}
