use serde_json;
use serde_json::value::Map;
use Replica;
use Value;
use attributed_string::AttributedString;
use object::Object;
use array::Array;

pub fn deserialize_str(str: &str, replica: &Replica) -> Value {
    let json: serde_json::value::Value = serde_json::de::from_str(str).expect("invalid JSON!");
    deserialize(&json, replica)
}

pub fn deserialize(json: &serde_json::value::Value, replica: &Replica) -> Value {
    if json.is_object() {
        let map = json.as_object().unwrap();
        match map.get("__TYPE__").and_then(|value| value.as_str()) {
            Some("attrstr") =>
                Value::AttrStr(de_attributed_string(map, replica)),
            _ =>
                Value::Obj(de_object(map, replica)),
        }
    } else if json.is_array() {
        let vec = json.as_array().unwrap();
        Value::Arr(de_array(vec, replica))

    } else if json.is_string() {
        let string = json.as_str().unwrap();
        Value::Str(string.to_string())

    } else if json.is_number() {
        let number = json.as_f64().unwrap();
        Value::Num(number)

    } else if json.is_boolean() {
        let bool_value = json.as_bool().unwrap();
        Value::Bool(bool_value)

    } else {
        Value::Null
    }
}

fn de_attributed_string(map: &Map<String, serde_json::value::Value>, replica: &Replica) -> AttributedString {
    let mut string = AttributedString::new();
    let text = map.get("text").and_then(|value| value.as_str()).unwrap_or("");
    string.insert_text(0, text.to_string(), replica);
    string
}

fn de_object(map: &Map<String, serde_json::value::Value>, replica: &Replica) -> Object {
    let mut object = Object::new();
    for (key, value) in map {
        let key = key.replace("~1", "__TYPE__").replace("~0","~");
        object.put(&key, deserialize(value, replica), replica);
    }
    object
}

fn de_array(vec: &Vec<serde_json::value::Value>, replica: &Replica) -> Array {
    let mut array = Array::new();
    for (i, value) in vec.iter().enumerate() {
        array.insert(i, deserialize(value, replica), replica);
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;
    use Value;
    use Replica;

    const REPLICA: Replica = Replica{site: 2, counter: 3};

    #[test]
    fn test_deserialize_null() {
        assert!(deserialize_str("null", &REPLICA) == Value::Null);
    }

    #[test]
    fn test_deserialize_bool() {
        assert!(deserialize_str("true", &REPLICA) == Value::Bool(true));
        assert!(deserialize_str("false", &REPLICA) == Value::Bool(false));
    }

    #[test]
    fn test_deserialize_number() {
        assert!(deserialize_str("243", &REPLICA) == Value::Num(243.0));
        assert!(deserialize_str("243.4", &REPLICA) == Value::Num(243.4));
        assert!(deserialize_str("-243.4", &REPLICA) == Value::Num(-243.4));
    }

    #[test]
    fn test_deserialize_string() {
        assert!(deserialize_str("\"\"", &REPLICA) == Value::Str("".to_string()));
        assert!(deserialize_str("\"Hello world!\"", &REPLICA) == Value::Str("Hello world!".to_string()));
    }

    #[test]
    fn test_deserialize_attributed_string() {
        let string = r#"{"__TYPE__":"attrstr","text":"Hello world!"}"#;
        let mut value = deserialize_str(&string, &REPLICA);

        let attrstr = value.as_attributed_string().unwrap();
        assert!(attrstr.len() == 12);
        assert!(attrstr.raw_string() == "Hello world!");
    }

    #[test]
    fn test_deserialize_array() {
        let mut value = deserialize_str(r#"[null, 1, "Hey!"]"#, &REPLICA);
        let mut array = value.as_array().unwrap();

        assert!(array.len() == 3);
        assert!(array.get_by_index(0).unwrap().value == Value::Null);
        assert!(array.get_by_index(1).unwrap().value == Value::Num(1.0));
        assert!(array.get_by_index(2).unwrap().value == Value::Str("Hey!".to_string()));
    }

    #[test]
    fn test_deserialize_object() {
        let string = r#"{"a":true, "~1":-3, "~0": false}"#;
        let mut value = deserialize_str(&string, &REPLICA);
        let mut object = value.as_object().unwrap();

        assert!(object.get_by_key("a").unwrap().value == Value::Bool(true));
        assert!(object.get_by_key("__TYPE__").unwrap().value == Value::Num(-3.0));
        assert!(object.get_by_key("~").unwrap().value == Value::Bool(false));
    }

    #[test]
    fn test_deserialize_nested() {
        let string = r#"{"foo":[1,true,null,"hm"], "bar":{"a":true}, "baz": {"__TYPE__":"attrstr","text":"Hello world!"}}"#;
        let mut value = deserialize_str(&string, &REPLICA);
        let mut object1 = value.as_object().unwrap();
        {
            let ref mut array = object1.get_by_key("foo").unwrap().value;
            assert!(array.as_array().is_some());
        }
        {
            let ref mut object2 = object1.get_by_key("bar").unwrap().value;
            assert!(object2.as_object().is_some());
        }
        {
            let ref mut attrstr = object1.get_by_key("baz").unwrap().value;
            assert!(attrstr.as_attributed_string().is_some());
        }
    }
}
