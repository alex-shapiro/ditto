use Replica;
use Value;
use op::remote::{UpdateObject,UpdateArray,UpdateAttributedString,IncrementNumber};

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
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_object())
            .and_then(|object| Some(object.put(key, value, replica)))
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> Option<UpdateObject> {
        self.root_value
            .get_nested(pointer)
            .and_then(|value| value.as_object())
            .and_then(|object| object.delete(key))
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, item: Value) -> Option<UpdateArray> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_array())
            .and_then(|array| array.insert(index, item, replica))
    }

    pub fn delete_item(&mut self, pointer: &str, index: usize) -> Option<UpdateArray> {
        self.root_value
            .get_nested(pointer)
            .and_then(|value| value.as_array())
            .and_then(|array| array.delete(index))
    }

    pub fn insert_text(&mut self, pointer: &str, index: usize, text: String) -> Option<UpdateAttributedString> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_attributed_string())
            .and_then(|string| string.insert_text(index, text, replica))
    }

    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> Option<UpdateAttributedString> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_attributed_string())
            .and_then(|string| string.delete_text(index, len, replica))
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: String) -> Option<UpdateAttributedString> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_attributed_string())
            .and_then(|string| string.replace_text(index, len, text, replica))
    }

    pub fn increment(&mut self, pointer: &str, amount: f64) -> Option<IncrementNumber> {
        self.root_value
            .get_nested(pointer)
            .and_then(|value| value.increment(amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Value;

    #[test]
    fn test_operations() {
        let mut crdt = CRDT::new_object(1);
        crdt.put("", "foo", Value::object()).unwrap();
        crdt.put("", "bar", Value::array()).unwrap();
        crdt.put("", "baz", Value::attrstr()).unwrap();

        // nested object operations
        crdt.put("/foo", "a", Value::Num(1.0)).unwrap();
        crdt.put("/foo", "b", Value::Bool(true)).unwrap();
        crdt.put("/foo", "c", Value::Str("hm?".to_string())).unwrap();
        crdt.delete("/foo", "b").unwrap();

        // nested array operations
        crdt.insert_item("/bar", 0, Value::Bool(true)).unwrap();
        crdt.insert_item("/bar", 1, Value::Bool(false)).unwrap();
        crdt.insert_item("/bar", 2, Value::Bool(true)).unwrap();
        crdt.delete_item("/bar", 1).unwrap();

        // nested attributed string operations
        crdt.insert_text("/baz", 0, "the ".to_string()).unwrap();
        crdt.insert_text("/baz", 4, "slow ".to_string()).unwrap();
        crdt.delete_text("/baz", 0, 1).unwrap();
        crdt.replace_text("/baz", 4, 4, "quick".to_string()).unwrap();

        // invalid operations
        assert!(None == crdt.put("/bar", "a", Value::Bool(true)));
        assert!(None == crdt.delete("/bar", "a"));
        assert!(None == crdt.delete("/foo", "z"));
        assert!(None == crdt.insert_item("/foo", 0, Value::Num(1.0)));
        assert!(None == crdt.delete_item("/foo", 0));
        assert!(None == crdt.insert_text("/bar", 0, "Hey!".to_string()));
        assert!(None == crdt.delete_text("/bar", 0, 1));
        assert!(None == crdt.replace_text("/bar", 0, 2, "this isn't right".to_string()));
        assert!(None == crdt.increment("/bar", 1.5));
    }
}
