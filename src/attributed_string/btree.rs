use super::element::Element;
use error::Error;

struct Node{
    len: usize,
    height: usize,
    value: NodeVal,
}

enum NodeVal {
    Leaf(Element),
    Internal(Vec<Node>),
}


impl Node {
    fn is_leaf(&self) -> bool {
        self.height == 0
    }

    fn children(&self) -> &[Node] {
        match self.value {
            NodeVal::Internal(ref children) => children,
            NodeVal::Leaf(_) => panic!("can't get children of a leaf!")
        }
    }

    /// finds the node that contains `index`.
    /// If the index is out of bounds, returns an error.
    /// Otherwise returns a reference to the node and
    /// the offset of the index inside the node.
    pub fn search(&self, index: usize) -> Result<(&Node, usize), Error> {
        if index >= self.len { return Err(Error::OutOfBounds) }

        let mut i = index;
        for child in self.children() {
            if i >= child.len {
                i -= child.len
            } else if child.is_leaf() {
                return Ok((&child, i))
            } else {
                return child.search(i);
            }
        }

        unreachable!()
    }
}
