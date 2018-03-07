//! A variant of a Tree that supports four operations:
//!
//! * `insert(e)` inserts an element into the tree.
//! * `remove(id)` removes an element from the tree.
//! * `lookup(id)` finds an element in the tree.
//! * `get_elt(idx)` finds the tree element at index i and the
//!   distance from its beginning to idx.
//! * `get_idx(id)` finds the start index of the tree element
//!   with id == x.
//!
//! All operations can be performed in O(log n) time.

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use std::mem;
use std::iter::FromIterator;

const B: usize = 6;
const MIN_LEN: usize = B - 1;
const CAPACITY: usize = 2 * B - 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Tree<T> {
    root: Node<T>,
}

#[derive(Debug, Clone, PartialEq)]
struct Node<T> {
    len: usize,
    elements: Vec<T>,
    children: Vec<Node<T>>,
}

pub trait Element {
    type Id: Clone + PartialEq + Ord;

    fn id(&self) -> &Self::Id;

    fn element_len(&self) -> usize { 1 }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    OutOfBounds,
    DuplicateId,
}

impl<T: Element> Tree<T> {
    /// Constructs a new, empty Tree.
    pub fn new() -> Self {
        Tree{root: Node::new()}
    }

    /// Returns the length of the tree.
    pub fn len(&self) -> usize {
        self.root.len
    }

    /// Returns true if the tree has length 0.
    /// Returns false otherwise.
    pub fn is_empty(&self) -> bool {
        self.root.len == 0
    }

    /// Inserts an element into the tree or returns an error
    /// if the contains an element with the same id. Each
    /// element must conform to the Element trait, which reuires
    /// the element to have a unique, totally ordered id and
    /// a length. The default element length is 1.
    pub fn insert(&mut self, element: T) -> Result<(), Error> {
        if self.root.is_full() {
            let old_root = mem::replace(&mut self.root, Node::new());
            self.root.len = old_root.len;
            self.root.children.push(old_root);
            self.root.split_child(0);
        }
        self.root.insert(element)
    }

    /// Deletes an element from the tree. Returns None if the
    /// element is not in the tree. This allows the CRDT to handle
    /// duplicate operations without losing consistency.
    pub fn remove(&mut self, id: &T::Id) -> Option<T> {
        self.root.remove(id)
    }

    /// Returns a refence to an element from the tree,
    /// or None if the element is not in the tree.
    pub fn lookup(&self, id: &T::Id) -> Option<&T> {
        self.root.lookup(id)
    }

    /// Returns a mutable refence to an element from the tree,
    /// or None if the element is not in the tree.
    pub fn lookup_mut(&mut self, id: &T::Id) -> Option<&mut T> {
        self.root.lookup_mut(id)
    }

    /// Returns the element that contains the character at
    /// location `index`, as well as the offset of `index`
    /// within the element. Returns an error if the index
    /// is out-of-bounds.
    pub fn get_elt(&self, idx: usize) -> Result<(&T, usize), Error> {
        if idx >= self.len() { return Err(Error::OutOfBounds) }
        Ok(self.root.get_elt(idx))
    }

    pub fn get_mut_elt(&mut self, idx: usize) -> Result<(&mut T, usize), Error> {
        if idx >= self.len() { return Err(Error::OutOfBounds) }
        Ok(self.root.get_mut_elt(idx))
    }

    /// Returns the start index of a tree element, or None
    /// if the element does not exist.
    pub fn get_idx(&self, id: &T::Id) -> Option<usize> {
        self.root.get_idx(id)
    }

    /// Returns an iterator that visits the tree elements in
    /// ascending order.
    pub fn iter(&self) -> Iter<T> {
        self.into_iter()
    }

    pub fn into_iter(self) -> IntoIter<T> {
        <Self as IntoIterator>::into_iter(self)
    }
}

impl<T: Element> Node<T> {

    fn new() -> Self {
        Node{len: 0, elements: vec![], children: vec![]}
    }

    fn lookup(&self, id: &T::Id) -> Option<&T> {
        match self.elements.binary_search_by(|elt| elt.id().cmp(id)) {
            Ok(idx) => self.elements.get(idx),
            Err(idx) => {
                if self.is_internal() {
                    self.children[idx].lookup(id)
                } else {
                    None
                }
            }
        }
    }

    fn lookup_mut(&mut self, id: &T::Id) -> Option<&mut T> {
        match self.elements.binary_search_by(|elt| elt.id().cmp(id)) {
            Ok(idx) => self.elements.get_mut(idx),
            Err(idx) => {
                if self.is_internal() {
                    let child = &mut self.children[idx];
                    child.lookup_mut(id)
                } else {
                    None
                }
            }
        }
    }

    fn get_elt(&self, mut idx: usize) -> (&T, usize) {
        if self.is_leaf() {
            for element in &self.elements {
                if idx < element.element_len() { return (element, idx) }
                else { idx -= element.element_len() }
            }
        } else {
            let mut elements = self.elements.iter();
            for child in &self.children {
                if idx < child.len { return child.get_elt(idx) }
                else { idx -= child.len }
                if let Some(element) = elements.next() {
                    if idx < element.element_len() { return (element, idx) }
                    else { idx -= element.element_len() }
                }
            }
        }
        unreachable!();
    }

    fn get_mut_elt(&mut self, mut idx: usize) -> (&mut T, usize) {
        if self.is_leaf() {
            for element in &mut self.elements {
                if idx < element.element_len() { return (element, idx) }
                else { idx -= element.element_len() }
            }
        } else {
            let mut elements = self.elements.iter_mut();
            for child in &mut self.children {
                if idx < child.len { return child.get_mut_elt(idx) }
                else { idx -= child.len }
                if let Some(element) = elements.next() {
                    if idx < element.element_len() { return (element, idx) }
                    else { idx -= element.element_len() }
                }
            }
        }
        unreachable!();
    }

    fn get_idx(&self, id: &T::Id) -> Option<usize> {
        let (contains_element, idx) =
            match self.elements.binary_search_by(|elt| elt.id().cmp(id)) {
                Ok(idx) => (true, idx),
                Err(idx) => (false, idx),
            };

        let mut char_idx = self.elements[..idx].iter().map(|e| e.element_len()).sum();
        if self.is_leaf() && contains_element {
            Some(char_idx)
        } else if self.is_leaf() {
            None
        } else if contains_element {
            char_idx += self.children[..idx+1].iter().map(|node| node.len).sum::<usize>();
            Some(char_idx)
        } else {
            char_idx += self.children[..idx].iter().map(|node| node.len).sum::<usize>();
            match self.children[idx].get_idx(id) {
                Some(sub_idx) => Some(char_idx + sub_idx),
                None => None,
            }
        }
    }

    /// Insert a new element into a tree. The root node must
    /// not be full (ie it must contain fewer than CAPACITY
    /// elements)
    fn insert(&mut self, elt: T) -> Result<(), Error> {
        let mut idx = {
            let id = elt.id();
            self.elements
                .binary_search_by(|e| e.id().cmp(id))
                .err().ok_or(Error::DuplicateId)?
        };

        let elt_len = elt.element_len();

        if self.is_leaf() {
            self.elements.insert(idx, elt);
        } else {
            if self.children[idx].is_full() {
                self.split_child(idx);
                if elt.id() > self.elements[idx].id() { idx += 1 }
            }
            self.children[idx].insert(elt)?;
        }

        self.len += elt_len;
        Ok(())
    }

    /// Delete an element from a tree, returning the removed element.
    /// The root node must contain at least MIN_LEN + 1 elements.
    fn remove(&mut self, id: &T::Id) -> Option<T> {
        let (contains_element, idx) =
            match self.elements.binary_search_by(|elt| elt.id().cmp(id)) {
                Ok(idx) => (true, idx),
                Err(idx) => (false, idx),
            };

        // if the parent is a leaf and it contains the element,
        // simply remove the element.
        if self.is_leaf() && contains_element {
            let removed_element = self.elements.remove(idx);
            self.len -= removed_element.element_len();
            Some(removed_element)

        // if the parent is a leaf and does not contain the element
        // then the element does not exist in the tree.
        } else if self.is_leaf() {
            None

        // if the parent is internal and it contains the element,
        // remove the element from the parent and rebalance from
        // either the child node to either the left or right of
        // the element.
        } else if contains_element {
            if self.child_has_spare_element(idx) {
                let prev = &mut self.children[idx];
                let predecessor_id = prev.last_id();
                let e = prev.remove(&predecessor_id).expect("Element must exist B!");
                let removed_element = mem::replace(&mut self.elements[idx], e);
                self.len -= removed_element.element_len();
                Some(removed_element)

            } else if self.child_has_spare_element(idx+1) {
                let next = &mut self.children[idx+1];
                let successor_id = next.first_id();
                let e = next.remove(&successor_id).expect("Element must exist C!");
                let removed_element = mem::replace(&mut self.elements[idx], e);
                self.len -= removed_element.element_len();
                Some(removed_element)

            } else {
                self.merge_children(idx);
                self.remove(id)
            }

        // if the parent is internal and does not contain the element
        // then call recursively on the correct child node. Before
        // the call, check that child has MIN_LEN + 1 elements. If not,
        // rebalance from the child's left and right siblings.
        } else {
            if !self.child_has_spare_element(idx) {
                if idx > 0 && self.child_has_spare_element(idx-1) {
                    let (sibling_elt, sibling_child) = self.children[idx-1].pop_last();
                    let parent_elt = mem::replace(&mut self.elements[idx-1], sibling_elt);
                    let child = &mut self.children[idx];
                    child.len += parent_elt.element_len();
                    child.elements.insert(0, parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.insert(0, c);
                    }
                } else if self.child_has_spare_element(idx+1) {
                    let (sibling_elt, sibling_child) = self.children[idx+1].pop_first();
                    let parent_elt = mem::replace(&mut self.elements[idx], sibling_elt);
                    let child = &mut self.children[idx];
                    child.len += parent_elt.element_len();
                    child.elements.push(parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.push(c);
                    }
                } else {
                    let mut idx = idx;
                    if idx == self.children.len() - 1 { idx -= 1 }
                    self.merge_children(idx);
                    return self.remove(id)
                }
            }
            let element = self.children[idx].remove(id);
            if let Some(ref e) = element { self.len -= e.element_len() }
            element
        }
    }

    /// Split the node's ith child in half. The original child MUST
    /// be full (ie, it contains 2B-1 elements and 2B children).
    /// Each new child has B-1 elements and B children. The original
    /// child's median element is promoted as the ith element of the
    /// parent node.
    fn split_child(&mut self, child_idx: usize) {
        let (median, new_child) = {
            let child = &mut self.children[child_idx];

            let elements = child.elements.split_off(B);
            let median   = child.elements.pop().expect("Element must exist A!");
            let children = if child.is_leaf() { vec![] } else { child.children.split_off(B) };

            let new_child_len =
                elements.iter().fold(0, |sum, e| sum + e.element_len()) +
                children.iter().fold(0, |sum, e| sum + e.len);

            let new_child = Node{len: new_child_len, elements, children};

            child.len -= new_child_len + median.element_len();

            (median, new_child)
        };

        self.elements.insert(child_idx, median);
        self.children.insert(child_idx + 1, new_child);
    }

    /// Merge the node's ith and (i+1)th children, then remove the median
    /// separating them as well as the (i+1)th node. The new node MUST
    /// contain 2B-1 elements.
    fn merge_children(&mut self, idx: usize) {
        let removed_element = self.elements.remove(idx);
        let Node{len, mut elements, mut children, ..} = self.children.remove(idx+1);

        {
            let child = &mut self.children[idx];
            child.len += removed_element.element_len() + len;
            child.elements.push(removed_element);
            child.elements.append(&mut elements);
            child.children.append(&mut children);
        }

        // When merging children of the root node it is possible for
        // root to end up with 0 elements and 1 child. When this happens,
        // replace the root node with its only child.
        if self.elements.is_empty() {
            let child = self.children.remove(0);
            mem::replace(self, child);
        }
    }

    fn pop_first(&mut self) -> (T, Option<Self>) {
        let element = self.elements.remove(0);
        self.len -= element.element_len();
        if self.is_internal() {
            let child = self.children.remove(0);
            self.len -= child.len;
            (element, Some(child))
        } else {
            (element, None)
        }
    }

    fn pop_last(&mut self) -> (T, Option<Self>) {
        let element = self.elements.pop().expect("Element must exist!");
        self.len -= element.element_len();
        if let Some(child) = self.children.pop() {
            self.len -= child.len;
            (element, Some(child))
        } else {
            (element, None)
        }
    }

    #[inline]
    fn child_has_spare_element(&self, child_idx: usize) -> bool {
        match self.children.get(child_idx) {
            Some(child) => child.elements.len() > MIN_LEN,
            None => false,
        }
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.elements.len() == CAPACITY
    }

    #[inline]
    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    #[inline]
    fn is_internal(&self) -> bool {
        !self.children.is_empty()
    }

    fn first_id(&self) -> T::Id {
        let mut node = self;
        while node.is_internal() { node = &node.children[0] }
        node.elements[0].id().to_owned()
    }

    fn last_id(&self) -> T::Id {
        let mut node = self;
        while node.is_internal() { node = node.children.last().expect("Child must exist!") }
        node.elements.last().expect("Element must exist E!").id().to_owned()
    }
}

pub struct Iter<'a, T: 'static> {
    stack: Vec<(&'a Node<T>, usize)>,
    node: &'a Node<T>,
    next_idx: usize,
}

impl<'a, T: 'static + Element> IntoIterator for &'a Tree<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        let mut node = &self.root;
        let mut stack = vec![];
        while node.is_internal() {
            stack.push((node, 0));
            node = &node.children[0];
        }
        Iter{stack, node, next_idx: 0}
    }
}

impl<'a, T: Element> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(element) = self.node.elements.get(self.next_idx) {
                self.next_idx += 1;
                while self.node.is_internal() {
                    self.stack.push((self.node, self.next_idx));
                    self.node = &self.node.children[self.next_idx];
                    self.next_idx = 0;
                }
                return Some(element)
            } else if let Some((node, next_idx)) = self.stack.pop() {
                self.node = node;
                self.next_idx = next_idx;
            } else {
                return None
            }
        }
    }
}

pub struct IntoIter<T: 'static> {
    tree: Tree<T>,
}

impl<T: 'static + Element> IntoIterator for Tree<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter{tree: self}
    }
}

impl<T: Element> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let id = if let Ok((elt, _)) = self.tree.get_elt(0) {
            elt.id().clone()
        } else {
            return None
        };

        self.tree.remove(&id)
    }
}

impl<'a, T: Element> FromIterator<T> for Tree<T> {
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> Self {
        let mut tree: Tree<T> = Tree::new();

        for element in iter {
            let _ = tree.insert(element);
        }

        tree
    }
}

impl<T: 'static + Element + Serialize> Serialize for Tree<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.iter().collect::<Vec<&T>>().serialize(serializer)
    }
}

impl<'de, T: Element + Deserialize<'de>> Deserialize<'de> for Tree<T> {
    fn deserialize<D>(deserializer: D) -> Result<Tree<T>, D::Error> where D: Deserializer<'de> {
        Ok(Vec::deserialize(deserializer)?.into_iter().collect::<Tree<T>>())
    }
}

impl<T: Element> Default for Tree<T> {
    fn default() -> Self {
        Tree::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::{self, Rng};

    #[derive(Debug, Clone, PartialEq)]
    struct TextElement {
        id: u64,
        text: String,
    }

    impl Element for TextElement {
        type Id = u64;

        fn id(&self) -> &Self::Id {
            &self.id
        }

        fn element_len(&self) -> usize {
            self.text.chars().count()
        }
    }

    #[test]
    fn test_new() {
        let tree: Tree<TextElement> = Tree::new();
        assert!(tree.len() == 0);
        assert!(tree.root.elements.is_empty());
        assert!(tree.root.children.is_empty());
    }

    #[test]
    fn get_elt_out_of_bounds() {
        let tree: Tree<TextElement> = Tree::new();
        assert!(tree.get_elt(1) == Err(Error::OutOfBounds));
    }

    #[test]
    fn get_elt_empty() {
        let tree: Tree<TextElement> = Tree::new();
        assert!(tree.get_elt(0) == Err(Error::OutOfBounds));
    }

    #[test]
    fn get_elt_nonempty() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 104, "hello");
        insert(&mut tree, 401, "world");

        let (elt, offset) = tree.get_elt(0).expect("Not out of bounds!");
        assert!(elt.text == "hello");
        assert!(offset == 0);

        let (elt, offset) = tree.get_elt(4).expect("Not out of bounds!");
        assert!(elt.text == "hello");
        assert!(offset == 4);

        let (elt, offset) = tree.get_elt(5).expect("Not out of bounds!");
        assert!(elt.text == "world");
        assert!(offset == 0);

        let (elt, offset) = tree.get_elt(7).expect("Not out of bounds!");
        assert!(elt.text == "world");
        assert!(offset == 2);

        assert!(tree.get_elt(10).unwrap_err() == Error::OutOfBounds);
    }

    #[test]
    fn get_idx_does_not_exist() {
        let tree: Tree<TextElement> = Tree::new();
        let id = 510423 as u64;
        assert!(tree.get_idx(&id).is_none());
    }

    #[test]
    fn get_idx_element_exists() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 58391, "hi");
        insert(&mut tree, 50174912, "there");

        let id = tree.root.elements[1].id;
        assert!(tree.get_idx(&id) == Some(2));
    }

    #[test]
    fn insert_basic() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 999, "the");

        assert!(tree.len() == 3);
        assert!(tree.root.is_leaf());
        assert!(tree.root.elements[0].text == "the");
    }

    #[test]
    fn insert_duplicate() {
        let mut tree: Tree<TextElement> = Tree::new();

        for i in 0..300 {
            insert(&mut tree, i, "x");
        }

        let element = TextElement{id: 275, text: "asdf".into()};
        assert!(tree.insert(element).unwrap_err() == Error::DuplicateId);
        assert!(tree.len() == 300);
    }

    #[test]
    fn insert_emoji() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 401, "hello");
        insert(&mut tree, 333, "😀🇦🇽");

        assert!(tree.len() == 8);
        assert!(tree.root.is_leaf());
        assert!(tree.root.elements[0].text == "😀🇦🇽");
        assert!(tree.root.elements[1].text == "hello");
    }

    #[test]
    fn remove_basic() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 401, "hello");
        insert(&mut tree, 1010, "howareyou");
        insert(&mut tree, 1024, "goodbye");
        assert!(tree.len() == 21);

        let e = tree.remove(&1010).unwrap();
        assert!(e.text == "howareyou");
        assert!(tree.len() == 12);
        assert!(tree.root.is_leaf());
        assert!(tree.root.elements[0].text == "hello");
        assert!(tree.root.elements[1].text == "goodbye");
    }

    #[test]
    fn remove_duplicate() {
        let mut tree: Tree<TextElement> = Tree::new();

        for i in 0..300 {
            insert(&mut tree, i, "x");
        }

        let element = tree.remove(&100).unwrap();
        assert!(tree.len() == 299);

        assert!(tree.remove(&element.id).is_none());
        assert!(tree.len() == 299);
    }

    #[test]
    fn remove_emoji() {
        let mut tree: Tree<TextElement> = Tree::new();
        insert(&mut tree, 0, "'sup");
        insert(&mut tree, 14012, "🤣➔🥅");
        insert(&mut tree, 50172501, "goodbye");
        assert!(tree.len() == 14);

        let e = tree.remove(&14012).unwrap();
        assert!(e.text == "🤣➔🥅");
        assert!(tree.len() == 11);
        assert!(tree.root.is_leaf());
        assert!(tree.root.elements[0].text == "'sup");
        assert!(tree.root.elements[1].text == "goodbye");
    }

    #[test]
    fn insert_and_remove_ordered() {
        let mut tree: Tree<TextElement> = Tree::new();
        let paragraph = r#"
        a ac adipiscing aliquam aliquet amet arcu at auctor commodo congue
        consectetur cras curabitur dignissim dolor dui egestas eleifend elit
        enim eros est et eu euismod fames feugiat finibus habitant hendrerit
        imperdiet integer interdum ipsum justo lectus lobortis lorem luctus
        maecenas magna malesuada mattis maximus metus mi mollis morbi nam nec
        netus nisi non nulla nunc odio ornare pellentesque pharetra placerat
        praesent pretium purus quam quis risus sagittis scelerisque sed sem
        senectus sit sollicitudin suspendisse tellus tempus tincidunt tortor
        tristique turpis ultricies urna ut vehicula vel venenatis vestibulum
        vitae volutpat"#;

        for (i, word) in paragraph.split_whitespace().enumerate() {
            insert(&mut tree, i as u64, word);
        }

        assert!(tree.len() == 543);
        assert!(tree.root.is_internal());

        let mut words = paragraph.split_whitespace();
        for element in tree.iter() {
            let word = words.next().unwrap();
            assert!(element.text == word);
        }

        let e1 = tree.remove(&0).unwrap();
        let e2 = tree.remove(&2).unwrap();
        let e3 = tree.remove(&4).unwrap();
        assert!(tree.len() == 525);
        assert!(e1.text == "a");
        assert!(e2.text == "adipiscing");
        assert!(e3.text == "aliquet");

        while tree.len() > 0 {
            let old_len = tree.len();
            let e_id = tree.get_elt(0).unwrap().0.id;
            let e = tree.remove(&e_id).unwrap();
            assert!(tree.len() == old_len - e.element_len());
        }
    }

    #[test]
    fn insert_and_remove_random() {
        let mut tree: Tree<TextElement> = Tree::new();
        let paragraph = r#"
        a ac adipiscing aliquam aliquet amet arcu at auctor commodo congue
        consectetur cras curabitur dignissim dolor dui egestas eleifend elit
        enim eros est et eu euismod fames feugiat finibus habitant hendrerit
        imperdiet integer interdum ipsum justo lectus lobortis lorem luctus
        maecenas magna malesuada mattis maximus metus mi mollis morbi nam nec
        netus nisi non nulla nunc odio ornare pellentesque pharetra placerat
        praesent pretium purus quam quis risus sagittis scelerisque sed sem
        senectus sit sollicitudin suspendisse tellus tempus tincidunt tortor
        tristique turpis ultricies urna ut vehicula vel venenatis vestibulum
        vitae volutpat"#;

        let mut rng = rand::thread_rng();
        let mut words: Vec<(usize, &str)> = paragraph.split_whitespace().enumerate().collect();
        let mut indices: Vec<u64> = words.iter().map(|&(i, _)| i as u64).collect();

        rng.shuffle(&mut words);
        rng.shuffle(&mut indices);

        for (i, word) in words {
            insert(&mut tree, i as u64, word);
        }

        assert!(tree.len() == 543);
        assert!(tree.root.is_internal());

        for i in indices {
            let old_len = tree.len();
            let e = tree.remove(&i).unwrap();
            assert!(tree.len() == old_len - e.element_len());
        }
    }

    fn insert(tree: &mut Tree<TextElement>, id: u64, text: &str) {
        tree.insert(TextElement{id: id, text: text.into()}).unwrap()
    }
}
