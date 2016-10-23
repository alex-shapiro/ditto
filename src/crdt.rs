use Replica;
use Value;
use op::remote::{UpdateObject,UpdateArray,UpdateAttributedString,IncrementNumber};
use raw;
use serde_json;
use serde_json::Value as Json;
use compact;

pub struct CRDT {
    root_value: Value,
    replica: Replica,
}

impl CRDT {
    pub fn new(json: &Json, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        let value = raw::decode(json, &replica);
        CRDT{root_value: value, replica: replica}
    }

    pub fn deserialize(json: &Json) -> Result<Self, compact::decoder::Error> {
        let replica = Replica::new(1, 0);
        let value = try!(compact::decode(json));
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn serialize(&self) -> Json {
        compact::encode(&self.root_value)
    }

    pub fn new_str(string: &str, site: u32) -> Self {
        let json: Json = serde_json::de::from_str(string).expect("invalid JSON!");
        CRDT::new(&json, site)
    }

    pub fn get(&mut self, pointer: &str) -> Option<Json> {
        self.root_value
            .get_nested(pointer)
            .and_then(|value| Some(raw::encode(value)))
    }

    pub fn get_str(&mut self, pointer: &str) -> Option<String> {
        self.get(pointer).and_then(|json| {
            Some(serde_json::ser::to_string(&json).ok().unwrap())
        })
    }

    pub fn put(&mut self, pointer: &str, key: &str, value: &Json) -> Option<UpdateObject> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_object())
            .and_then(|object| Some(object.put(key, raw::decode(value, replica), replica)))
    }

    pub fn put_str(&mut self, pointer: &str, key: &str, value: &str) -> Option<UpdateObject> {
        let json: Json = serde_json::de::from_str(value).expect("invalid JSON!");
        self.put(pointer, key, &json)
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> Option<UpdateObject> {
        self.root_value
            .get_nested(pointer)
            .and_then(|value| value.as_object())
            .and_then(|object| object.delete(key))
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, item: &Json) -> Option<UpdateArray> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;
        root_value
            .get_nested(pointer)
            .and_then(|value| value.as_array())
            .and_then(|array| array.insert(index, raw::decode(item, replica), replica))
    }

    pub fn insert_item_str(&mut self, pointer: &str, index: usize, item: &str) -> Option<UpdateArray> {
        let json: Json = serde_json::de::from_str(item).expect("invalid JSON!");
        self.insert_item(pointer, index, &json)
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
