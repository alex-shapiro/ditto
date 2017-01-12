use super::element::Element;
use error::Error;

const B: usize = 6;
const MIN_LEN: usize = B - 1;
const CAPACITY: usize = 2 * B - 1;

struct Node {
    chars: usize,
    leaf: bool,
    elements: Vec<Element>,
    children: Vec<Node>,
}

impl Node {
    /// finds the node that contains `index`.
    /// If the index is out of bounds, returns an error.
    /// Otherwise returns a reference to the node and
    /// the offset of the index inside the node.
    pub fn search(&self, index: usize) -> Result<(&Node, usize), Error> {
        if index >= self.chars { return Err(Error::OutOfBounds) }

        let mut i = index;
        for child in &self.children {
            if i >= child.chars {
                i -= child.chars
            } else if child.leaf {
                return Ok((&child, i))
            } else {
                return child.search(i);
            }
        }

        unreachable!()
    }
}
