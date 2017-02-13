use array;
use attributed_string;
use object;
use op::{NestedRemoteOp, RemoteOp};
use serde::{Serialize, Serializer};
use serde::ser::SerializeSeq;
// use serde::ser::SerializeMap;
use Value;

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match *self {
            Value::Obj(ref object) =>
                serializer.serialize_some(object),
            Value::Arr(ref array) =>
                serializer.serialize_some(array),
            Value::AttrStr(ref attrstr) =>
                serializer.serialize_some(attrstr),
            Value::Str(ref string) =>
                serializer.serialize_str(string),
            Value::Num(number) =>
                serializer.serialize_f64(number),
            Value::Bool(bool_value) =>
                serializer.serialize_bool(bool_value),
            Value::Null =>
                serializer.serialize_unit(),
        }
    }
}

impl Serialize for NestedRemoteOp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(None)?;
        match self.op {
            // encode UpdateObject op as [3,pointer,key,[ObjectElement],[ObjectElement]]
            RemoteOp::UpdateObject(ref op) => {
                seq.serialize_element(&3)?;
                seq.serialize_element(&self.pointer)?;
                seq.serialize_element(&op.key)?;
                seq.serialize_element(&op.inserts)?;
                seq.serialize_element(&op.deletes)?;
            },
            // encode UpdateArray op as [4,pointer,[ArrayElement],[ArrayElement]]
            RemoteOp::UpdateArray(ref op) => {
                seq.serialize_element(&4)?;
                seq.serialize_element(&self.pointer)?;
                seq.serialize_element(&op.inserts)?;
                seq.serialize_element(&op.deletes)?;
            },
            // encode UpdateAttributedString as [5,pointer,[AttrStrElement],[AttrStrElement]]
            RemoteOp::UpdateAttributedString(ref op) => {
                seq.serialize_element(&5)?;
                seq.serialize_element(&self.pointer)?;
                seq.serialize_element(&op.inserts)?;
                seq.serialize_element(&op.deletes)?;
            },
            // encode IncrementNumber as [6,pointer,amount]
            RemoteOp::IncrementNumber(ref op) => {
                seq.serialize_element(&6)?;
                seq.serialize_element(&self.pointer)?;
                seq.serialize_element(&op.amount)?;
            },
        }
        seq.end()
    }
}

// Encode AttributedString as [0,[Element]]
impl Serialize for attributed_string::AttributedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        let elements: Vec<&attributed_string::element::Element> = self.elements().collect();
        seq.serialize_element(&0)?;
        seq.serialize_element(&elements)?;
        seq.end()
    }
}

// Encode Array as [1,[Element]]
impl Serialize for array::Array {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&1)?;
        seq.serialize_element(&self.elements())?;
        seq.end()
    }
}

// encode Object as [2,[Element]]
impl Serialize for object::Object {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&2)?;
        seq.serialize_element(&self.elements_vec())?;
        seq.end()
    }
}

// encode AttributedString element as [SequenceUID,text]
impl Serialize for attributed_string::element::Element {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.uid.to_string())?;
        seq.serialize_element(&self.text)?;
        seq.end()
    }
}

// encode Array element as [SequenceUID, Value]
impl Serialize for array::element::Element {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.uid.to_string())?;
        seq.serialize_element(&self.value)?;
        seq.end()
    }
}

// encode Object element as [ObjectUID, Value]
impl Serialize for object::element::Element {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.uid.to_string())?;
        seq.serialize_element(&self.value)?;
        seq.end()
    }
}
