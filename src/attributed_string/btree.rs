use super::element::Element;
use error::Error;
use std::mem;

const B: usize = 6;
const MIN_LEN: usize = B - 1;
const CAPACITY: usize = 2 * B - 1;

struct BTree {
    root: Node,
}

struct Node {
    len: usize,
    leaf: bool,
    elements: Vec<Element>,
    children: Vec<Node>,
}

impl BTree {
    pub fn new(element: Element) -> Self {
        BTree{
            root: Node{
            len: element.len,
            leaf: true,
            elements: vec![element],
            children: vec![],
        }}
    }

    pub fn insert(&mut self, element: Element) {
        if self.root.is_full() {
            let new_root = Node{len: 0, leaf: false, elements: vec![], children: vec![]};
            let old_root = mem::replace(&mut self.root, new_root);
            self.root.len = old_root.len;
            self.root.children.push(old_root);
            self.root.split_child(0);
        }
        self.root.insert_nonfull(element);
    }

    pub fn search(&self, index: usize) -> Result<(&Node, usize), Error> {
        self.root.search(index)
    }
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
    pub fn split_child(&mut self, i: usize) {
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

    /// Insert a new element into a node that is not full.
    fn insert_nonfull(&mut self, elt: Element) {
        let mut pos = self.elements.binary_search(&elt).err().expect("Duplicate UID!");
        if self.leaf {
            self.elements.insert(pos, elt);
        } else {
            if self.children[pos].is_full() {
                self.split_child(pos);
                if elt > self.elements[pos] { pos += 1 }
            }
            self.children[pos].insert_nonfull(elt)
        }
    }

    pub fn is_full(&self) -> bool {
        self.elements.len() == CAPACITY
    }
}
