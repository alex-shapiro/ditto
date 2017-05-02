extern crate ditto;
extern crate serde;
extern crate serde_json;
extern crate rmp_serde as rmps;

use ditto::{Register, RegisterValue, Replica, Error, Crdt};

#[test]
fn test_serialize_deserialize() {
    let register1: Register<i32> = Register::new(123);

    // json
    let s_json = serde_json::to_string(&register1).unwrap();
    let mut register2: Register<i32> = serde_json::from_str(&s_json).unwrap();
    assert!(register1.get() == register2.get());
    assert!(register1.site() == register2.site());
    assert!(register2.add_site(2).is_err());

    // msgpack
    let s_msgpack = rmps::to_vec(&register1).unwrap();
    let mut register3: Register<i32> = rmps::from_slice(&s_msgpack).unwrap();
    assert!(register1.get() == register3.get());
    assert!(register1.site() == register3.site());
    assert!(register3.add_site(2).is_err());

    println!("json {} bytes: {}", s_json.len(), &s_json);
    println!("msgpack {} bytes: {:?}", s_msgpack.len(), s_msgpack);
}

#[test]
fn test_serialize_deserialize_value() {
    let value1: RegisterValue<String> = RegisterValue::new("Bob".to_owned(), &Replica::new(123, 32));

    let s_json = serde_json::to_string(&value1).unwrap();
    let value2: RegisterValue<String> = serde_json::from_str(&s_json).unwrap();
    assert!(value2.get() == "Bob");

    let s_msgpack = rmps::to_vec(&value1).unwrap();
    let value3: RegisterValue<String> = rmps::from_slice(&s_msgpack).unwrap();
    assert!(value3.get() == "Bob");

    println!("json {} bytes: {}", s_json.len(), &s_json);
    println!("msgpack {} bytes: {:?}", s_msgpack.len(), s_msgpack);
}

#[test]
fn test_add_site() {
    let mut register1 = Register::new(123);
    let mut register2 = Register::from_value(register1.clone_value(), 0);
    assert!(register2.update(456).unwrap_err() == Error::AwaitingSite);

    let remote_ops = register2.add_site(2).unwrap();
    let _ = register1.execute_remote(&remote_ops[0]);
    assert!(register1.get() == &456);
    assert!(register2.get() == &456);
}

#[test]
fn test_concurrent_updates() {
    let mut register1: Register<i32> = Register::new(11);
    let mut register2: Register<i32> = Register::from_value(register1.clone_value(), 2);
    let mut register3: Register<i32> = Register::from_value(register1.clone_value(), 3);
    let mut register4: Register<i32> = Register::from_value(register1.clone_value(), 4);

    let remote_op1 = register2.update(44).unwrap();
    let remote_op2 = register1.update(22).unwrap();

    let local_op11 = register1.execute_remote(&remote_op1);
    assert!(register1.get() == &22);
    assert!(local_op11.is_none());

    let local_op21 = register2.execute_remote(&remote_op2);
    assert!(register2.get() == &22);
    assert!(local_op21.unwrap().new_value == 22);

    let local_op31 = register3.execute_remote(&remote_op1);
    let local_op32 = register3.execute_remote(&remote_op2);
    assert!(register2.get() == &22);
    assert!(local_op31.unwrap().new_value == 44);
    assert!(local_op32.unwrap().new_value == 22);

    let local_op32 = register4.execute_remote(&remote_op2);
    let local_op31 = register4.execute_remote(&remote_op1);
    assert!(register2.get() == &22);
    assert!(local_op32.unwrap().new_value == 22);
    assert!(local_op31.is_none());
}
