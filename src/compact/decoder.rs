use array::Array;
use array::element::Element as ArrayElement;
use attributed_string::AttributedString;
use attributed_string::element::Element as AttrStrElement;
use Error;
use object::element::Element as ObjectElement;
use object::Object;
use object::uid::UID as ObjectUID;
use op::{NestedRemoteOp, RemoteOp};
use op::remote::{UpdateObject,UpdateArray,UpdateAttributedString,IncrementNumber};
use sequence::uid::UID as SequenceUID;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::str::FromStr;
use Value;

use serde::{Deserialize, Deserializer};
use serde::de::{self, Visitor, SeqVisitor};
use std::fmt;

pub fn decode(json: &Json) -> Result<Value, Error> {
    match *json {
        Json::Array(ref vec) =>
            decode_json_array(vec),
        Json::String(ref string) =>
            Ok(Value::Str(string.to_string())),
        Json::Number(ref number) =>
            Ok(Value::Num(number.as_f64().expect("Must decode as valid f64!"))),
        Json::Bool(bool_value) =>
            Ok(Value::Bool(bool_value)),
        Json::Null =>
            Ok(Value::Null),
        Json::Object(_) =>
            Err(Error::DecodeCompact),
    }
}

pub fn decode_op(json: &Json) -> Result<NestedRemoteOp, Error> {
    let op_array = try!(json.as_array().ok_or(Error::DecodeCompact));
    if op_array.len() < 2 { return Err(Error::DecodeCompact) }

    let op_type = try!(op_array[0].as_u64().ok_or(Error::DecodeCompact));
    let pointer = try!(op_array[1].as_str().ok_or(Error::DecodeCompact));

    let op = try!(match op_type {
        3 => decode_op_update_object(op_array),
        4 => decode_op_update_array(op_array),
        5 => decode_op_update_attributed_string(op_array),
        6 => decode_op_increment_number(op_array),
        _ => return Err(Error::DecodeCompact),
    });

    Ok(NestedRemoteOp{pointer: pointer.to_owned(), op: op})
}

#[inline]
fn decode_json_array(vec: &[Json]) -> Result<Value, Error> {
    if vec.len() != 2 { return Err(Error::DecodeCompact) }

    let data_type = try!(vec[0].as_u64().ok_or(Error::DecodeCompact));
    let ref data_elements = try!(vec[1].as_array().ok_or(Error::DecodeCompact));

    match data_type {
        0 => decode_attributed_string(data_elements),
        1 => decode_array(data_elements),
        2 => decode_object(data_elements),
        _ => Err(Error::DecodeCompact)
    }
}

#[inline]
fn decode_attributed_string(encoded_elements: &[Json]) -> Result<Value, Error> {
    let elements: Vec<AttrStrElement> =
        try!(encoded_elements
            .into_iter()
            .map(|json| decode_attributed_string_element(json))
            .collect());

    let string = AttributedString::assemble(elements);
    Ok(Value::AttrStr(string))
}

#[inline]
fn decode_array(encoded_elements: &[Json]) -> Result<Value, Error> {
    let mut elements: Vec<ArrayElement> = Vec::with_capacity(encoded_elements.len() + 2);

    elements.push(ArrayElement::start_marker());
    for json in encoded_elements {
        let element = try!(decode_array_element(json));
        elements.push(element);
    }
    elements.push(ArrayElement::end_marker());
    let array = Array::assemble(elements);
    Ok(Value::Arr(array))
}

#[inline]
fn decode_object(encoded_elements: &[Json]) -> Result<Value, Error> {
    let mut map: HashMap<String,Vec<ObjectElement>> = HashMap::new();

    for json in encoded_elements {
        let element = try!(decode_object_element(json));
        let key = element.uid.key.clone();
        map.entry(key).or_insert(vec![]).push(element);
    }

    let object = Object::assemble(map);
    Ok(Value::Obj(object))
}

#[inline]
fn decode_attributed_string_element(element: &Json) -> Result<AttrStrElement, Error> {
    let element_vec = try!(element.as_array().ok_or(Error::DecodeCompact));
    if element_vec.len() != 2 { return Err(Error::DecodeCompact) }

    let encoded_uid = try!(element_vec[0].as_str().ok_or(Error::DecodeCompact));
    let text        = try!(element_vec[1].as_str().ok_or(Error::DecodeCompact));
    let uid         = try!(SequenceUID::from_str(encoded_uid));
    Ok(AttrStrElement::text(text.to_string(), uid))
}

#[inline]
fn decode_array_element(element: &Json) -> Result<ArrayElement, Error> {
    let element_vec = try!(element.as_array().ok_or(Error::DecodeCompact));
    if element_vec.len() != 2 { return Err(Error::DecodeCompact) }

    let encoded_uid = try!(element_vec[0].as_str().ok_or(Error::DecodeCompact));
    let item        = try!(decode(&element_vec[1]));
    let uid         = try!(SequenceUID::from_str(encoded_uid));
    Ok(ArrayElement::new(item, uid))
}

#[inline]
fn decode_object_element(element: &Json) -> Result<ObjectElement, Error> {
    let element_vec = try!(element.as_array().ok_or(Error::DecodeCompact));
    if element_vec.len() != 2 { return Err(Error::DecodeCompact) }

    let encoded_uid = try!(element_vec[0].as_str().ok_or(Error::DecodeCompact));
    let value       = try!(decode(&element_vec[1]));
    let uid         = try!(ObjectUID::from_str(encoded_uid));
    Ok(ObjectElement{uid: uid, value: value})
}

#[inline]
// decode [3,pointer,key,[ObjectElement],[ObjectElement]] as UpdateObject
fn decode_op_update_object(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 5 { return Err(Error::DecodeCompact) }

    let key     = try!(as_str(&op_vec[2])).to_owned();
    let inserts = try!(decode_object_elements(&op_vec[3]));
    let deletes = try!(decode_object_elements(&op_vec[4]));
    let op      = UpdateObject{key: key, inserts: inserts, deletes: deletes};
    Ok(RemoteOp::UpdateObject(op))
}

#[inline]
// decode [4,pointer,[ArrayElement],[ArrayElement]] as nested UpdateArray
fn decode_op_update_array(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 4 { return Err(Error::DecodeCompact) }

    let inserts = try!(decode_array_elements(&op_vec[2]));
    let deletes = try!(decode_array_elements(&op_vec[3]));
    let op      = UpdateArray{inserts: inserts, deletes: deletes};
    Ok(RemoteOp::UpdateArray(op))
}

#[inline]
// decode [5,pointer,[AttrStrElement],[SequenceUID]] as UpdateAttributedString
fn decode_op_update_attributed_string(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 4 { return Err(Error::DecodeCompact) }

    let inserts = try!(decode_attrstr_elements(&op_vec[2]));
    let deletes = try!(decode_attrstr_elements(&op_vec[3]));
    let op = UpdateAttributedString{inserts: inserts, deletes: deletes};
    Ok(RemoteOp::UpdateAttributedString(op))
}

#[inline]
// decode [6,pointer,amount] as IncrementNumber
fn decode_op_increment_number(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 3 { return Err(Error::DecodeCompact) }
    let amount = try!(op_vec[2].as_f64().ok_or(Error::DecodeCompact));
    let op = IncrementNumber{amount: amount};
    Ok(RemoteOp::IncrementNumber(op))
}

#[inline]
fn as_str(json: &Json) -> Result<&str, Error> {
    json.as_str().ok_or(Error::DecodeCompact)
}

#[inline]
fn as_array(json: &Json) -> Result<&Vec<Json>, Error> {
    json.as_array().ok_or(Error::DecodeCompact)
}

#[inline]
fn decode_object_elements(encoded_elements_json: &Json) -> Result<Vec<ObjectElement>, Error> {
    let encoded_elements = try!(as_array(encoded_elements_json));
    let mut elements = Vec::with_capacity(encoded_elements.len());
    for encoded_element in encoded_elements {
        let element = try!(decode_object_element(&encoded_element));
        elements.push(element);
    }
    Ok(elements)
}

#[inline]
fn decode_array_elements(encoded_elements_json: &Json) -> Result<Vec<ArrayElement>, Error> {
    let encoded_elements = try!(as_array(encoded_elements_json));
    let mut elements = Vec::with_capacity(encoded_elements.len());
    for encoded_element in encoded_elements {
        let element = try!(decode_array_element(&encoded_element));
        elements.push(element);
    }
    Ok(elements)
}

#[inline]
fn decode_attrstr_elements(encoded_elements_json: &Json) -> Result<Vec<AttrStrElement>, Error> {
    let encoded_elements = try!(as_array(encoded_elements_json));
    let mut elements = Vec::with_capacity(encoded_elements.len());
    for encoded_element in encoded_elements {
        let element = try!(decode_attributed_string_element(&encoded_element));
        elements.push(element);
    }
    Ok(elements)
}

impl Deserialize for Value {
    fn deserialize<D>(deserializer: D) -> Result<Value, D::Error> where D: Deserializer {
        struct ValueVisitor;

        impl Visitor for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid encoded CRDT value")
            }

            fn visit_unit<E>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
                Ok(Value::Num(value as f64))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
                Ok(Value::Num(value as f64))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
                Ok(Value::Num(value as f64))
            }

            fn visit_str<E>(self, value: &str) -> Result<Value, E> where E: de::Error {
                self.visit_string(String::from(value))
            }

            fn visit_string<E>(self, value: String) -> Result<Value, E> {
                Ok(Value::Str(value))
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Value, V::Error> where V: SeqVisitor {
                let code: u8 = visitor.visit()?.ok_or(de::Error::missing_field("opcode"))?;
                match code {
                    0 => {
                        let elements: Vec<AttrStrElement> = visitor.visit()?.ok_or(de::Error::missing_field("AttrStr elements"))?;
                        let attrstr = AttributedString::assemble(elements);
                        Ok(Value::AttrStr(attrstr))
                    },
                    1 => {
                        let mut elements: Vec<ArrayElement> = visitor.visit()?.ok_or(de::Error::missing_field("Array elements"))?;
                        elements.insert(0, ArrayElement::start_marker());
                        elements.push(ArrayElement::end_marker());
                        let array = Array::assemble(elements);
                        Ok(Value::Arr(array))
                    },
                    2 => {
                        let mut map: HashMap<String,Vec<ObjectElement>> = HashMap::new();
                        let elements: Vec<ObjectElement> = visitor.visit()?.ok_or(de::Error::missing_field("Object elements"))?;

                        for element in elements {
                            let key = element.uid.key.clone();
                            map.entry(key).or_insert(vec![]).push(element);
                        }

                        let object = Object::assemble(map);
                        Ok(Value::Obj(object))
                    }
                    _ => return Err(de::Error::missing_field("invalid Value code")),
                }
            }
        }

        deserializer.deserialize(ValueVisitor)
    }
}

impl Deserialize for AttrStrElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer {
        struct AttrStrElementVisitor;

        impl Visitor for AttrStrElementVisitor {
            type Value = AttrStrElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid AttrStrElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqVisitor {
                let encoded_uid: String = visitor.visit()?.ok_or(de::Error::missing_field("AttrStrElement uid"))?;
                let text: String = visitor.visit()?.ok_or(de::Error::missing_field("AttrStrElement text"))?;
                let uid = SequenceUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("AttrStrElement uid"))?;
                Ok(AttrStrElement::text(text.to_string(), uid))
            }
        }

        deserializer.deserialize_seq(AttrStrElementVisitor)
    }
}

impl Deserialize for ArrayElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer {
        struct ArrayElementVisitor;

        impl Visitor for ArrayElementVisitor {
            type Value = ArrayElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid ArrayElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqVisitor {
                let encoded_uid: String = visitor.visit()?.ok_or(de::Error::missing_field("ArrayElement uid"))?;
                let value: ::Value = visitor.visit()?.ok_or(de::Error::missing_field("ArrayElement value"))?;
                let uid = SequenceUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("ArrayElement uid"))?;
                Ok(ArrayElement::new(value, uid))
            }
        }

        deserializer.deserialize_seq(ArrayElementVisitor)
    }
}

impl Deserialize for ObjectElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer {
        struct ObjectElementVisitor;

        impl Visitor for ObjectElementVisitor {
            type Value = ObjectElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid ObjectElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqVisitor {
                let encoded_uid: String = visitor.visit()?.ok_or(de::Error::missing_field("ObjectElement uid"))?;
                let value: ::Value = visitor.visit()?.ok_or(de::Error::missing_field("ObjectElement value"))?;
                let uid = ObjectUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("ObjectElement uid"))?;
                Ok(ObjectElement{uid: uid, value: value})
            }
        }

        deserializer.deserialize_seq(ObjectElementVisitor)
    }
}
