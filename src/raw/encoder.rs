use Value;
use array::Array;
use attributed_string::AttributedString;
use object::Object;
use op::local::{LocalOp, Put, Delete, InsertItem, DeleteItem, InsertText, DeleteText, IncrementNumber};
use serde_json::builder::ObjectBuilder;
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

pub fn encode_op(op: &LocalOp) -> Json {
    match *op {
        LocalOp::Put(ref op) =>
            encode_op_put(op),
        LocalOp::Delete(ref op) =>
            encode_op_delete(op),
        LocalOp::InsertItem(ref op) =>
            encode_op_insert_item(op),
        LocalOp::DeleteItem(ref op) =>
            encode_op_delete_item(op),
        LocalOp::InsertText(ref op) =>
            encode_op_insert_text(op),
        LocalOp::DeleteText(ref op) =>
            encode_op_delete_text(op),
        LocalOp::IncrementNumber(ref op) =>
            encode_op_increment_number(op),
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
    ObjectBuilder::new()
        .insert("__TYPE__", Json::String("attrstr".to_string()))
        .insert("text", Json::String(string.raw_string()))
        .build()
}

fn encode_op_put(op: &Put) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("put".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("key", Json::String(op.key.clone()))
        .insert("value", encode(&op.value))
        .build()
}

fn encode_op_delete(op: &Delete) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("delete".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("key", Json::String(op.key.clone()))
        .build()
}

fn encode_op_insert_item(op: &InsertItem) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("insert_item".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("index", Json::U64(op.index as u64))
        .insert("value", encode(&op.value))
        .build()
}

fn encode_op_delete_item(op: &DeleteItem) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("delete_item".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("index", Json::U64(op.index as u64))
        .build()
}

fn encode_op_insert_text(op: &InsertText) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("insert_text".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("index", Json::U64(op.index as u64))
        .insert("text", Json::String(op.text.clone()))
        .build()
}

fn encode_op_delete_text(op: &DeleteText) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("delete_text".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("index", Json::U64(op.index as u64))
        .insert("len", Json::U64(op.len as u64))
        .build()
}

fn encode_op_increment_number(op: &IncrementNumber) -> Json {
    ObjectBuilder::new()
        .insert("op", Json::String("increment_number".to_string()))
        .insert("path", Json::String(op.path.clone()))
        .insert("amount", Json::F64(op.amount))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Value;
    use Replica;
    use array::Array;
    use attributed_string::AttributedString;
    use op::local::{LocalOp, Put, Delete, InsertItem, DeleteItem, InsertText, DeleteText, IncrementNumber};
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

    #[test]
    fn test_encode_op_put() {
        let op = LocalOp::Put(Put{
            path: "/a/sdf/x".to_string(),
            key: "foo".to_string(),
            value: Value::Bool(true),
        });

        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"put""#));
        assert!(json.contains(r#""path":"/a/sdf/x""#));
        assert!(json.contains(r#""key":"foo""#));
        assert!(json.contains(r#""value":true"#));
    }

    #[test]
    fn test_encode_op_delete() {
        let op = LocalOp::Delete(Delete{
            path: "/a/sdf/x".to_string(),
            key: "foo".to_string(),
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"delete""#));
        assert!(json.contains(r#""path":"/a/sdf/x""#));
        assert!(json.contains(r#""key":"foo""#));
    }

    #[test]
    fn test_encode_op_insert_item() {
        let op = LocalOp::InsertItem(InsertItem{
            path: "/1/203/xx".to_string(),
            index: 43,
            value: Value::array(),
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"insert_item""#));
        assert!(json.contains(r#""path":"/1/203/xx""#));
        assert!(json.contains(r#""index":43"#));
        assert!(json.contains(r#""value":[]"#));
    }

    #[test]
    fn test_encode_op_delete_item() {
        let op = LocalOp::DeleteItem(DeleteItem{
            path: "/1/203/xx".to_string(),
            index: 43,
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"delete_item""#));
        assert!(json.contains(r#""path":"/1/203/xx""#));
        assert!(json.contains(r#""index":43"#));
    }

    #[test]
    fn test_encode_op_insert_text() {
        let op = LocalOp::InsertText(InsertText{
            path: "/1/203/xx".to_string(),
            index: 112,
            text: "Hiya".to_string()
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"insert_text""#));
        assert!(json.contains(r#""path":"/1/203/xx""#));
        assert!(json.contains(r#""index":112"#));
        assert!(json.contains(r#""text":"Hiya""#));
    }

    #[test]
    fn test_encode_op_delete_text() {
        let op = LocalOp::DeleteText(DeleteText{
            path: "/1/203/xx".to_string(),
            index: 112,
            len: 84,
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"delete_text""#));
        assert!(json.contains(r#""path":"/1/203/xx""#));
        assert!(json.contains(r#""index":112"#));
        assert!(json.contains(r#""len":84"#));
    }

    #[test]
    fn test_encode_op_increment_number() {
        let op = LocalOp::IncrementNumber(IncrementNumber{
            path: "/1/203/xx".to_string(),
            amount: 232.013,
        });
        let json = encode_op_str(&op);
        assert!(json.contains(r#""op":"increment_number""#));
        assert!(json.contains(r#""path":"/1/203/xx""#));
        assert!(json.contains(r#""amount":232.013"#));
    }

    fn encode_str(value: &Value) -> String {
        let json = encode(value);
        serde_json::to_string(&json).ok().unwrap()
    }

    fn encode_op_str(op: &LocalOp) -> String {
        let json = encode_op(op);
        serde_json::to_string(&json).ok().unwrap()
    }
}
