use array::Array;
use array::element::Element as ArrayElement;
use attributed_string::AttributedString;
use attributed_string::element::Element as StringElement;
use error::Error;
use object::element::Element as ObjectElement;
use object::Object;
use object::uid::UID as ObjectUID;
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
