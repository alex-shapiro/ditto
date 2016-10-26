use compact;
use Error;
use op::local::LocalOp;
use op::NestedRemoteOp;
use op::remote::{UpdateObject,UpdateArray,UpdateAttributedString,IncrementNumber};
use raw;
use Replica;
use serde_json::Value as Json;
use serde_json;
use Value;

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

    pub fn new_str(string: &str, site: u32) -> Self {
        let json: Json = serde_json::de::from_str(string).expect("invalid JSON!");
        CRDT::new(&json, site)
    }

    pub fn serialize(&self) -> Json {
        compact::encode(&self.root_value)
    }

    pub fn deserialize(json: &Json) -> Result<Self, Error> {
        let replica = Replica::new(1, 0);
        let value = try!(compact::decode(json));
        Ok(CRDT{root_value: value, replica: replica})
    }

    pub fn get(&mut self, pointer: &str) -> Option<Json> {
        let value = self.root_value.get_nested(pointer).ok();
        value.and_then(|value| Some(raw::encode(value)))
    }

    pub fn get_str(&mut self, pointer: &str) -> Option<String> {
        self.get(pointer).and_then(|json| {
            Some(serde_json::ser::to_string(&json).ok().unwrap())
        })
    }

    pub fn put(&mut self, pointer: &str, key: &str, value: &Json) -> Result<UpdateObject, Error> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let mut object_value = try!(root_value.get_nested(pointer));
        let mut object = try!(object_value.as_object());
        Ok(object.put(key, raw::decode(value, replica), replica))
    }

    pub fn put_str(&mut self, pointer: &str, key: &str, item: &str) -> Result<UpdateObject, Error> {
        let json: Json = serde_json::de::from_str(item).expect("invalid JSON!");
        self.put(pointer, key, &json)
    }

    pub fn delete(&mut self, pointer: &str, key: &str) -> Result<UpdateObject, Error> {
        let mut object_value = try!(self.root_value.get_nested(pointer));
        let mut object = try!(object_value.as_object());
        object.delete(key)
    }

    pub fn insert_item(&mut self, pointer: &str, index: usize, item: &Json) -> Result<UpdateArray, Error> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let mut array_value = try!(root_value.get_nested(pointer));
        let mut array = try!(array_value.as_array());
        array.insert(index, raw::decode(item, replica), replica)
    }

    pub fn insert_item_str(&mut self, pointer: &str, index: usize, item: &str) -> Result<UpdateArray, Error> {
        let json: Json = serde_json::de::from_str(item).expect("invalid JSON!");
        self.insert_item(pointer, index, &json)
    }

    pub fn delete_item(&mut self, pointer: &str, index: usize) -> Result<UpdateArray, Error> {
        let mut array_value = try!(self.root_value.get_nested(pointer));
        let mut array = try!(array_value.as_array());
        array.delete(index)
    }

    pub fn insert_text(&mut self, pointer: &str, index: usize, text: String) -> Result<UpdateAttributedString, Error> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let mut attrstr_value = try!(root_value.get_nested(pointer));
        let mut attrstr = try!(attrstr_value.as_attributed_string());
        attrstr.insert_text(index, text, replica)
    }

    pub fn delete_text(&mut self, pointer: &str, index: usize, len: usize) -> Result<UpdateAttributedString, Error> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let mut attrstr_value = try!(root_value.get_nested(pointer));
        let mut attrstr = try!(attrstr_value.as_attributed_string());
        attrstr.delete_text(index, len, replica)
    }

    pub fn replace_text(&mut self, pointer: &str, index: usize, len: usize, text: String) -> Result<UpdateAttributedString, Error> {
        let root_value = &mut self.root_value;
        let replica = &self.replica;

        let mut attrstr_value = try!(root_value.get_nested(pointer));
        let mut attrstr = try!(attrstr_value.as_attributed_string());
        attrstr.replace_text(index, len, text, replica)
    }

    pub fn increment(&mut self, pointer: &str, amount: f64) -> Result<IncrementNumber, Error> {
        let mut number_value = try!(self.root_value.get_nested(pointer));
        number_value.increment(amount)
    }

    pub fn execute_remote(&mut self, nested_op: &NestedRemoteOp) -> Result<Vec<LocalOp>, Error> {
        let ref pointer = nested_op.pointer;
        let mut value = try!(self.root_value.get_nested(pointer));
        value.execute_remote(&nested_op.op)
    }
}
