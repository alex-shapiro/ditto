// //! A modified Rope data structure for efficiently manipulating
// //! AttributedStrings. All replacements occur in O(log N) time.

// use self::Node::{Nil, Leaf, Branch};
// use super::Element;
// use error::Error;
// use Replica;
// use sequence::uid::{self, UID};
// use std::cmp::max;

// pub struct Rope {
//     root: Node,
// }

// /// A `Node` can either be empty, a leaf, or a branch.
// enum Node {
//     Nil,
//     Leaf(Element),
//     Branch(Box<Internal>),
// }

// /// A `Branch` is a concatenation of two `Rope`s.
// /// `Branch`s are the internal nodes of the `Rope`'s tree.
// struct Internal {
//     len:         usize,
//     height:      usize,
//     left:        Node,
//     right:       Node,
//     left_bound:  UID,
//     right_bound: UID,
// }

// impl Rope {
//     pub fn new() -> Rope {
//         Rope{root: Node::leaf("", &*uid::MIN, &*uid::MAX, &Replica{site: 0, counter: 0})}
//     }

//     pub fn replace(&mut self, begin: usize, end: usize, string: &str, replica: &Replica) -> Result<(), Error> {
//         if (begin > end) || (end > self.root.len()) {
//             return Err(Error::InvalidIndex)
//         }

//         let mut op = UpdateAttributedString::new(vec![], vec![]);

//         let (l1, r, changeset1) = self.root.split(end, replica);
//         let (l2, deleted, changeset2) = l1.split(begin, replica);

//         let (l, )






//         // 1. Split the rope at the point where the delete begins; a split inside of an existing element leads to the creation of a new element. This should create two ropes (L1 and R1) and a changeset (CH1).
//         // 2. Split the rope at the index where the delete ends; again, a split inside of an existing element leads to the creation of a new element. This should create two ropes (L2 and R2) and a changeset (CH2).
//         // 3. Turn the new text into a rope, L3.
//         // 4. Join L1 and L3. This creates a new rope (L1') and a changeset (CH3).
//         // 5. Join L1' and R2 -> R.
//         // 6. Merge CH1, CH2, and CH3 -> CH.
//         // 7. Return (R, CH).




//     }
// }

// impl<'a> From<&'a str> for Rope {
//     fn from(string: &str) -> Rope {
//         match string.len() {
//             0 => Rope::new(),
//             _ => Rope{root: Node::leaf(string, &*uid::MIN, &*uid::MAX, &Replica{site: 0, counter: 0})},
//         }
//     }
// }

// impl Node {
//     fn leaf(string: &str, uid1: &UID, uid2: &UID, replica: &Replica) -> Node {
//         let element = Element::between_uids(uid1, uid2, string, replica);
//         Node::Leaf(element)
//     }

//     fn len(&self) -> usize {
//         match *self {
//             Nil => 0,
//             Leaf(ref element) => element.len,
//             Branch(ref internal) => internal.len,
//         }

//     }

//     fn height(&self) -> usize {
//         match *self {
//             Nil => 0,
//             Leaf(_) => 1,
//             Branch(ref internal) => internal.height,
//         }

//     }

//     fn concat(left: Node, right: Node) -> Node {
//         match (left, right) {
//             (Nil, r) => r,
//             (l, Nil) => l,
//             (l, r) => Branch(Box::new(Internal::new(l, r)))
//         }
//     }

//     fn split(self, index: usize, replica: &Replica, op: &mut UpdateAttributedString) -> (Node, Node) {
//         if index == 0 {
//             return (Nil, self)
//         } else if index == self.len() {
//             return (self, Nil)
//         } else {
//             match self {
//                 Nil => (Nil, Nil),
//                 Leaf(element) => {
//                     let (left, right) = element.split

//                     let right = s.chars


//                 }
//             }
//         }
//     }
// }

// impl Internal {
//     fn new(left: Node, right: Node) -> Self {
//         Internal {
//             len: left.len() + right.len(),
//             height: max(left.height(), right.height()) + 1,
//             left: left,
//             right: right,
//         }
//     }
// }
