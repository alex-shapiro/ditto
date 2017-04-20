use super::LocalValue;
use serde::de::{self, Deserialize, Deserializer, Visitor, SeqAccess, MapAccess};
use std::collections::HashMap;
use std::fmt;

impl<'de> Deserialize<'de> for LocalValue {
    fn deserialize<D>(deserializer: D) -> Result<LocalValue, D::Error> where D: Deserializer<'de> {
        struct LocalValueVisitor;

        impl<'de> Visitor<'de> for LocalValueVisitor {
            type Value = LocalValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid local value")
            }

            fn visit_unit<E>(self) -> Result<LocalValue, E> {
                Ok(LocalValue::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<LocalValue, E> {
                Ok(LocalValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<LocalValue, E> {
                Ok(LocalValue::Num(value as f64))
            }

            fn visit_u64<E>(self, value: u64) -> Result<LocalValue, E> {
                Ok(LocalValue::Num(value as f64))
            }

            fn visit_f64<E>(self, value: f64) -> Result<LocalValue, E> {
                Ok(LocalValue::Num(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<LocalValue, E> where E: de::Error {
                self.visit_string(String::from(value))
            }

            fn visit_string<E>(self, value: String) -> Result<LocalValue, E> {
                Ok(LocalValue::Str(value))
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<LocalValue, V::Error> where V: SeqAccess<'de> {
                let mut items: Vec<LocalValue> = vec![];
                while let Some(value) = visitor.next_element()? { items.push(value) }
                Ok(LocalValue::Arr(items))
            }

            fn visit_map<M>(self, mut visitor: M) -> Result<LocalValue, M::Error> where M: MapAccess<'de> {
                let mut map: HashMap<String, LocalValue> = HashMap::new();
                while let Some((key, value)) = visitor.next_entry()? { map.insert(key, value); }

                if let Some(LocalValue::Str(special_type)) = map.remove("__TYPE__") {
                    if special_type == "attrstr" {
                        if let Some(LocalValue::Str(text)) = map.remove("text") {
                            return Ok(LocalValue::AttrStr(text))
                        } else {
                            return Err(de::Error::missing_field("AttrStr text"))
                        }
                    } else if special_type == "counter" {
                        if let Some(LocalValue::Num(number)) = map.remove("value") {
                            return Ok(LocalValue::Counter(number))
                        } else {
                            return Err(de::Error::missing_field("Counter value"))
                        }
                    } else {
                        return Err(de::Error::missing_field("invalid special type"))
                    }
                }

                Ok(LocalValue::Obj(map))
            }
        }

        deserializer.deserialize_any(LocalValueVisitor)
    }
}
