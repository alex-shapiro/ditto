pub mod decoder;
pub mod encoder;

pub use self::decoder::decode;
pub use self::encoder::{encode, encode_op};
use Error;
use Replica;
use serde::{Serialize, Serializer};
use serde_json;
use Value;

pub struct LocalValue(Value);

impl LocalValue {
    pub fn new(value: Value) -> Self {
        LocalValue(value)
    }

    pub fn from_str(string: &str, replica: &Replica) -> Result<Self, Error> {
        let json: serde_json::Value = serde_json::from_str(string)?;
        let value = decode(&json, replica)?;
        Ok(LocalValue(value))
    }

    pub fn value(self) -> Value {
        self.0
    }
}

impl Serialize for LocalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_some(&encode(&self.0))
    }
}
