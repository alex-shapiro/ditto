// //! A Rope for efficiently storing and manipulating large amounts of attributed text

// use super::Element;

// /// A `Rope` is a tree of `Element`s that allows more efficient storage and
// /// manipulation of large amounts of attributed text than a `Vec`.
// pub struct Rope {
//     root: Node,
// }

// /// A `Node` can either be empty, a leaf, or a branch.
// enum Node {
//     Leaf(Element),
//     Branch(Box<Branch>),
// }

// /// A `Branch` is a concatenation of two `Rope`s.
// /// `Branch`s are the internal nodes of the `Rope`'s tree.
// struct Branch {
//     len:    usize,
//     height: usize,
//     left:   Node,
//     right:  Node,
// }

// impl Rope {
//     pub fn new() -> Rope {
//         Rope{root: Node::leaf("")}
//     }

//     fn append(&mut self, Rope: Rope) {
//     }

//     fn prepend(&mut self, Rope: Rope) {
//     }

//     fn insert(&mut self, rope: Rope) {
//     }

//     fn delete(&mut self, rope: Rope) {
//     }

//     fn replace(&mut self, rope: Rope) {
//     }
// }

// impl From<&str> for Rope {
//     fn from(string: &str) -> Rope {
//         match string.len() {
//             0 => Rope::new(),
//             _ => Rope{root: Node::leaf(string)},
//         }
//     }
// }

// impl Node {
//     fn leaf(string: &str) -> Node {
//         let uid1 = UID::min();
//         let uid2 = UID::max();

//         let element = Element::between(string.)
//     }
// }
