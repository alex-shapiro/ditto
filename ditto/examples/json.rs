extern crate ditto;
extern crate serde;
extern crate rmp_serde;
#[macro_use] extern crate serde_json;

use ditto::{Json, Error};

fn main() {
    // create a List CRDT
    let mut json1 = Json::from_str(r#"
    {
        "foo": 123,
        "bar": [
            "Hello",
            "Aloha",
            "Hola"
        ],
        "baz": true
    }
    "#).unwrap();

    // encode via rmp_serde instead of serde_json to save on bandwidth.
    // the second site hasn't been allocated a site yet, but that's ok.
    let encoded_state = rmp_serde::to_vec(&json1.state()).unwrap();
    let decoded_state = rmp_serde::from_slice(&encoded_state).unwrap();
    let mut json2 = Json::from_state(decoded_state, None).unwrap();

    // when json2 inserts a new value, the state updates but an AwaitingSite
    // error is returned and the op to distribute the edit is cached.
    assert_eq!(json2.insert("/bar/0", "Bonjour"), Err(Error::AwaitingSite));
    assert_eq!(json2.insert("/bar/1", "Hallo"), Err(Error::AwaitingSite));

    // json2 receives its site id and sends its cached ops to site 1.
    let ops = json2.add_site_id(2).unwrap();
    let encoded_ops = rmp_serde::to_vec(&ops).unwrap();
    let decoded_ops: Vec<ditto::json::Op> = rmp_serde::from_slice(&encoded_ops).unwrap();

    // site 1 executes all the ops sent to it by site 2
    for op in decoded_ops {
        json1.execute_op(op);
    }

    // site 1 and site 2 are in sync with the expected value
    assert_eq!(json1.state(), json2.state());
    assert_eq!(json1.local_value(), json!{{
        "foo": 123.0,
        "bar": [
            "Bonjour",
            "Hallo",
            "Hello",
            "Aloha",
            "Hola"
        ],
        "baz": true
    }});
}
