use std::cmp;
use rand;
use rand::distributions::{IndependentSample, Range};
use Site;

static BASE_LEVEL: u32 = 3;
static BOUNDARY: u32 = 10;

struct PathElt {
    pub index: u32,
    pub site: u32,
}

impl PathElt {
    pub fn new(index: u32, site: Site) -> PathElt {
        PathElt{index: index, site: site}
    }
}

pub type Path = Vec<PathElt>;

/// The minimum path for any sequence
pub fn min() -> Path {
    vec![PathElt::new(0, 0)]
}

/// The maximum path for any sequence
pub fn max() -> Path {
    vec![PathElt::new(2u32.pow(BASE_LEVEL), 0)]
}

// /// Creates a new path that is in between path1 and path2,
// /// where path1 comes before path2. Uses LSEQ algorithm.
// pub fn between(path1: &Path, path2: &Path, site: Site) {
//     let mut new_path: Path = vec![];
//     let p1 = path1.iter();
//     let p2 = path2.iter();

//     loop {
//         let e1 = p1.next();
//         let e2 = p2.next();
//         if e1.is_some() && e2.is_some() {

//         } else if e1.is_some() {

//         } else if e2.is_some() {

//         } else {
//             let new_element = PathElement::new()
//             new_path.push()
//         }
//     }
// }

/// Generates an index that falls between index1 and index2.
/// Uses either boundary+ or boundary- strategy.
///
/// boundary+ is used on odd levels and returns an integer
/// from interval [index1+1, min(index1+BOUNDARY, index2-1)]
///
/// boundary- is used on even levels and returns an integer
/// from interval [max(index1+1, index2-BOUNDARY), index2-1]
///
fn generate_index(index1: u32, index2: u32, level: u32) -> u32 {
    let range =
        if use_plus_strategy(level) {
            let lo_bound = index1 + 1;
            let hi_bound = cmp::min(index1+BOUNDARY, index2);
            Range::new(lo_bound, hi_bound)
        } else if index2 <= BOUNDARY {
            let lo_bound = index1+1;
            let hi_bound = index2;
            Range::new(lo_bound, hi_bound)
        } else {
            let lo_bound = cmp::max(index1+1, index2-BOUNDARY);
            let hi_bound = index2;
            Range::new(lo_bound, hi_bound)
        };
    let mut rng = rand::thread_rng();
    range.ind_sample(&mut rng)
}

fn use_plus_strategy(level: u32) -> bool {
    level % 2 == 1
}

#[test]
fn test_generate_index() {
    let i1 = generate_index(0,8,3);
    println!("i1 is {}", i1);
    assert!(1 <= i1 && i1 <= 7);

    let i2 = generate_index(0,16,4);
    println!("i2 is {}", i2);
    assert!(6 <= i2 && i2 <= 15);

    let i3 = generate_index(4,6,4);
    println!("i3 is {}", i3);
    assert!(i3 == 5);
}
