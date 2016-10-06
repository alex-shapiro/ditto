use std::cmp;
use rand;
use rand::distributions::{IndependentSample, Range};
use Site;

static BASE_LEVEL: u32 = 3;
static BOUNDARY: u32 = 10;

#[derive(Clone, PartialEq, PartialOrd)]
pub struct PathElt {
    pub index: u32,
    pub site: u32,
}

impl PathElt {
    pub fn new(index: u32, site: Site) -> PathElt {
        PathElt{index: index, site: site}
    }

    pub fn between(index1: u32, index2: u32, level: u32, site: Site) -> PathElt {
        let index = generate_index(index1, index2, level);
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

/// Creates a new path that is in between path1 and path2,
/// where path1 < path2. Uses LSEQ algorithm.
pub fn between(path1: &Path, path2: &Path, site: Site) -> Path {
    let mut new_path: Path = vec![];
    let mut path1 = path1.iter();
    let mut path2 = path2.iter();
    let mut level = BASE_LEVEL;

    loop {
        let elt1 = path1.next();
        let elt2 = path2.next();

        if elt1.is_some() && elt2.is_some() {
            let e1 = elt1.unwrap();
            let e2 = elt2.unwrap();
            if e1.index + 1 < e2.index {
                let element = PathElt::between(e1.index, e2.index, level, site);
                new_path.push(element);
                break;
            } else if e1.site < site && site < e2.site {
                let element = PathElt::new(e1.index, site);
                new_path.push(element);
                break;
            } else {
                let element: PathElt = e1.clone();
                new_path.push(element);
                level += 1;
            }

        } else if elt1.is_some() {
            let e1 = elt1.unwrap();
            let e2_index = 2u32.pow(level);
            if e1.index + 1 < e2_index {
                let element = PathElt::between(e1.index, e2_index, level, site);
                new_path.push(element);
                break;
            } else {
                let element = e1.clone();
                new_path.push(element);
                level += 1;
            }

        } else if elt2.is_some() {
            let e2 = elt2.unwrap();
            if e2.index > 1 {
                let element = PathElt::between(0, e2.index, level, site);
                new_path.push(element);
                break;
            } else {
                let element = PathElt::new(0,0);
                new_path.push(element);
                level += 1;
            }

        } else {
            let element = PathElt::between(0, 2u32.pow(level), level, site);
            new_path.push(element);
            break;
        }
    }
    new_path
}

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

#[cfg(test)]
mod tests {
    use Site;
    use super::PathElt;
    use super::Path;

    fn gen_path(prototype: Vec<(u32, Site)>) -> Path {
        prototype.iter().map(|&(index, site)| PathElt::new(index,site)).collect()
    }

    #[test]
    fn test_generate_index() {
        let i1 = super::generate_index(0,8,3);
        println!("i1 is {}", i1);
        assert!(1 <= i1 && i1 <= 7);

        let i2 = super::generate_index(0,16,4);
        println!("i2 is {}", i2);
        assert!(6 <= i2 && i2 <= 15);

        let i3 = super::generate_index(4,6,4);
        println!("i3 is {}", i3);
        assert!(i3 == 5);
    }

    #[test]
    fn test_between_1() {
        let path1 = gen_path(vec![(1,3),(2,4)]);
        let path2 = gen_path(vec![(1,3),(15,3)]);
        let path  = super::between(&path1, &path2, 12);
        assert!(path[0] == PathElt{index: 1, site: 3});
        assert!(5 <= path[1].index && path[1].index <= 14);
        assert!(path[1].site == 12);
    }

    #[test]
    fn test_between_2() {
        let path1 = gen_path(vec![(5,1)]);
        let path2 = gen_path(vec![(5,3)]);
        let path  = super::between(&path1, &path2, 2);
        assert!(path == gen_path(vec![(5,2)]));
    }

    #[test]
    fn test_between_3() {
        let path1 = gen_path(vec![(5,1),(8,2)]);
        let path2 = gen_path(vec![(5,2)]);
        let path  = super::between(&path1, &path2, 2);
        assert!(9 <= path[1].index && path[1].index <= 15);
    }

    #[test]
    fn test_between_4() {
        let path1 = gen_path(vec![(5,1)]);
        let path2 = gen_path(vec![(5,2),(1,1)]);
        let path  = super::between(&path1, &path2, 2);
        assert!(path[1] == PathElt{index: 0, site: 0});
        assert!(1 <= path[2].index && path[2].index <= 10);
    }
}
