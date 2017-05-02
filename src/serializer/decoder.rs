use array::Array;
use array::element::Element as ArrayElement;
use attributed_string::AttributedString;
use attributed_string::element::Element as AttrStrElement;
use counter::Counter;
use object::element::Element as ObjectElement;
use object::Object;
use object::uid::UID as ObjectUID;
use op::{NestedRemoteOp, RemoteOp};
use op::remote::{IncrementCounter,UpdateObject,UpdateArray,UpdateAttributedString};
use sequence::uid::UID as SequenceUID;
use std::collections::HashMap;
use std::str::FromStr;
use Value;
use Replica;
use serde::{Deserialize, Deserializer};
use serde::de::{self, Visitor, SeqAccess};
use std::fmt;

impl<'de> Deserialize<'de> for NestedRemoteOp {
    fn deserialize<D>(deserializer: D) -> Result<NestedRemoteOp, D::Error> where D: Deserializer<'de> {
        struct NestedRemoteOpVisitor;

        impl<'de> Visitor<'de> for NestedRemoteOpVisitor {
            type Value = NestedRemoteOp;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid NestedRemoteOp")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<NestedRemoteOp, V::Error> where V: SeqAccess<'de> {
                let code: u8 = visitor.next_element()?.ok_or(de::Error::missing_field("opcode"))?;
                let pointer: String = visitor.next_element()?.ok_or(de::Error::missing_field("pointer"))?;

                let op = match code {
                    5 => {
                        let amount: f64 = visitor.next_element()?.ok_or(de::Error::missing_field("IncrementCounter amount"))?;
                        let replica: Replica = visitor.next_element()?.ok_or(de::Error::missing_field("IncrementCounter replica"))?;
                        let op = IncrementCounter{amount: amount, replica: replica};
                        RemoteOp::IncrementCounter(op)
                    },
                    6 => {
                        let key: String = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateObject key"))?;
                        let inserts: Vec<ObjectElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateObject inserts"))?;
                        let deletes: Vec<ObjectElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateObject deletes"))?;
                        let op = UpdateObject{key: key, inserts: inserts, deletes: deletes};
                        RemoteOp::UpdateObject(op)
                    },
                    7 => {
                        let inserts: Vec<ArrayElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateArray inserts"))?;
                        let deletes: Vec<ArrayElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateArray deletes"))?;
                        let op = UpdateArray{inserts: inserts, deletes: deletes};
                        RemoteOp::UpdateArray(op)
                    },
                    8 => {
                        let inserts: Vec<AttrStrElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateAttrstr inserts"))?;
                        let deletes: Vec<AttrStrElement> = visitor.next_element()?.ok_or(de::Error::missing_field("UpdateAttrstr deletes"))?;
                        let op = UpdateAttributedString{inserts: inserts, deletes: deletes};
                        RemoteOp::UpdateAttributedString(op)
                    },
                    _ => return Err(de::Error::missing_field("invalid NestedRemoteOp code")),
                };

                Ok(NestedRemoteOp{pointer: pointer, op: op})
            }
        }

        deserializer.deserialize_any(NestedRemoteOpVisitor)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Value, D::Error> where D: Deserializer<'de> {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid encoded CRDT value")
            }

            fn visit_unit<E>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
                Ok(Value::Num(value as f64))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
                Ok(Value::Num(value as f64))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
                Ok(Value::Num(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Value, E> where E: de::Error {
                self.visit_string(String::from(value))
            }

            fn visit_string<E>(self, value: String) -> Result<Value, E> {
                Ok(Value::Str(value))
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Value, V::Error> where V: SeqAccess<'de> {
                let code: u8 = visitor.next_element()?.ok_or(de::Error::missing_field("opcode"))?;
                match code {
                    0 => {
                        let elements: Vec<AttrStrElement> = visitor.next_element()?.ok_or(de::Error::missing_field("AttrStr elements"))?;
                        let attrstr = AttributedString::assemble(elements);
                        Ok(Value::AttrStr(attrstr))
                    },
                    1 => {
                        let mut elements: Vec<ArrayElement> = visitor.next_element()?.ok_or(de::Error::missing_field("Array elements"))?;
                        elements.insert(0, ArrayElement::start_marker());
                        elements.push(ArrayElement::end_marker());
                        let array = Array::assemble(elements);
                        Ok(Value::Arr(array))
                    },
                    2 => {
                        let mut map: HashMap<String,Vec<ObjectElement>> = HashMap::new();
                        let elements: Vec<ObjectElement> = visitor.next_element()?.ok_or(de::Error::missing_field("Object elements"))?;

                        for element in elements {
                            let key = element.uid.key.clone();
                            map.entry(key).or_insert(vec![]).push(element);
                        }

                        let object = Object::assemble(map);
                        Ok(Value::Obj(object))
                    },
                    3 => {
                        let value: f64 = visitor.next_element()?.ok_or(de::Error::missing_field("Counter value"))?;
                        let replicas: Vec<Replica> = visitor.next_element()?.ok_or(de::Error::missing_field("Counter replicas"))?;
                        let mut site_counters = HashMap::new();
                        for replica in replicas {
                            site_counters.insert(replica.site, replica.counter);
                        }
                        let counter = Counter{value: value, site_counters: site_counters};
                        Ok(Value::Counter(counter))
                    }
                    _ => return Err(de::Error::missing_field("invalid Value code")),
                }
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl<'de> Deserialize<'de> for AttrStrElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        struct AttrStrElementVisitor;

        impl<'de> Visitor<'de> for AttrStrElementVisitor {
            type Value = AttrStrElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid AttrStrElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqAccess<'de> {
                let encoded_uid: String = visitor.next_element()?.ok_or(de::Error::missing_field("AttrStrElement uid"))?;
                let text: String = visitor.next_element()?.ok_or(de::Error::missing_field("AttrStrElement text"))?;
                let uid = SequenceUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("AttrStrElement uid"))?;
                Ok(AttrStrElement::text(text.to_string(), uid))
            }
        }

        deserializer.deserialize_seq(AttrStrElementVisitor)
    }
}

impl<'de> Deserialize<'de> for ArrayElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        struct ArrayElementVisitor;

        impl<'de> Visitor<'de> for ArrayElementVisitor {
            type Value = ArrayElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid ArrayElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqAccess<'de> {
                let encoded_uid: String = visitor.next_element()?.ok_or(de::Error::missing_field("ArrayElement uid"))?;
                let value: ::Value = visitor.next_element()?.ok_or(de::Error::missing_field("ArrayElement value"))?;
                let uid = SequenceUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("ArrayElement uid"))?;
                Ok(ArrayElement::new(value, uid))
            }
        }

        deserializer.deserialize_seq(ArrayElementVisitor)
    }
}

impl<'de> Deserialize<'de> for ObjectElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        struct ObjectElementVisitor;

        impl<'de> Visitor<'de> for ObjectElementVisitor {
            type Value = ObjectElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid ObjectElement")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqAccess<'de> {
                let encoded_uid: String = visitor.next_element()?.ok_or(de::Error::missing_field("ObjectElement uid"))?;
                let value: ::Value = visitor.next_element()?.ok_or(de::Error::missing_field("ObjectElement value"))?;
                let uid = ObjectUID::from_str(&encoded_uid).map_err(|_| de::Error::missing_field("ObjectElement uid"))?;
                Ok(ObjectElement{uid: uid, value: value})
            }
        }

        deserializer.deserialize_seq(ObjectElementVisitor)
    }
}
