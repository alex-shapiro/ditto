use Value;
use array::Array;
use attributed_string::AttributedString;
use object::Object;
use serde_json::Value as Json;
use serde_json::value::Map as SerdeMap;

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

fn encode_object(object: &Object) -> Json {
    let mut map: SerdeMap<String, Json> = SerdeMap::new();
    for (key, elements) in object.elements() {
        let encoded_key = key.replace("~","~0").replace("__TYPE__","~1");
        let ref value = elements.iter().min_by_key(|e| e.uid.site).unwrap().value;
        map.insert(encoded_key, encode(value));
    }
    Json::Object(map)
}

fn encode_array(array: &Array) -> Json {
    let vec: Vec<Json> =
        array
            .elements()
            .iter()
            .map(|e| &e.value)
            .map(|value| encode(value))
            .collect();
    Json::Array(vec)
}

fn encode_attributed_string(string: &AttributedString) -> Json {
    let mut map: SerdeMap<String, Json> = SerdeMap::new();
    map.insert("__TYPE__".to_string(), Json::String("attrstr".to_string()));
    map.insert("text".to_string(), Json::String(string.raw_string()));
    Json::Object(map)
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

    const REPLICA: Replica = Replica{site: 4, counter: 103};

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

    #[test]
    fn test_encode_attributed_string() {
        let mut attrstr = AttributedString::new();
        attrstr.insert_text(0, "the ".to_string(), &REPLICA);
        attrstr.insert_text(4, "quick ".to_string(), &REPLICA);
        attrstr.insert_text(10, "brown ".to_string(), &REPLICA);
        attrstr.insert_text(16, "fox".to_string(), &REPLICA);
        let value = Value::AttrStr(attrstr);
        assert!(encode_str(&value) == r#"{"__TYPE__":"attrstr","text":"the quick brown fox"}"#);
    }

    #[test]
    fn test_encode_array() {
        let mut array = Array::new();
        array.insert(0, Value::Num(1.0), &REPLICA);
        array.insert(1, Value::Bool(true), &REPLICA);
        array.insert(2, Value::Str("hey".to_string()), &REPLICA);
        let value = Value::Arr(array);
        assert!(encode_str(&value) == r#"[1.0,true,"hey"]"#);
    }

    #[test]
    fn test_encode_object() {
        let mut object = Object::new();
        object.put("a", Value::Num(1.0), &REPLICA);
        object.put("__TYPE__", Value::Null, &REPLICA);
        object.put("~cookies~", Value::Bool(true), &REPLICA);
        let value = Value::Obj(object);
        let json = encode_str(&value);
        assert!(json.contains(r#""a":1.0"#));
        assert!(json.contains(r#""~0cookies~0":true"#));
        assert!(json.contains(r#""~1":null"#));
    }

    #[test]
    fn test_encode_nested() {
        let mut array = Array::new();
        array.insert(0, Value::object(), &REPLICA);
        array.insert(1, Value::attrstr(), &REPLICA);
        array.insert(2, Value::array(), &REPLICA);
        array.insert(3, Value::Str("hi!".to_string()), &REPLICA);
        array.insert(4, Value::Num(-3234.1), &REPLICA);
        array.insert(5, Value::Bool(true), &REPLICA);
        array.insert(6, Value::Null, &REPLICA);
        let value = Value::Arr(array);
        let json = encode_str(&value);
        assert!(json == r#"[{},{"__TYPE__":"attrstr","text":""},[],"hi!",-3234.1,true,null]"#);
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::ser::to_string(&json).ok().unwrap()
    }
}
