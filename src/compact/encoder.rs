use Value;
use array;
use attributed_string;
use object;
use serde_json::Value as Json;

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

#[cfg(test)]
mod tests {
    use super::*;
    use Value;
    use Replica;
    use array::Array;
    use attributed_string::AttributedString;
    use object::Object;
    use serde_json;

    #[test]
    fn test_encode_null() {
        assert!(encode_str(&Value::Null) == "null");
    }

    #[test]
    fn test_encode_bool() {
        assert!(encode_str(&Value::Bool(true)) == "true");
        assert!(encode_str(&Value::Bool(false)) == "false");
    }

    #[test]
    fn test_encode_number() {
        assert!(encode_str(&Value::Num(304.3)) == "304.3");
    }

    #[test]
    fn test_encode_string() {
        assert!(encode_str(&Value::Str("hi".to_string())) == r#""hi""#);
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::ser::to_string(&json).ok().unwrap()
    }

}
