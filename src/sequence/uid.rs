//! A sequence Uid is an unique identifier that maintains
//! a total order of sequential elements in a distributed
//! environment.
//!
//! ## Implementation Notes
//!
//! New Uids are generated with a variant of the LSEQ
//! algorithm. It diverges from the original paper by
//! choosing allocation strategy deterministically rather
//! than according to a random coin flip. Randomized
//! allocation strategies cannot converge in a distributed
//! environment because replicas cannot reliably coordinate
//! strategies, and so Uid bit size increases quickly with
//! the number of sequence elements. However, a
//! deterministic allocation strategy known by all replicas
//! ahead of time is inherently coordinated, and Uid bit
//! size increases more slowly with the number of elements.

use base64;
use Error;
use num_bigint::{BigUint, ToBigUint};
use num_traits::cast::ToPrimitive;
use rand::distributions::{Range};
use rand::Rng;
use rand;
use dot::{Dot, SiteId, Counter};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{self, Visitor, SeqAccess};
use std::cmp::{self, Ordering};
use std::fmt::{self, Debug};
use std::str::FromStr;
use vlq;

const BASE_LEVEL: usize = 20;
const MAX_LEVEL:  usize = 64;
const BOUNDARY:   usize = 40;

#[derive(Clone,PartialEq,Eq)]
pub struct Uid {
    pub position: BigUint,
    pub site_id:  SiteId,
    pub counter:  Counter,
}

lazy_static! {
    pub static ref MIN: Uid = Uid::min();
    pub static ref MAX: Uid = Uid::max();
}

impl Uid {
    fn new(position: BigUint, site_id: u32, counter: u32) -> Self {
        Uid{position, site_id, counter}
    }

    pub fn dot(&self) -> Dot {
        Dot{site_id: self.site_id, counter: self.counter}
    }

    pub fn min() -> Self {
        Uid::new(big(1 << BASE_LEVEL), 0, 0)
    }

    pub fn max() -> Self {
        let position = big((1 << (BASE_LEVEL+1)) - 1);
        Uid::new(position, u32::max_value(), u32::max_value())
    }

    pub fn between(uid1: &Uid, uid2: &Uid, dot: Dot) -> Self {
        let position1            = &uid1.position;
        let position2            = &uid2.position;
        let mut position         = big(1);
        let mut significant_bits = 1;

        for level in BASE_LEVEL..(MAX_LEVEL+1) {
            significant_bits += level;
            let pos1 = Uid::get_pos(position1, level, significant_bits).unwrap_or(0);
            let pos2 = Uid::get_pos(position2, level, significant_bits).unwrap_or((1 << level) - 1);

            if pos1 + 1 < pos2 {
                let pos = Uid::generate_pos(pos1, pos2, level);
                position = (position << level) + big(pos);
                return Uid::new(position, dot.site_id, dot.counter);
            } else {
                position = (position << level) + big(pos1);
            }
        }
        panic!(format!("Uid cannot have more than ({}) levels", MAX_LEVEL));
    }

    /// Gets the value for a particular level in a position
    /// if the position has a value for that level. A position's
    /// most significant bit is not part of any level (it
    /// is a placeholder to prevent the highest level from being
    /// truncated). The next 3 most significant bits form level 3,
    /// the next 4 most significant bits form level 4; and so on.
    fn get_pos(position: &BigUint, level: usize, significant_bits: usize) -> Option<usize> {
        let bits = position.bits();
        if bits >= significant_bits {
            let insignificant_bits = bits - significant_bits;
            let level_mask = big((1 << level) - 1);
            let pos = (position >> insignificant_bits) & level_mask;
            Some(pos.to_usize().unwrap())
        } else {
            None
        }
    }

    /// Generates a number that falls between pos1 and pos2.
    /// It requires pos1 + 1 < pos2. Uses boundary+ strategy
    /// on odd levels and boundary+ strategy on even levels.
    ///
    /// boundary+ returns an integer in the interval
    /// [pos1+1, min(pos1+BOUNDARY, pos2-1)]
    ///
    /// boundary- returns an integer in the interval
    /// [max(pos1+1, pos2-BOUNDARY), pos2-1]
    ///
    fn generate_pos(pos1: usize, pos2: usize, level: usize) -> usize {
        let range =
            if Uid::use_boundary_plus_strategy(level) {
                let lo_bound = pos1+1;
                let hi_bound = cmp::min(pos1+BOUNDARY, pos2);
                Range::new(lo_bound, hi_bound)
            } else if pos2 <= BOUNDARY {
                let lo_bound = pos1+1;
                let hi_bound = pos2;
                Range::new(lo_bound, hi_bound)
            } else {
                let lo_bound = cmp::max(pos1+1, pos2-BOUNDARY);
                let hi_bound = pos2;
                Range::new(lo_bound, hi_bound)
            };
        let mut rng = rand::rngs::OsRng::new().unwrap();
        rng.sample(range)
    }

    // TODO: Use Boundary- for arrays on odd levels.
    // In a Text crdt, Boundary- doesn't make any sense
    // because no one inserts text backwards. On an array,
    // it might make more sense because array ops are more
    // likely to be truly random access.
    fn use_boundary_plus_strategy(_: usize) -> bool {
        true
    }

    fn to_vlq(&self) -> Vec<u8> {
        let mut vlq = vlq::encode_biguint(&self.position);
        vlq.append(&mut vlq::encode_u32(self.site_id));
        vlq.append(&mut vlq::encode_u32(self.counter));
        vlq

    }

    fn from_vlq(vlq: &[u8]) -> Result<Self, Error> {
        let (position, vlq_rest1) = vlq::decode_biguint(vlq)?;
        let (site_id, vlq_rest2) = vlq::decode_u32(vlq_rest1)?;
        let (counter, _) = vlq::decode_u32(vlq_rest2)?;
        Ok(Uid{position, site_id, counter})
    }
}

fn big(num: usize) -> BigUint {
    num.to_biguint().unwrap()
}

impl PartialOrd for Uid {
    fn partial_cmp(&self, other: &Uid) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Uid {
    fn cmp(&self, other: &Uid) -> Ordering {
        let mut self_position = self.position.clone();
        let mut other_position = other.position.clone();

        let self_bits = self_position.bits();
        let other_bits = other_position.bits();

        // truncate
        if self_bits > other_bits {
            self_position >>= self_bits - other_bits;
        } else {
            other_position >>= other_bits - self_bits;
        }

        // compare
        if self_position < other_position {
            Ordering::Less
        } else if self_position > other_position {
            Ordering::Greater
        } else if self_bits < other_bits {
            Ordering::Less
        } else if self_bits > other_bits {
            Ordering::Greater
        } else if self.site_id < other.site_id {
            Ordering::Less
        } else if self.site_id > other.site_id {
            Ordering::Greater
        } else if self.counter < other.counter {
            Ordering::Less
        } else if self.counter > other.counter {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl Debug for Uid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let uid = self.position.to_str_radix(2);
        write!(f, "Uid{{position: {}, site_id: {}, counter: {}}}", uid, self.site_id, self.counter)
    }
}

impl ToString for Uid {
    fn to_string(&self) -> String {
        base64::encode_config(&self.to_vlq(), base64::URL_SAFE_NO_PAD)
    }
}

impl FromStr for Uid {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vlq = base64::decode_config(s, base64::URL_SAFE_NO_PAD).map_err(|_| Error::DeserializeSequenceUid)?;
        Uid::from_vlq(&vlq)
    }
}

impl Serialize for Uid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_bytes(&self.to_vlq())
    }
}

impl<'de> Deserialize<'de> for Uid {
    fn deserialize<D>(deserializer: D) -> Result<Uid, D::Error> where D: Deserializer<'de> {
        struct UidVisitor;

        impl<'de> Visitor<'de> for UidVisitor {
            type Value = Uid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a byte buffer")
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error> where V: SeqAccess<'de> {
                let mut vec = Vec::with_capacity(visitor.size_hint().unwrap_or(0));
                while let Some(byte) = visitor.next_element()? { vec.push(byte); }
                Ok(Uid::from_vlq(&vec).map_err(|_| de::Error::missing_field("invalid VLQ value"))?)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where E: de::Error {
                Ok(Uid::from_vlq(v).map_err(|_| de::Error::missing_field("invalid VLQ value"))?)
            }
        }

        deserializer.deserialize_any(UidVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::{BigUint, ToBigUint};
    use std::str::FromStr;
    use serde_json;
    use rmp_serde;

    const DOT: Dot = Dot{site_id: 3, counter: 2};

    #[test]
    fn test_min() {
        let uid = Uid::min();
        assert!(uid.position == big(0b1_00000000000000000000));
        assert!(uid.site_id == 0);
        assert!(uid.counter == 0);
    }

    #[test]
    fn test_max() {
        let uid = Uid::max();
        assert!(uid.position == big(0b1_11111111111111111111));
        assert!(uid.site_id == 4294967295);
        assert!(uid.counter == 4294967295);
    }

    #[test]
    fn test_ord() {
        let uid0 = Uid::min();
        let uid1 = Uid{position: big(0b1_00000000000000101001), site_id: 8, counter: 382};
        let uid2 = Uid{position: big(0b1_00000000000101010010), site_id: 1, counter: 5};
        let uid3 = Uid{position: big(0b1_00000000000101010010), site_id: 1, counter: 5};
        let uid4 = Uid{position: big(0b1_00000000001011110010), site_id: 4, counter: 4};
        let uid5 = Uid{position: big(0b1_00000000001011111101), site_id: 4, counter: 4};
        let uid6 = Uid::max();

        let mut uids: Vec<&Uid> = vec![&uid4, &uid1, &uid5, &uid6, &uid0, &uid2, &uid3];
        uids.sort();

        assert!(uids[0] == &uid0);
        assert!(uids[1] == &uid1);
        assert!(uids[2] == &uid2);
        assert!(uids[3] == &uid2);
        assert!(uids[4] == &uid4);
        assert!(uids[5] == &uid5);
        assert!(uids[6] == &uid6);
    }

    #[test]
    fn test_between_trivial() {
        let uid1 = Uid::min();
        let uid2 = Uid::max();
        let uid  = Uid::between(&uid1, &uid2, DOT);

        assert!(big(0b1_00000000000000000000) < uid.position);
        assert!(big(0b1_11111111111111111111) > uid.position);
        assert!(uid.site_id == 3);
        assert!(uid.counter == 2);
    }

    #[test]
    fn test_between_basic() {
        let uid1 = Uid{position: big(0b1_01111111111111111110), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_10000000000000000000), site_id: 1, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position == big(0b1_01111111111111111111));
    }

    #[test]
    fn test_between_multi_level() {
        let uid1 = Uid{position: big(0b1_11111000000000000000), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_11111000000000000001), site_id: 1, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position > big(0b1_11111000000000000000_000000000000000000000));
        assert!(uid.position < big(0b1_11111000000000000000_000000000000000101001));
    }

    #[test]
    fn test_between_squeeze() {
        let uid1 = Uid{position: big(0b1_11111000000000000000_001101010010101010101_1010101010101010101010), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_11111000000000000000_001101010010101010111_1010101010101010101010), site_id: 1, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position == big(0b1_11111000000000000000_001101010010101010110));
    }

    #[test]
    fn test_between_equals() {
        let uid1 = Uid{position: big(0b1_00110011100000000010), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_00110011100000000010), site_id: 2, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position > big(0b1_00110011100000000010_000000000000000000000));
        assert!(uid.position < big(0b1_00110011100000000010_000000000000000101001));
    }

    #[test]
    fn test_between_first_is_shorter() {
        let uid1 = Uid{position: big(0b1_11111000000000000000), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_11111000000000000000_001101010010101010101), site_id: 2, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position > big(0b1_11111000000000000000_000000000000000000000));
        assert!(uid.position < big(0b1_11111000000000000000_000000000000000101001));
    }

    #[test]
    fn test_between_first_is_longer() {
        let uid1 = Uid{position: big(0b1_11111000000000000000_001101010010101010110), site_id: 1, counter: 1};
        let uid2 = Uid{position: big(0b1_11111000000000000000), site_id: 2, counter: 1};
        let uid  = Uid::between(&uid1, &uid2, DOT);
        assert!(uid.position > big(0b1_11111000000000000000_001101010010101010110));
        assert!(uid.position < big(0b1_11111000000000000000_001101010010101111111));
    }

    #[test]
    fn test_to_from_string() {
        let uid = Uid{position: big(0b1_010_1010), site_id: 4, counter: 83};
        let serialized = uid.to_string();
        let deserialized = Uid::from_str(&serialized).unwrap();
        assert!(serialized == "gSoEUw");
        assert!(deserialized == uid);
    }

    #[test]
    fn test_serialize() {
        let uid1 = Uid{position: big(0b1_010_1010_01101_100110_1011011_10111011_011001101), site_id: 491, counter: 82035};
        let s_json = serde_json::to_string(&uid1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&uid1).unwrap();
        let uid2: Uid = serde_json::from_str(&s_json).unwrap();
        let uid3: Uid = rmp_serde::from_slice(&s_msgpack).unwrap();
        assert!(uid1 == uid2);
        assert!(uid1 == uid3);
    }

    #[test]
    fn test_serialize_deserialize_invalid() {
        let serialized = "bjad%%";
        let deserialized = Uid::from_str(&serialized);
        assert!(deserialized.is_err());
    }

    fn big(num: usize) -> BigUint {
        num.to_biguint().unwrap()
    }
}
