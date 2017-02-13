pub mod decoder;
pub mod encoder;

#[cfg(test)]
mod tests {
    use super::*;
    use array::Array;
    use attributed_string::AttributedString;
    use object::Object;
    use op::{NestedRemoteOp, RemoteOp};
    use op::remote::IncrementNumber;
    use Replica;
    use serde_json;
    use Value;

    #[test]
    fn test_null() {
        let original = Value::Null;
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "null");
        assert!(original == decoded);
    }

    #[test]
    fn test_bool_true() {
        let original = Value::Bool(true);
        let encoded  = encode_str(&original);
        let decoded: Value  = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "true");
        assert!(original == decoded);
    }

    #[test]
    fn test_bool_false() {
        let original = Value::Bool(false);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "false");
        assert!(original == decoded);
    }

    #[test]
    fn test_number() {
        let original = Value::Num(304.3);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "304.3");
        assert!(original == decoded);
    }

    #[test]
    fn test_string() {
        let original = Value::Str("hi!".to_string());
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(encoded == "\"hi!\"");
        assert!(original == decoded);
    }

    #[test]
    fn test_attributed_string() {
        let mut string = AttributedString::new();
        let _ = string.insert_text(0,  "the ".to_string(), &Replica::new(1, 1));
        let _ = string.insert_text(4,  "quick ".to_string(), &Replica::new(1, 2));
        let _ = string.insert_text(16, "brown ".to_string(), &Replica::new(1, 3));
        let _ = string.insert_text(22, "fox ".to_string(), &Replica::new(1, 4));
        let _ = string.insert_text(26, "jumped".to_string(), &Replica::new(1, 5));

        let original = Value::AttrStr(string);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_array() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::Null, &Replica::new(1, 1));
        let _ = array.insert(1, Value::Bool(true), &Replica::new(2, 1));
        let _ = array.insert(2, Value::Num(-132.0), &Replica::new(14, 3));
        let _ = array.insert(3, Value::Str("x".to_string()), &Replica::new(48, 84));
        let _ = array.insert(4, Value::Bool(false), &Replica::new(1, 552));

        let original = Value::Arr(array);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();

        println!("original: {:?}", original);
        println!("decoded: {:?}", decoded);

        assert!(original == decoded);
    }

    #[test]
    fn test_object() {
        let mut object = Object::new();
        let _ = object.put("", Value::Null, &Replica::new(1, 1));
        let _ = object.put("a", Value::Bool(true), &Replica::new(2, 1));
        let _ = object.put("__TYPE__", Value::Num(-132.0), &Replica::new(14, 3));
        let _ = object.put("~0", Value::Str("x".to_string()), &Replica::new(48, 84));
        let _ = object.put("x/y", Value::Bool(false), &Replica::new(1, 552));

        let original = Value::Obj(object);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_nested() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::object(), &Replica::new(34,2));
        let _ = array.insert(1, Value::attrstr(), &Replica::new(392,12));
        let _ = array.insert(2, Value::array(), &Replica::new(4782,4));

        let original = Value::Arr(array);
        let encoded  = encode_str(&original);
        let decoded: Value = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_update_object() {
        let replica    = Replica::new(4,3);
        let mut object = Object::new();
        let op         = object.put("foo", Value::Num(409.1), &replica);
        let remote_op  = RemoteOp::UpdateObject(op);
        let pointer    = "hfas/fs,bar/bhsd".to_owned();

        let original = NestedRemoteOp{pointer: pointer, op: remote_op};
        let encoded  = serde_json::to_string(&original).unwrap();
        let decoded: NestedRemoteOp = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_update_array() {
        let replica   = Replica::new(4,3);
        let mut array = Array::new();
        let op        = array.insert(0, Value::Num(409.1), &replica).unwrap();
        let remote_op = RemoteOp::UpdateArray(op);
        let pointer   = "/asdf/hjkl".to_owned();

        let original = NestedRemoteOp{pointer: pointer, op: remote_op};
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: NestedRemoteOp = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_update_attributed_string() {
        let replica     = Replica::new(4,3);
        let mut attrstr = AttributedString::new();
        let op          = attrstr.insert_text(0, "hello".to_owned(), &replica).unwrap();
        let remote_op   = RemoteOp::UpdateAttributedString(op);
        let pointer     = "/asdf/hjkl".to_owned();

        let original = NestedRemoteOp{pointer: pointer, op: remote_op};
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: NestedRemoteOp = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    #[test]
    fn test_increment_number() {
        let op        = IncrementNumber{amount: 483.24};
        let remote_op = RemoteOp::IncrementNumber(op);
        let pointer   = "/7nlhs/fhs04-baz/f7y2".to_owned();

        let original = NestedRemoteOp{pointer: pointer, op: remote_op};
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: NestedRemoteOp = serde_json::from_str(&encoded).unwrap();
        assert!(original == decoded);
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::to_string(&json).ok().unwrap()
    }
}
