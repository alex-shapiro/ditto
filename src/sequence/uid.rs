//! A sequence UID is an unique identifier that maintains
//! a total order of sequential elements in a distributed
//! environment.
//!
//! ## Implementation Notes
//!
//! New UIDs are generated with a variant of the LSEQ
//! algorithm. It diverges from the original paper by
//! choosing allocation strategy deterministically rather
//! than according to a random coin flip. Randomized
//! allocation strategies cannot converge in a distributed
//! environment because replicas cannot reliably coordinate
//! strategies, and so UID bit size increases quickly with
//! the number of sequence elements. However, a
//! deterministic allocation strategy known by all replicas
//! ahead of time is inherently coordinated, and UID bit
//! size increases more slowly with the number of elements.

use error::Error;
use num::bigint::{BigUint, ToBigUint};
use num::cast::ToPrimitive;
use rand::distributions::{IndependentSample, Range};
use rand;
use Replica;
use rustc_serialize::base64::{self, ToBase64, FromBase64};
use std::cmp::{self, Ordering};
use std::fmt::{self, Debug};
use std::str::FromStr;
use vlq;

const BASE_LEVEL: usize = 3;
const MAX_LEVEL:  usize = 32;
const BOUNDARY:   usize = 10;

#[derive(Clone,PartialEq,Eq)]
pub struct UID {
    position: BigUint,
    pub site: u32,
    pub counter: u32,
}

impl UID {
    fn new(position: BigUint, site: u32, counter: u32) -> Self {
        UID{position: position, site: site, counter: counter}
    }

    pub fn set_replica(&mut self, replica: &Replica) {
        self.site = replica.site;
        self.counter = replica.counter;
    }

    pub fn min() -> Self {
        UID::new(big(1 << BASE_LEVEL), 0, 0)
    }

    pub fn max() -> Self {
        let position = big((1 << BASE_LEVEL+1) - 1);
        UID::new(position, u32::max_value(), u32::max_value())
    }

    pub fn between(uid1: &UID, uid2: &UID, replica: &Replica) -> Self {
        let ref position1       = uid1.position;
        let ref position2       = uid2.position;
        let mut position        = big(1);
        let mut significant_bits = 1;

        for level in BASE_LEVEL..(MAX_LEVEL+1) {
            significant_bits += level;
            let pos1 = UID::get_pos(position1, level, significant_bits).unwrap_or(0);
            let pos2 = UID::get_pos(position2, level, significant_bits).unwrap_or((1 << level) - 1);

            if pos1 + 1 < pos2 {
                let pos = UID::generate_pos(pos1, pos2, level);
                position = (position << level) + big(pos);
                return UID::new(position, replica.site, replica.counter);
            } else {
                position = (position << level) + big(pos1);
            }
        }
        panic!(format!("UID cannot have more than ({}) levels", MAX_LEVEL));
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
            let insignificant_bits = position.bits() - significant_bits;
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
            if UID::use_boundary_plus_strategy(level) {
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
        let mut rng = rand::thread_rng();
        range.ind_sample(&mut rng)
    }

    fn use_boundary_plus_strategy(level: usize) -> bool {
        level % 2 == 1
    }
}

fn big(num: usize) -> BigUint {
    num.to_biguint().unwrap()
}

impl PartialOrd for UID {
    fn partial_cmp(&self, other: &UID) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UID {
    fn cmp(&self, other: &UID) -> Ordering {
        let mut self_position = self.position.clone();
        let mut other_position = other.position.clone();

        let self_bits = self_position.bits();
        let other_bits = other_position.bits();

        // truncate
        if self_bits > other_bits {
            self_position = self_position >> (self_bits - other_bits);
        } else {
            other_position = other_position >> (other_bits - self_bits);
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
        } else if self.site < other.site {
            Ordering::Less
        } else if self.site > other.site {
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

impl Debug for UID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let uid = self.position.to_str_radix(2);
        write!(f, "<{}, {}, {}>", uid, self.site, self.counter)
    }
}

impl ToString for UID {
    fn to_string(&self) -> String {
        let mut vlq = vlq::encode_biguint(&self.position);
        vlq.append(&mut vlq::encode_u32(self.site));
        vlq.append(&mut vlq::encode_u32(self.counter));

        vlq.to_base64(base64::Config{
            char_set: base64::CharacterSet::Standard,
            newline: base64::Newline::LF,
            pad: false,
            line_length: None,
        })
    }
}

impl FromStr for UID {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.from_base64() {
            Ok(vlq) => {
                let (position, vlq_rest1) = try!(vlq::decode_biguint(&vlq));
                let (site, vlq_rest2)     = try!(vlq::decode_u32(&vlq_rest1));
                let (counter, _)          = try!(vlq::decode_u32(&vlq_rest2));
                Ok(UID{position: position, site: site, counter: counter})
            },
            Err(_) =>
                Err(Error::DeserializeSequenceUID),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::bigint::{BigUint, ToBigUint};
    use Replica;
    use std::str::FromStr;

    const REPLICA: Replica = Replica{site: 3, counter: 2};

    #[test]
    fn test_min() {
        let uid = UID::min();
        assert!(uid.position == big(0b1000));
        assert!(uid.site == 0);
        assert!(uid.counter == 0);
    }

    #[test]
    fn test_max() {
        let uid = UID::max();
        assert!(uid.position == big(0b1111));
        assert!(uid.site == 4294967295);
        assert!(uid.counter == 4294967295);
    }

    #[test]
    fn test_set_replica() {
        let mut uid = UID::min();
        let mut replica = Replica{site: 432, counter: 182012};
        uid.set_replica(&mut replica);
        assert!(uid.site == 432);
        assert!(uid.counter == 182012);
    }

    #[test]
    fn test_ord() {
        let uid1 = UID::min();
        let uid2 = UID{position: big(0b10000101), site: 8, counter: 382};
        let uid3 = UID{position: big(0b1010101), site: 1, counter: 5};
        let uid4 = UID{position: big(0b1010101), site: 1, counter: 5};
        let uid5 = UID{position: big(0b1101111), site: 4, counter: 4};
        let uid6 = UID{position: big(0b11011111), site: 4, counter: 4};
        let uid7 = UID::max();

        let mut uids: Vec<&UID> = vec![&uid5, &uid2, &uid6, &uid7, &uid1, &uid3, &uid4];
        uids.sort();

        assert!(uids[0] == &uid1);
        assert!(uids[1] == &uid2);
        assert!(uids[2] == &uid3);
        assert!(uids[3] == &uid3);
        assert!(uids[4] == &uid5);
        assert!(uids[5] == &uid6);
        assert!(uids[6] == &uid7);
    }

    #[test]
    fn test_between_trivial() {
        let uid1 = UID::min();
        let uid2 = UID::max();
        let uid  = UID::between(&uid1, &uid2, &REPLICA);

        assert!(big(0b1_000) < uid.position);
        assert!(big(0b1_111) > uid.position);
        assert!(uid.site == 3);
        assert!(uid.counter == 2);
    }

    #[test]
    fn test_between_basic() {
        let uid1 = UID{position: big(0b1_010), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_100), site: 1, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(uid.position == big(0b1011));
    }

    #[test]
    fn test_between_multi_level() {
        let uid1 = UID{position: big(0b1_100_0001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_100_0011), site: 1, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_100_0010) == uid.position);
    }

    #[test]
    fn test_between_squeeze() {
        let uid1 = UID{position: big(0b1_010_0100_00100), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_011_0101_00110), site: 1, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_010_0100_00101) == uid.position);
    }

    #[test]
    fn test_between_boundary_plus() {
        let uid1 = UID{position: big(0b1_001_0001_00001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_001_0001_11101), site: 1, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_001_0001_00001) < uid.position);
        assert!(big(0b1_001_0001_01101) > uid.position);
    }

    #[test]
    fn test_between_boundary_minus() {
        let uid1 = UID{position: big(0b1_001_0001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_001_1111), site: 1, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_001_0100) < uid.position);
        assert!(big(0b1_001_1111) > uid.position);
    }

    #[test]
    fn test_between_equals() {
        let uid1 = UID{position: big(0b1_001_0001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_001_0001), site: 2, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_001_0001_00000) < uid.position);
        assert!(big(0b1_001_0001_01011) > uid.position);
    }

    #[test]
    fn test_between_first_is_shorter() {
        let uid1 = UID{position: big(0b1_001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_001_0001), site: 2, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_001_0000_00000) < uid.position);
        assert!(big(0b1_001_0000_01011) > uid.position);
    }

    #[test]
    fn test_between_first_is_longer() {
        let uid1 = UID{position: big(0b1_001_0001), site: 1, counter: 1};
        let uid2 = UID{position: big(0b1_010), site: 2, counter: 1};
        let uid  = UID::between(&uid1, &uid2, &REPLICA);
        assert!(big(0b1_001_0100) < uid.position);
        assert!(big(0b1_001_1111) > uid.position);
    }

    #[test]
    fn test_to_from_string() {
        let uid = UID{position: big(0b1_010_1010), site: 4, counter: 83};
        let serialized = uid.to_string();
        let deserialized = UID::from_str(&serialized).unwrap();
        assert!(serialized == "gSoEUw");
        assert!(deserialized == uid);
    }

    #[test]
    fn test_serialize_deserialize_invalid() {
        let serialized = "bjad%%";
        let deserialized = UID::from_str(&serialized);
        assert!(deserialized.is_err());
    }

    fn big(num: usize) -> BigUint {
        num.to_biguint().unwrap()
    }
}
