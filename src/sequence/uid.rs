use std::cmp;
use std::cmp::Ordering;
use rand;
use rand::distributions::{IndependentSample, Range};
use num::BigUint;
use Replica;

const BASE_LEVEL: u32 = 3;
const MAX_LEVEL:  u32 = 32;
const BOUNDARY:   u32 = 10;

#[derive(Clone,PartialEq,Eq,Ord)]
pub struct UID {
    positions: BigUint,
    site: u32,
    counter: u32,
}

impl UID {
    pub fn min() -> Self {
        UID{positions: BigUint::zero(), site: 0, counter: 0}
    }

    pub fn max() -> Self {
        let positions = BigUint::from(1 << BASE_LEVEL);
        UID{positions: positions, site: 0, counter: 0}
    }

    /// Creates a new UID that falls between uid1 and uid2
    pub fn between(uid1: &UID, uid2: &UID, replica: &Replica) -> Self {
        let mut positions1 = uid1.positions;
        let mut positions2 = uid2.positions;
        let mut positions  = BigUint::zero();

        for level in BASE_LEVEL..(MAX_LEVEL+1) {
            let pos1 = get_pos(positions1, level).unwrap_or(BigUint::zero());
            let pos2 = get_pos(positions2, level).unwrap_or(BigUint::from(1 << level));

            if pos1 + 1 < pos2 {
                let pos = generate_pos(pos1, pos2, level);
                positions = (positions << level) + pos;
                return UID{
                    positions: positions,
                    site: replica.site,
                    counter: replica.counter,
                }
            } else {
                positions = (positions << level) + pos1;
                positions1 >>= level;
                positions2 >>= level;
            }
        }
        panic!("Can't have more than 32 levels in a UID!");
    }
}

/// Gets the next position in a positions BigUint, if
/// the next position exists. The lowest position is
/// the `level` lowest bits in the BigUint.
fn get_pos(positions: &BigUint, level: u32) -> Option<u32> {
    let bits = positions.bits();
    if bits >= level {
        let mut mask = (BigUint::zero() << level+1) - 1;
        let pos: u32 = positions & mask;
        Some(pos)
    } else {
        None
    }
}

/// Generates an position that falls between pos1 and pos2.
/// Uses either boundary+ or boundary- strategy.
///
/// boundary+ is used on odd levels and returns an integer
/// from interval [pos1+1, min(pos1+BOUNDARY, pos2-1)]
///
/// boundary- is used on even levels and returns an integer
/// from interval [max(pos1+1, pos2-BOUNDARY), pos2-1]
///
fn generate_pos(pos1: u32, pos2: u32, level: u32) -> u32 {
    let range =
        if use_plus_strategy(level) {
            let lo_bound = pos1+1;
            let hi_bound = cmp::min(pos1+BOUNDARY, pos2);
            let diff = hi_bound - lo_bound;

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

fn use_plus_strategy(level: u32) -> bool {
    level % 2 == 1
}

impl PartialOrd for UID {
    fn partial_cmp(&self, other: &UID) -> Option<Ordering> {
        let mut self_positions = self.positions;
        let mut other_positions = other.positions;

        let self_bits = self_positions.bits();
        let other_bits = other_positions.bits();

        // truncate
        if self_bits > other_bits {
            self_positions >>= self_bits - other_bits;
        } else {
            other_positions >>= other_bits - self_bits;
        }

        if self_positions < other_positions {
            Some(Ordering::Less)
        } else if self_positions > other_positions {
            Some(Ordering::Greater)
        } else if self_bits < other_bits {
            Some(Ordering::Less)
        } else if self_bits > other_bits {
            Some(Ordering::Greater)
        } else if self.site < other.site {
            Some(Ordering::Less)
        } else if self.site > other.site {
            Some(Ordering::Greater)
        } else if self.counter < other.counter {
            Some(Ordering::Less)
        } else if self.counter > other.counter {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}
