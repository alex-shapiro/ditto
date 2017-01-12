use super::element::Element;
use error::Error;
use std::mem;

const B: usize = 6;
const MIN_LEN: usize = B - 1;
const CAPACITY: usize = 2 * B - 1;

struct Node {
    len: usize,
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
        if index >= self.len { return Err(Error::OutOfBounds) }

        let mut i = index;
        for child in &self.children {
            if i >= child.len {
                i -= child.len
            } else if child.leaf {
                return Ok((&child, i))
            } else {
                return child.search(i);
            }
        }

        unreachable!()
    }

    /// Split the node's ith child in half. The original child MUST
    /// be full (ie, it contains 2B-1 elements and 2B children).
    /// Each new child has B-1 elements and B children. The original
    /// child's median element is promoted as the ith element of the
    /// parent node.
    fn split_child(&mut self, i: usize) {
        let (median, new_child) = {
            let ref mut child = self.children[i];
            let mut elements = child.elements.split_off(MIN_LEN);
            let mut children = match child.leaf {
                true  => vec![],
                false => child.children.split_off(B),
            };

            let median = elements.remove(0);

            let mut new_child_len = elements.iter().map(|e| e.len).sum();
            new_child_len += children.iter().map(|e| e.len).sum();
            let mut new_child = Node{
                len: new_child_len,
                leaf: child.leaf,
                elements: elements,
                children: children,
            };

            child.len -= new_child.len + median.len;
            (median, new_child)
        };

        self.elements.insert(i, median);
        self.children.insert(i + 1, new_child);
    }
}
