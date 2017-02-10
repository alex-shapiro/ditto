use Value;
use array::Array;
use attributed_string::AttributedString;
use object::Object;
use op::{NestedLocalOp, LocalOp};
use op::local::{Put, Delete, InsertItem, DeleteItem, InsertText, DeleteText, ReplaceText, IncrementNumber};
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
            json!(string),
        Value::Num(number) =>
            json!(number),
        Value::Bool(bool_value) =>
            json!(bool_value),
        Value::Null =>
            json!(null),
    }
}

pub fn encode_op(nested_op: &NestedLocalOp) -> Json {
    let pointer = &nested_op.pointer;
    let operation = &nested_op.op;
    match *operation {
        LocalOp::Put(ref op) =>
            encode_op_put(op, pointer),
        LocalOp::Delete(ref op) =>
            encode_op_delete(op, pointer),
        LocalOp::InsertItem(ref op) =>
            encode_op_insert_item(op, pointer),
        LocalOp::DeleteItem(ref op) =>
            encode_op_delete_item(op, pointer),
        LocalOp::InsertText(ref op) =>
            encode_op_insert_text(op, pointer),
        LocalOp::DeleteText(ref op) =>
            encode_op_delete_text(op, pointer),
        LocalOp::ReplaceText(ref op) =>
            encode_op_replace_text(op, pointer),
        LocalOp::IncrementNumber(ref op) =>
            encode_op_increment_number(op, pointer),
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

fn encode_attributed_string(attrstr: &AttributedString) -> Json {
    json!({
        "__TYPE__": "attrstr",
        "text": attrstr.to_string()
    })
}

fn encode_op_put(op: &Put, pointer: &str) -> Json {
    json!({
        "op": "put",
        "pointer": pointer,
        "key": op.key,
        "value": encode(&op.value),
    })
}

fn encode_op_delete(op: &Delete, pointer: &str) -> Json {
    json!({
        "op": "delete",
        "pointer": pointer,
        "key": op.key,
    })
}

fn encode_op_insert_item(op: &InsertItem, pointer: &str) -> Json {
    json!({
        "op": "insert_item",
        "pointer": pointer,
        "index": op.index,
        "value": encode(&op.value),
    })
}

fn encode_op_delete_item(op: &DeleteItem, pointer: &str) -> Json {
    json!({
        "op": "delete_item",
        "pointer": pointer,
        "index": op.index,
    })
}

fn encode_op_insert_text(op: &InsertText, pointer: &str) -> Json {
    json!({
        "op": "insert_text",
        "pointer": pointer,
        "index": op.index,
        "text": op.text,
    })
}

fn encode_op_delete_text(op: &DeleteText, pointer: &str) -> Json {
    json!({
        "op": "delete_text",
        "pointer": pointer,
        "index": op.index,
        "len": op.len,
    })
}

fn encode_op_replace_text(op: &ReplaceText, pointer: &str) -> Json {
    json!({
        "op": "replace_text",
        "pointer": pointer,
        "index": op.index,
        "len": op.len,
        "text": op.text,
    })
}

fn encode_op_increment_number(op: &IncrementNumber, pointer: &str) -> Json {
    json!({
        "op": "increment_number",
        "pointer": pointer,
        "amount": op.amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Value;
    use Replica;
    use array::Array;
    use attributed_string::AttributedString;
    use op::NestedLocalOp;
    use op::local::{LocalOp, Put, Delete, InsertItem, DeleteItem, InsertText, DeleteText, ReplaceText, IncrementNumber};
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
        let _ = attrstr.insert_text(0, "the ".to_string(), &REPLICA);
        let _ = attrstr.insert_text(4, "quick ".to_string(), &REPLICA);
        let _ = attrstr.insert_text(10, "brown ".to_string(), &REPLICA);
        let _ = attrstr.insert_text(16, "fox".to_string(), &REPLICA);
        let value = Value::AttrStr(attrstr);
        assert!(encode_str(&value) == r#"{"__TYPE__":"attrstr","text":"the quick brown fox"}"#);
    }

    #[test]
    fn test_encode_array() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::Num(1.0), &REPLICA);
        let _ = array.insert(1, Value::Bool(true), &REPLICA);
        let _ = array.insert(2, Value::Str("hey".to_string()), &REPLICA);
        let value = Value::Arr(array);
        assert!(encode_str(&value) == r#"[1.0,true,"hey"]"#);
    }

    #[test]
    fn test_encode_object() {
        let mut object = Object::new();
        let _ = object.put("a", Value::Num(1.0), &REPLICA);
        let _ = object.put("__TYPE__", Value::Null, &REPLICA);
        let _ = object.put("~cookies~", Value::Bool(true), &REPLICA);
        let value = Value::Obj(object);
        let json = encode_str(&value);
        assert!(json.contains(r#""a":1.0"#));
        assert!(json.contains(r#""~0cookies~0":true"#));
        assert!(json.contains(r#""~1":null"#));
    }

    #[test]
    fn test_encode_nested() {
        let mut array = Array::new();
        let _ = array.insert(0, Value::object(), &REPLICA);
        let _ = array.insert(1, Value::attrstr(), &REPLICA);
        let _ = array.insert(2, Value::array(), &REPLICA);
        let _ = array.insert(3, Value::Str("hi!".to_string()), &REPLICA);
        let _ = array.insert(4, Value::Num(-3234.1), &REPLICA);
        let _ = array.insert(5, Value::Bool(true), &REPLICA);
        let _ = array.insert(6, Value::Null, &REPLICA);
        let value = Value::Arr(array);
        let json = encode_str(&value);
        assert!(json == r#"[{},{"__TYPE__":"attrstr","text":""},[],"hi!",-3234.1,true,null]"#);
    }

    #[test]
    fn test_encode_op_put() {
        let nested_op = NestedLocalOp{
            pointer: "/a/sdf/x".to_string(),
            op: LocalOp::Put(Put{key: "foo".to_string(), value: Value::Bool(true)}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"put""#));
        assert!(json.contains(r#""pointer":"/a/sdf/x""#));
        assert!(json.contains(r#""key":"foo""#));
        assert!(json.contains(r#""value":true"#));
    }

    #[test]
    fn test_encode_op_delete() {
        let nested_op = NestedLocalOp{
            pointer: "/a/sdf/x".to_string(),
            op: LocalOp::Delete(Delete{key: "foo".to_string()}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"delete""#));
        assert!(json.contains(r#""pointer":"/a/sdf/x""#));
        assert!(json.contains(r#""key":"foo""#));
    }

    #[test]
    fn test_encode_op_insert_item() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::InsertItem(InsertItem{index: 43, value: Value::array()}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"insert_item""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""index":43"#));
        assert!(json.contains(r#""value":[]"#));
    }

    #[test]
    fn test_encode_op_delete_item() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::DeleteItem(DeleteItem{index: 43}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"delete_item""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""index":43"#));
    }

    #[test]
    fn test_encode_op_insert_text() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::InsertText(InsertText{index: 112, text: "Hiya".to_string()}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"insert_text""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""index":112"#));
        assert!(json.contains(r#""text":"Hiya""#));
    }

    #[test]
    fn test_encode_op_delete_text() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::DeleteText(DeleteText{index: 112,len: 84}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"delete_text""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""index":112"#));
        assert!(json.contains(r#""len":84"#));
    }

    #[test]
    fn test_encode_op_replace_text() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::ReplaceText(ReplaceText{index: 112,len: 84, text: "hello!".to_owned()}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"replace_text""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""index":112"#));
        assert!(json.contains(r#""len":84"#));
        assert!(json.contains(r#""text":"hello!"#));
    }

    #[test]
    fn test_encode_op_increment_number() {
        let nested_op = NestedLocalOp{
            pointer: "/1/203/xx".to_string(),
            op: LocalOp::IncrementNumber(IncrementNumber{amount: 232.013,}),
        };

        let json = encode_op_str(&nested_op);
        assert!(json.contains(r#""op":"increment_number""#));
        assert!(json.contains(r#""pointer":"/1/203/xx""#));
        assert!(json.contains(r#""amount":232.013"#));
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::to_string(&json).ok().unwrap()
    }

    fn encode_op_str(op: &NestedLocalOp) -> String {
        let json = encode_op(op);
        serde_json::to_string(&json).ok().unwrap()
    }
}
