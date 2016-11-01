use array;
use attributed_string;
use object;
use op::{NestedRemoteOp, RemoteOp};
use op::remote::{UpdateObject, UpdateArray, UpdateAttributedString, IncrementNumber};
use serde_json::Value as Json;
use Value;

pub fn encode(value: &Value) -> Json {
    match *value {
        Value::Obj(ref object) =>
            encode_object(object),
        Value::Arr(ref array) =>
            encode_array(array),
        Value::AttrStr(ref string) =>
            encode_attributed_string(string),
        Value::Str(ref string) =>
            Json::String(string.to_string()),
        Value::Num(number) =>
            Json::F64(number),
        Value::Bool(bool_value) =>
            Json::Bool(bool_value),
        Value::Null =>
            Json::Null,
    }
}

pub fn encode_op(nested_op: &NestedRemoteOp) -> Json {
    let pointer = nested_op.pointer.clone();
    let operation = &nested_op.op;
    match *operation {
        RemoteOp::UpdateObject(ref op) =>
            encode_op_update_object(op, pointer),
        RemoteOp::UpdateArray(ref op) =>
            encode_op_update_array(op, pointer),
        RemoteOp::UpdateAttributedString(ref op) =>
            encode_op_update_attributed_string(op, pointer),
        RemoteOp::IncrementNumber(ref op) =>
            encode_op_increment_number(op, pointer),
    }
}

#[inline]
// Encode AttributedString as [0,[Element]]
fn encode_attributed_string(string: &attributed_string::AttributedString) -> Json {
    let mut elements: Vec<Json> = Vec::new();
    for element in string.elements() {
        elements.push(encode_attributed_string_element(element));
    }
    Json::Array(vec![Json::U64(0), Json::Array(elements)])
}

#[inline]
// Encode Array as [1,[Element]]
fn encode_array(array: &array::Array) -> Json {
    let mut elements: Vec<Json> = Vec::with_capacity(array.len());
    for element in array.elements() {
        elements.push(encode_array_element(&element))
    }
    Json::Array(vec![Json::U64(1), Json::Array(elements)])
}

#[inline]
// encode Object as [2,[Element]]
fn encode_object(object: &object::Object) -> Json {
    let mut elements: Vec<Json> = Vec::new();
    for (_, key_elements) in object.elements() {
        for element in key_elements {
            elements.push(encode_object_element(&element))
        }
    }
    Json::Array(vec![Json::U64(2), Json::Array(elements)])
}

#[inline]
// encode AttributedString element as [SequenceUID,text]
fn encode_attributed_string_element(element: &attributed_string::element::Element) -> Json {
    let mut element_vec: Vec<Json> = Vec::with_capacity(2);
    element_vec.push(Json::String(element.uid.to_string()));
    element.text().and_then(|text| Some(element_vec.push(Json::String(text.to_string()))));
    Json::Array(element_vec)
}

#[inline]
// encode Array element as [SequenceUID,Value]
fn encode_array_element(element: &array::element::Element) -> Json {
    let mut element_vec: Vec<Json> = Vec::with_capacity(2);
    element_vec.push(Json::String(element.uid.to_string()));
    element_vec.push(encode(&element.value));
    Json::Array(element_vec)
}

#[inline]
// encode Object element as [ObjectUID,Value]
fn encode_object_element(element: &object::element::Element) -> Json {
    let mut element_vec: Vec<Json> = Vec::with_capacity(2);
    element_vec.push(Json::String(element.uid.to_string()));
    element_vec.push(encode(&element.value));
    Json::Array(element_vec)
}

// encode UpdateObject op as [3,pointer,key,[ObjectElement],[ObjectElement]]
fn encode_op_update_object(op: &UpdateObject, pointer: String) -> Json {
    let mut op_vec: Vec<Json> = Vec::with_capacity(5);
    let mut inserts_vec: Vec<Json> = Vec::with_capacity(op.inserts.len());
    let mut deletes_vec: Vec<Json> = Vec::with_capacity(op.deletes.len());

    for insert in &op.inserts {
        let encoded_insert = encode_object_element(insert);
        inserts_vec.push(encoded_insert);
    }
    for delete in &op.deletes {
        let encoded_delete = encode_object_element(delete);
        deletes_vec.push(encoded_delete);
    }

    op_vec.push(Json::U64(3));
    op_vec.push(Json::String(pointer));
    op_vec.push(Json::String(op.key.to_string()));
    op_vec.push(Json::Array(inserts_vec));
    op_vec.push(Json::Array(deletes_vec));
    Json::Array(op_vec)
}

// encode UpdateArray op as [4,pointer,[ArrayElement],[SequenceUID]]
fn encode_op_update_array(op: &UpdateArray, pointer: String) -> Json {
    let mut op_vec: Vec<Json> = Vec::with_capacity(4);
    let mut inserts_vec: Vec<Json> = Vec::with_capacity(op.inserts.len());
    let mut deletes_vec: Vec<Json> = Vec::with_capacity(op.deletes.len());

    for elt in &op.inserts {
        inserts_vec.push(encode_array_element(&elt));
    }
    for uid in &op.deletes {
        deletes_vec.push(Json::String(uid.to_string()));
    }

    op_vec.push(Json::U64(4));
    op_vec.push(Json::String(pointer));
    op_vec.push(Json::Array(inserts_vec));
    op_vec.push(Json::Array(deletes_vec));
    Json::Array(op_vec)
}

// encode UpdateAttributedString as [5,pointer,[AttrStrElement],[SequenceUID]]
fn encode_op_update_attributed_string(op: &UpdateAttributedString, pointer: String) -> Json {
    let mut op_vec: Vec<Json> = Vec::with_capacity(4);
    let mut inserts_vec: Vec<Json> = Vec::with_capacity(op.inserts.len());
    let mut deletes_vec: Vec<Json> = Vec::with_capacity(op.deletes.len());

    for elt in &op.inserts {
        inserts_vec.push(encode_attributed_string_element(&elt));
    }
    for uid in &op.deletes {
        deletes_vec.push(Json::String(uid.to_string()));
    }

    op_vec.push(Json::U64(5));
    op_vec.push(Json::String(pointer));
    op_vec.push(Json::Array(inserts_vec));
    op_vec.push(Json::Array(deletes_vec));
    Json::Array(op_vec)
}

// encode IncrementNumber as [6,pointer,amount]
fn encode_op_increment_number(op: &IncrementNumber, pointer: String) -> Json {
    Json::Array(vec![Json::U64(6), Json::String(pointer), Json::F64(op.amount)])
}
