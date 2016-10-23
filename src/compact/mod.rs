pub mod decoder;
pub mod encoder;

pub use self::decoder::decode;
pub use self::encoder::encode;

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
    fn test_null() {
        let original = Value::Null;
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(encoded == "null");
        assert!(original == decoded);
    }

    #[test]
    fn test_bool_true() {
        let original = Value::Bool(true);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(encoded == "true");
        assert!(original == decoded);
    }

    #[test]
    fn test_bool_false() {
        let original = Value::Bool(false);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(encoded == "false");
        assert!(original == decoded);
    }

    #[test]
    fn test_number() {
        let original = Value::Num(304.3);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(encoded == "304.3");
        assert!(original == decoded);
    }

    #[test]
    fn test_string() {
        let original = Value::Str("hi!".to_string());
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(encoded == "\"hi!\"");
        assert!(original == decoded);
    }

    #[test]
    fn test_attributed_string() {
        let mut string = AttributedString::new();
        string.insert_text(0,  "the ".to_string(), &Replica::new(1, 1));
        string.insert_text(4,  "quick ".to_string(), &Replica::new(1, 2));
        string.insert_text(16, "brown ".to_string(), &Replica::new(1, 3));
        string.insert_text(22, "fox ".to_string(), &Replica::new(1, 4));
        string.insert_text(26, "jumped".to_string(), &Replica::new(1, 5));

        let original = Value::AttrStr(string);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_array() {
        let mut array = Array::new();
        array.insert(0, Value::Null, &Replica::new(1, 1));
        array.insert(1, Value::Bool(true), &Replica::new(2, 1));
        array.insert(2, Value::Num(-132.0), &Replica::new(14, 3));
        array.insert(3, Value::Str("x".to_string()), &Replica::new(48, 84));
        array.insert(4, Value::Bool(false), &Replica::new(1, 552));

        let original = Value::Arr(array);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_object() {
        let mut object = Object::new();
        object.put("", Value::Null, &Replica::new(1, 1));
        object.put("a", Value::Bool(true), &Replica::new(2, 1));
        object.put("__TYPE__", Value::Num(-132.0), &Replica::new(14, 3));
        object.put("~0", Value::Str("x".to_string()), &Replica::new(48, 84));
        object.put("x/y", Value::Bool(false), &Replica::new(1, 552));

        let original = Value::Obj(object);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_nested() {
        let mut array   = Array::new();
        array.insert(0, Value::object(), &Replica::new(34,2));
        array.insert(1, Value::attrstr(), &Replica::new(392,12));
        array.insert(2, Value::array(), &Replica::new(4782,4));

        let original = Value::Arr(array);
        let encoded  = encode_str(&original);
        let decoded  = decode_str(&encoded).ok().unwrap();
        assert!(original == decoded);
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::to_string(&json).ok().unwrap()
    }

    fn decode_str(value: &str) -> Result<Value,decoder::Error> {
        let json: serde_json::Value = serde_json::from_str(value).ok().unwrap();
        decode(&json)
    }
}
