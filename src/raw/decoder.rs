use Replica;
use Value;
use array::Array;
use attributed_string::AttributedString;
use error::Error;
use object::Object;
use serde_json::Value as Json;
use serde_json::value::Map;

pub fn decode(json: &Json, replica: &Replica) -> Result<Value, Error> {
    match *json {
        Json::Object(ref map) =>
            decode_map(map, replica),
        Json::Array(ref vec) =>
            decode_array(vec, replica),
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
    }
}

fn decode_map(map: &Map<String,Json>, replica: &Replica) -> Result<Value, Error> {
    let special_type = map.get("__TYPE__").and_then(|json| json.as_str());
    match special_type {
        Some("attrstr") => decode_attributed_string(map, replica),
        _ => decode_object(map, replica),
    }
}

fn decode_object(map: &Map<String, Json>, replica: &Replica) -> Result<Value, Error> {
    let mut object = Object::new();
    for (key, encoded_value) in map {
        let key = key.replace("~1", "__TYPE__").replace("~0", "~");
        let value = decode(encoded_value, replica)?;
        object.put(&key, value, replica);
    }
    Ok(Value::Obj(object))
}

fn decode_array(vec: &Vec<Json>, replica: &Replica) -> Result<Value, Error> {
    let mut array = Array::new();
    for (i, encoded_value) in vec.iter().enumerate() {
        let value = decode(encoded_value, replica)?;
        let _ = array.insert(i, value, replica);
    }
    Ok(Value::Arr(array))
}

fn decode_attributed_string(map: &Map<String, Json>, replica: &Replica) -> Result<Value, Error> {
    let mut string = AttributedString::new();
    let text = map.get("text").and_then(|json| json.as_str()).ok_or(Error::InvalidJson)?;
    let _ = string.insert_text(0, text.to_string(), replica);
    Ok(Value::AttrStr(string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Replica;
    use serde_json;
    use Value;

    const REPLICA: Replica = Replica {
        site: 2,
        counter: 3,
    };

    #[test]
    fn test_decode_null() {
        assert!(decode_str("null", &REPLICA) == Value::Null);
    }

    #[test]
    fn test_decode_bool() {
        assert!(decode_str("true", &REPLICA) == Value::Bool(true));
        assert!(decode_str("false", &REPLICA) == Value::Bool(false));
    }

    #[test]
    fn test_decode_number() {
        assert!(decode_str("243", &REPLICA) == Value::Num(243.0));
        assert!(decode_str("243.4", &REPLICA) == Value::Num(243.4));
        assert!(decode_str("-243.4", &REPLICA) == Value::Num(-243.4));
    }

    #[test]
    fn test_decode_string() {
        assert!(decode_str("\"\"", &REPLICA) == Value::Str("".to_string()));
        assert!(decode_str("\"Hello world!\"", &REPLICA) == Value::Str("Hello world!".to_string()));
    }

    #[test]
    fn test_decode_attributed_string() {
        let string = r#"{"__TYPE__":"attrstr","text":"Hello world!"}"#;
        let mut value = decode_str(&string, &REPLICA);

        let attrstr = value.as_attributed_string().unwrap();
        assert!(attrstr.len() == 12);
        assert!(attrstr.to_string() == "Hello world!");
    }

    #[test]
    fn test_decode_array() {
        let mut value = decode_str(r#"[null, 1, "Hey!"]"#, &REPLICA);
        let mut array = value.as_array().unwrap();

        assert!(array.len() == 3);
        assert!(array.get_by_index(0).unwrap().value == Value::Null);
        assert!(array.get_by_index(1).unwrap().value == Value::Num(1.0));
        assert!(array.get_by_index(2).unwrap().value == Value::Str("Hey!".to_string()));
    }

    #[test]
    fn test_decode_object() {
        let string = r#"{"a":true, "~1":-3, "~0": false}"#;
        let mut value = decode_str(&string, &REPLICA);
        let mut object = value.as_object().unwrap();

        assert!(object.get_by_key("a").unwrap().value == Value::Bool(true));
        assert!(object.get_by_key("__TYPE__").unwrap().value == Value::Num(-3.0));
        assert!(object.get_by_key("~").unwrap().value == Value::Bool(false));
    }

    #[test]
    fn test_decode_nested() {
        let string = r#"{"foo":[1,true,null,"hm"], "bar":{"a":true}, "baz": {"__TYPE__":"attrstr","text":"Hello world!"}}"#;
        let mut value = decode_str(&string, &REPLICA);
        let mut object1 = value.as_object().unwrap();
        {
            let ref mut array = object1.get_by_key("foo").unwrap().value;
            assert!(array.as_array().is_ok());
        }
        {
            let ref mut object2 = object1.get_by_key("bar").unwrap().value;
            assert!(object2.as_object().is_ok());
        }
        {
            let ref mut attrstr = object1.get_by_key("baz").unwrap().value;
            assert!(attrstr.as_attributed_string().is_ok());
        }
    }

    fn decode_str(string: &str, replica: &Replica) -> Value {
        let json: serde_json::Value = serde_json::de::from_str(string).expect("invalid JSON!");
        decode(&json, replica).unwrap()
    }
}
