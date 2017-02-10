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
            json!(string),
        Value::Num(number) =>
            json!(number),
        Value::Bool(bool_value) =>
            json!(bool_value),
        Value::Null =>
            json!(null),
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
    let elements: Vec<Json> = string.elements().map(|e| encode_attributed_string_element(e)).collect();
    json!([0, elements])
}

#[inline]
// Encode Array as [1,[Element]]
fn encode_array(array: &array::Array) -> Json {
    let elements: Vec<Json> = array.elements().iter().map(|e| encode_array_element(e)).collect();
    json!([1, elements])
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
    json!([2, elements])
}

#[inline]
// encode AttributedString element as [SequenceUID,text]
fn encode_attributed_string_element(element: &attributed_string::element::Element) -> Json {
    json!([element.uid.to_string(), element.text.clone()])
}

#[inline]
// encode Array element as [SequenceUID,Value]
fn encode_array_element(element: &array::element::Element) -> Json {
    json!([element.uid.to_string(), encode(&element.value)])
}

#[inline]
// encode Object element as [ObjectUID,Value]
fn encode_object_element(element: &object::element::Element) -> Json {
    json!([element.uid.to_string(), encode(&element.value)])
}

// encode UpdateObject op as [3,pointer,key,[ObjectElement],[ObjectElement]]
fn encode_op_update_object(op: &UpdateObject, pointer: String) -> Json {
    let inserts_vec: Vec<Json> = op.inserts.iter().map(|i| encode_object_element(i)).collect();
    let deletes_vec: Vec<Json> = op.deletes.iter().map(|d| encode_object_element(d)).collect();
    json!([3, pointer, op.key.to_string(), inserts_vec, deletes_vec])
}

// encode UpdateArray op as [4,pointer,[ArrayElement],[ArrayElement]]
fn encode_op_update_array(op: &UpdateArray, pointer: String) -> Json {
    let inserts_vec: Vec<Json> = op.inserts.iter().map(|i| encode_array_element(i)).collect();
    let deletes_vec: Vec<Json> = op.deletes.iter().map(|d| encode_array_element(d)).collect();
    json!([4, pointer, inserts_vec, deletes_vec])
}

// encode UpdateAttributedString as [5,pointer,[AttrStrElement],[AttrStrElement]]
fn encode_op_update_attributed_string(op: &UpdateAttributedString, pointer: String) -> Json {
    let inserts_vec: Vec<Json> = op.inserts.iter().map(|i| encode_attributed_string_element(i)).collect();
    let deletes_vec: Vec<Json> = op.deletes.iter().map(|d| encode_attributed_string_element(d)).collect();
    json!([5, pointer, inserts_vec, deletes_vec])
}

// encode IncrementNumber as [6,pointer,amount]
fn encode_op_increment_number(op: &IncrementNumber, pointer: String) -> Json {
    json!([6, pointer, op.amount])
}
