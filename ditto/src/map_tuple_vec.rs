//! Serialize and Deserialize a HashMap as a Vec of tuples.
//! This allows a serialized HashMap to have keys of any type
//! instead of only Strings.

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::ser::SerializeSeq;
use serde::de::{Visitor, SeqAccess};

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

pub fn serialize<K, V, S>(data: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer,
          K: Hash + Eq + Serialize,
          V: Serialize,
{
    let mut seq = serializer.serialize_seq(Some(data.len()))?;
    for kv_pair in data.iter() {
        seq.serialize_element(&kv_pair)?;
    }
    seq.end()
}

pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
    where D: Deserializer<'de>,
          K: Hash + Eq + Deserialize<'de>,
          V: Deserialize<'de>,
{
    struct HashMapVisitor<K: Hash + Eq, V> {
        marker: PhantomData<HashMap<K, V>>,
    }

    impl<K: Hash + Eq, V> HashMapVisitor<K, V> {
        fn new() -> Self {
            HashMapVisitor{marker: PhantomData}
        }
    }

    impl<'de, K, V> Visitor<'de> for HashMapVisitor<K, V> where
        K: Hash + Eq + Deserialize<'de>,
        V: Deserialize<'de>,
    {
        type Value = HashMap<K, V>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a list of (K, Vec<V>) tuples")
        }

        fn visit_seq<Vis>(self, mut visitor: Vis) -> Result<Self::Value, Vis::Error> where Vis: SeqAccess<'de> {
            let mut hash_map = HashMap::with_capacity(visitor.size_hint().unwrap_or(0));
            while let Some((key, values)) = visitor.next_element()? {
                hash_map.insert(key, values);
            }
            Ok(hash_map)
        }
    }

    deserializer.deserialize_seq(HashMapVisitor::new())
}
