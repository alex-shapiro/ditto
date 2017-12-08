extern crate serde;
extern crate serde_json;
extern crate rmp_serde;

pub fn test_serde<T>(value: T)
    where T: ::std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned + PartialEq
 {
    let json = serde_json::to_string(&value).unwrap();
    let value2 = serde_json::from_str(&json).unwrap();
    assert_eq!(value, value2);

    let msgpack = rmp_serde::to_vec(&value).unwrap();
    let value3 = rmp_serde::from_slice(&msgpack).unwrap();
    assert_eq!(value, value3);
}
