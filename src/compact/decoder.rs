use array::Array;
use array::element::Element as ArrayElement;
use attributed_string::AttributedString;
use attributed_string::element::Element as StringElement;
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

pub fn decode(json: &Json) -> Result<Value, Error> {
    match *json {
        Json::Array(ref vec) =>
            decode_json_array(vec),
        Json::String(ref string) =>
            Ok(Value::Str(string.to_string())),
        Json::F64(number) =>
            Ok(Value::Num(number)),
        Json::U64(number) =>
            Ok(Value::Num(number as f64)),
        Json::I64(number) =>
            Ok(Value::Num(number as f64)),
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
    let mut elements: Vec<StringElement> = Vec::with_capacity(encoded_elements.len() + 2);
    let mut len = 0;

    elements.push(StringElement::start_marker());
    for json in encoded_elements {
        let element = try!(decode_attributed_string_element(json));
        len += element.len();
        elements.push(element);
    }
    elements.push(StringElement::end_marker());
    let string = AttributedString::assemble(elements, len);
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
fn decode_attributed_string_element(element: &Json) -> Result<StringElement, Error> {
    let element_vec = try!(element.as_array().ok_or(Error::DecodeCompact));
    if element_vec.len() != 2 { return Err(Error::DecodeCompact) }

    let encoded_uid = try!(element_vec[0].as_str().ok_or(Error::DecodeCompact));
    let text        = try!(element_vec[1].as_str().ok_or(Error::DecodeCompact));
    let uid         = try!(SequenceUID::from_str(encoded_uid));
    Ok(StringElement::new_text(text.to_string(), uid))
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
// decode [4,pointer,[ArrayElement],[SequenceUID]] as nested UpdateArray
fn decode_op_update_array(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 4 { return Err(Error::DecodeCompact) }

    // decode inserts
    let encoded_inserts = try!(as_array(&op_vec[2]));
    let mut inserts = Vec::with_capacity(encoded_inserts.len());
    for encoded_element in encoded_inserts {
         let element = try!(decode_array_element(encoded_element));
         inserts.push(element);
    }

    // decode deletes
    let encoded_deletes = try!(as_array(&op_vec[3]));
    let mut deletes = Vec::with_capacity(encoded_deletes.len());
    for encoded_uid in encoded_deletes {
        let uid_str = try!(as_str(encoded_uid));
        let uid = try!(SequenceUID::from_str(uid_str));
        deletes.push(uid);
    }

    let op = UpdateArray{inserts: inserts, deletes: deletes, deleted_elements: vec![]};
    Ok(RemoteOp::UpdateArray(op))
}

#[inline]
// decode [5,pointer,[AttrStrElement],[SequenceUID]]  UpdateAttributedString
fn decode_op_update_attributed_string(op_vec: &Vec<Json>) -> Result<RemoteOp, Error> {
    if op_vec.len() != 4 { return Err(Error::DecodeCompact) }

    // decode inserts
    let encoded_inserts = try!(as_array(&op_vec[2]));
    let mut inserts = Vec::with_capacity(encoded_inserts.len());
    for encoded_element in encoded_inserts {
         let element = try!(decode_attributed_string_element(encoded_element));
         inserts.push(element);
    }

    // decode deletes
    let encoded_deletes = try!(as_array(&op_vec[3]));
    let mut deletes = Vec::with_capacity(encoded_deletes.len());
    for encoded_uid in encoded_deletes {
        let uid_str = try!(as_str(encoded_uid));
        let uid = try!(SequenceUID::from_str(uid_str));
        deletes.push(uid);
    }

    let op = UpdateAttributedString{inserts: inserts, deletes: deletes, deleted_elements: vec![]};
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
