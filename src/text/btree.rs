//! A Counted BTree that holds text elements. It can perform lookups
//! by either element UID or character index. It performs all lookups,
//! inserts, and removes in O(log N) time.

use super::element::{self, Element};
use sequence::uid::UID;
use Error;
use std::iter::IntoIterator;
use std::mem;
use serde::{Serialize, Serializer, Deserialize, Deserializer};

const B: usize = 6;
const MIN_LEN: usize = B - 1;
const CAPACITY: usize = 2 * B - 1;

#[derive(Debug, Clone, PartialEq)]
pub struct BTree {
    root: Node,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    len: usize,
    elements: Vec<Element>,
    children: Vec<Node>,
}

impl BTree {
    /// Constructs a new, empty BTree.
    pub fn new() -> Self {
        BTree{root: Node::new()}
    }

    /// Inserts an element into the BTree. Returns an error if the
    /// BTree contains an element with the same UID. Given that a
    /// UID is only generated once, this allows the CRDT to handle
    /// duplicate operations without losing consistency.
    pub fn insert(&mut self, element: Element) -> Result<(), Error> {
        if self.root.is_full() {
            let old_root = mem::replace(&mut self.root, Node::new());
            self.root.len = old_root.len;
            self.root.children.push(old_root);
            self.root.split_child(0);
        }
        self.root.insert(element)
    }

    /// Deletes an element from the BTree. Returns None if the
    /// element is not in the BTree. This allows the CRDT to handle
    /// duplicate operations without losing consistency.
    pub fn remove(&mut self, uid: &UID) -> Option<Element> {
        self.root.remove(uid)
    }

    /// Returns the element that contains the character at
    /// location `index`, as well as the offset of `index`
    /// within the element. Returns an error if the index
    /// is out-of-bounds.
    pub fn get_element(&self, index: usize) -> Result<(&Element, usize), Error> {
        if index > self.len() { return Err(Error::OutOfBounds) }
        if index == self.len() { return Ok((&*element::END, 0)) }
        Ok(self.root.get_element(index))
    }

    /// Returns the starting character index of an element.
    /// Returns None if the element is not in the BTree.
    pub fn get_index(&self, uid: &UID) -> Option<usize> {
        self.root.get_index(uid)
    }

    /// Returns the number of unicode characters in the BTree.
    pub fn len(&self) -> usize {
        self.root.len
    }
}

impl Node {
    fn new() -> Self {
        Node{len: 0, elements: vec![], children: vec![]}
    }

    fn get_element(&self, mut i: usize) -> (&Element, usize) {
        if self.is_leaf() {
            for element in &self.elements {
                if i < element.len { return (element, i) }
                else { i -= element.len }
            }
        } else {
            let mut elements = self.elements.iter();
            for child in &self.children {
                if i < child.len { return child.get_element(i) }
                else { i -= child.len }
                if let Some(element) = elements.next() {
                    if i < element.len { return (element, i) }
                    else { i -= element.len }
                }
            }
        }
        unreachable!("node: {:?}, index: {}", self, i);
    }

    fn get_index(&self, uid: &UID) -> Option<usize> {
        let (contains_element, index) =
            match self.elements.binary_search_by(|elt| elt.uid.cmp(uid)) {
                Ok(index) => (true, index),
                Err(index) => (false, index),
            };

        let mut char_index = self.elements[..index].iter().map(|e| e.len).sum();
        if self.is_leaf() && contains_element {
            Some(char_index)
        } else if self.is_leaf() {
            None
        } else if contains_element {
            char_index += self.children[..index+1].iter().map(|node| node.len).sum();
            Some(char_index)
        } else {
            char_index += self.children[..index].iter().map(|node| node.len).sum();
            match self.children[index].get_index(uid) {
                Some(sub_index) => Some(char_index + sub_index),
                None => None,
            }
        }
    }

    /// Split the node's ith child in half. The original child MUST
    /// be full (ie, it contains 2B-1 elements and 2B children).
    /// Each new child has B-1 elements and B children. The original
    /// child's median element is promoted as the ith element of the
    /// parent node.
    fn split_child(&mut self, i: usize) {
        let (median, new_child) = {
            let ref mut child = self.children[i];

            let elements = child.elements.split_off(B);
            let median   = child.elements.pop().expect("Element must exist A!");
            let children = if child.is_leaf() { vec![] } else { child.children.split_off(B) };

            child.len =
                child.elements.iter().fold(0, |sum, e| sum + e.len) +
                child.children.iter().fold(0, |sum, e| sum + e.len);

            let new_child_len =
                elements.iter().fold(0, |sum, e| sum + e.len) +
                children.iter().fold(0, |sum, e| sum + e.len);

            let new_child = Node{len: new_child_len, elements, children};

            (median, new_child)
        };

        self.elements.insert(i, median);
        self.children.insert(i + 1, new_child);
    }

    /// Merge the node's ith and (i+1)th children, then remove the median
    /// separating them as well as the (i+1)th node. The new node MUST
    /// contain 2B-1 elements.
    fn merge_children(&mut self, index: usize) {
        let removed_element = self.elements.remove(index);
        let Node{len, mut elements, mut children, ..} = self.children.remove(index+1);

        {
            let ref mut child = self.children[index];
            child.len += removed_element.len + len;
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

    /// Insert a new element into a tree. The root node must
    /// not be full (ie it must contain fewer than CAPACITY
    /// elements)
    fn insert(&mut self, elt: Element) -> Result<(), Error> {
        let mut index = self.elements.binary_search(&elt).err().ok_or(Error::DuplicateUID)?;
        self.len += elt.len;
        if self.is_leaf() {
            self.elements.insert(index, elt);
            Ok(())
        } else {
            if self.children[index].is_full() {
                self.split_child(index);
                if elt > self.elements[index] { index += 1 }
            }
            self.children[index].insert(elt)
        }
    }

    /// Delete an element from a tree, returning the removed element.
    /// The root node must contain at least MIN_LEN + 1 elements.
    fn remove(&mut self, uid: &UID) -> Option<Element> {
        let (contains_element, mut index) =
            match self.elements.binary_search_by(|elt| elt.uid.cmp(uid)) {
                Ok(index) => (true, index),
                Err(index) => (false, index),
            };

        // if the parent is a leaf and it contains the element,
        // simply remove the element.
        if self.is_leaf() && contains_element {
            let removed_element = self.elements.remove(index);
            self.len -= removed_element.len;
            Some(removed_element)

        // if the parent is a leaf and does not contain the element
        // then the element does not exist in the BTree.
        } else if self.is_leaf() {
            None

        // if the parent is internal and it contains the element,
        // remove the element from the parent and rebalance from
        // either the child node to either the left or right of
        // the element.
        } else if contains_element {
            if self.child_has_spare_element(index) {
                let ref mut prev = self.children[index];
                let predecessor_uid = prev.last_uid();
                let e = prev.remove(&predecessor_uid).expect("Element must exist B!");
                let removed_element = mem::replace(&mut self.elements[index], e);
                self.len -= removed_element.len;
                Some(removed_element)

            } else if self.child_has_spare_element(index+1) {
                let ref mut next = self.children[index+1];
                let successor_uid = next.first_uid();
                let e = next.remove(&successor_uid).expect("Element must exist C!");
                let removed_element = mem::replace(&mut self.elements[index], e);
                self.len -= removed_element.len;
                Some(removed_element)

            } else {
                self.merge_children(index);
                if self.is_leaf() {
                    self.remove(uid)
                } else {
                    let element = self.children[index].remove(uid);
                    if let Some(ref e) = element { self.len -= e.len }
                    element
                }
            }

        // if the parent is internal and does not contain the element
        // then call recursively on the correct child node. Before
        // the call, check that child has MIN_LEN + 1 elements. If not,
        // rebalance from the child's left and right siblings.
        } else {
            if !self.child_has_spare_element(index) {
                if index > 0 && self.child_has_spare_element(index-1) {
                    let (sibling_elt, sibling_child) = self.children[index-1].pop_last();
                    let parent_elt = mem::replace(&mut self.elements[index-1], sibling_elt);
                    let child = &mut self.children[index];
                    child.len += parent_elt.len;
                    child.elements.insert(0, parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.insert(0, c);
                    }
                } else if self.child_has_spare_element(index+1) {
                    let (sibling_elt, sibling_child) = self.children[index+1].pop_first();
                    let parent_elt = mem::replace(&mut self.elements[index], sibling_elt);
                    let child = &mut self.children[index];
                    child.len += parent_elt.len;
                    child.elements.push(parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.push(c);
                    }
                } else {
                    if self.children.len()-1 == index { index -= 1 }
                    self.merge_children(index);
                    return self.remove(uid)
                }
            }
            let element = self.children[index].remove(uid);
            if let Some(ref e) = element { self.len -= e.len }
            element
        }
    }

    fn pop_first(&mut self) -> (Element, Option<Self>) {
        let element = self.elements.remove(0);
        self.len -= element.len;
        if self.is_internal() {
            let child = self.children.remove(0);
            self.len -= child.len;
            (element, Some(child))
        } else {
            (element, None)
        }
    }

    fn pop_last(&mut self) -> (Element, Option<Self>) {
        let element = self.elements.pop().expect("Element must exist D!");
        self.len -= element.len;
        if let Some(child) = self.children.pop() {
            self.len -= child.len;
            (element, Some(child))
        } else {
            (element, None)
        }
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.elements.len() == CAPACITY
    }

    #[inline]
    fn child_has_spare_element(&self, index: usize) -> bool {
        match self.children.get(index) {
            Some(child) => child.elements.len() > MIN_LEN,
            None => false,
        }
    }

    #[inline]
    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    #[inline]
    fn is_internal(&self) -> bool {
        !self.is_leaf()
    }

    fn first_uid(&self) -> UID {
        let mut node = self;
        while node.is_internal() { node = &node.children[0] }
        node.elements[0].uid.clone()
    }

    fn last_uid(&self) -> UID {
        let mut node = self;
        while node.is_internal() { node = &node.children.last().expect("Child must exist!") }
        node.elements.last().expect("Element must exist E!").uid.clone()
    }
}

pub struct BTreeIter<'a> {
    stack: Vec<(&'a Node, usize)>,
    node: &'a Node,
    next_index: usize,
}

impl<'a> IntoIterator for &'a BTree {
    type Item = &'a Element;
    type IntoIter = BTreeIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let mut node = &self.root;
        let mut stack = vec![];
        while node.is_internal() {
            stack.push((node, 0));
            node = &node.children[0];
        }
        BTreeIter{stack: stack, node: node, next_index: 0}
    }
}

impl<'a> Iterator for BTreeIter<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref element) = self.node.elements.get(self.next_index) {
                self.next_index += 1;
                while self.node.is_internal() {
                    self.stack.push((self.node, self.next_index));
                    self.node = &self.node.children[self.next_index];
                    self.next_index = 0;
                }
                return Some(element)
            } else {
                if let Some((node, next_index)) = self.stack.pop() {
                    self.node = node;
                    self.next_index = next_index;
                } else {
                    return None
                }
            }
        }
    }
}

impl Serialize for BTree {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let vec: Vec<&Element> = self.into_iter().collect();
        vec.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BTree {
    fn deserialize<D>(deserializer: D) -> Result<BTree, D::Error> where D: Deserializer<'de> {
        let vec = Vec::deserialize(deserializer)?;
        let mut btree = BTree::new();
        for element in vec.into_iter() {
            let _ = btree.insert(element);
        }
        Ok(btree)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use text::element;
    use rand;
    use rand::distributions::{IndependentSample, Range};
    use Replica;

    #[test]
    fn test_new() {
        let btree = BTree::new();
        assert!(btree.len() == 0);
        assert!(btree.root.elements.is_empty());
        assert!(btree.root.children.is_empty());
    }

    #[test]
    fn get_element_out_of_bounds() {
        let btree = BTree::new();
        assert!(btree.get_element(1) == Err(Error::OutOfBounds))
    }

    #[test]
    fn get_element_empty() {
        let btree = BTree::new();
        let (elt, offset) = btree.get_element(0).expect("Not out of bounds!");
        assert!(elt.is_end_marker());
        assert!(offset == 0);
    }

    #[test]
    fn get_element_nonempty() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hello");
        insert_at(&mut btree, 5, "world");

        let (elt, offset) = btree.get_element(0).expect("Not out of bounds!");
        assert!(elt.text == "hello");
        assert!(offset == 0);

        let (elt, offset) = btree.get_element(4).expect("Not out of bounds!");
        assert!(elt.text == "hello");
        assert!(offset == 4);

        let (elt, offset) = btree.get_element(5).expect("Not out of bounds!");
        assert!(elt.text == "world");
        assert!(offset == 0);

        let (elt, offset) = btree.get_element(7).expect("Not out of bounds!");
        assert!(elt.text == "world");
        assert!(offset == 2);

        let (elt, offset) = btree.get_element(10).expect("Not out of bounds!");
        assert!(elt.is_end_marker());
        assert!(offset == 0);
    }

    #[test]
    fn get_index_element_does_not_exist() {
        let btree = BTree::new();
        let uid = UID::between(&UID::min(), &UID::max(), &Replica{site: 1, counter: 1});
        assert!(btree.get_index(&uid).is_none());
    }

    #[test]
    fn get_index_element_exists() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hi");
        insert_at(&mut btree, 2, "there");

        let uid = btree.root.elements[1].uid.clone();
        assert!(btree.get_index(&uid) == Some(2));
    }

    #[test]
    fn insert_basic() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "the");
        assert!(btree.len() == 3);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].text == "the");
    }

    #[test]
    fn insert_emoji() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hello");
        insert_at(&mut btree, 0, "😀🇦🇽");
        assert!(btree.len() == 8);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].text == "😀🇦🇽");
        assert!(btree.root.elements[1].text == "hello");
    }

    #[test]
    fn remove_basic() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hello");
        insert_at(&mut btree, 5, "howareyou");
        insert_at(&mut btree, 14, "goodbye");
        assert!(btree.len() == 21);

        let e = remove_at(&mut btree, 5);
        assert!(e.text == "howareyou");
        assert!(btree.len() == 12);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].text == "hello");
        assert!(btree.root.elements[1].text == "goodbye");
    }

    #[test]
    fn remove_emoji() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "'sup");
        insert_at(&mut btree, 4, "🤣➔🥅");
        insert_at(&mut btree, 7, "goodbye");
        assert!(btree.len() == 14);

        let e = remove_at(&mut btree, 4);
        assert!(e.text == "🤣➔🥅");
        assert!(btree.len() == 11);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].text == "'sup");
        assert!(btree.root.elements[1].text == "goodbye");
    }

    #[test]
    fn insert_and_remove_ordered() {
        let mut btree = BTree::new();
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

        for word in paragraph.split_whitespace().rev() {
            insert_at(&mut btree, 0, word);
        }

        assert!(btree.len() == 543);
        assert!(btree.root.is_internal());
        let mut words = paragraph.split_whitespace();
        for element in btree.into_iter() {
            let word = words.next().unwrap();
            assert!(element.text == word);
        }

        let e1 = remove_at(&mut btree, 0);
        let e2 = remove_at(&mut btree, 2);
        let e3 = remove_at(&mut btree, 9);
        assert!(btree.len() == 525);
        assert!(e1.text == "a");
        assert!(e2.text == "adipiscing");
        assert!(e3.text == "aliquet");

        while btree.len() > 0 {
            let old_len = btree.len();
            let e1 = remove_at(&mut btree, 0);
            assert!(btree.len() == old_len - e1.len);
        }
    }

    #[test]
    fn insert_and_remove_random() {
        let mut btree = BTree::new();
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

        for word in paragraph.split_whitespace() {
            insert_random(&mut btree, word);
        }

        assert!(btree.len() == 543);
        assert!(btree.root.is_internal());

        while btree.len() > 0 {
            let old_len = btree.len();
            let e1 = remove_random(&mut btree);
            assert!(btree.len() == old_len - e1.len);
        }
    }

    fn insert_at(btree: &mut BTree, index: usize, text: &'static str) {
        debug_assert!(index <= btree.len());
        let element = {
            let (next, offset) = btree.get_element(index).unwrap();
            let prev = match index-offset {
                0 => &*element::START,
                _ => { let (prev, _) = btree.get_element(index-offset-1).unwrap(); prev },
            };
            Element::between(prev, next, text.to_owned(), &Replica{site: 1, counter: 1})
        };
        let elt_len = element.len;
        let old_len = btree.len();
        let _ = btree.insert(element);
        debug_assert!(btree.len() == old_len + elt_len);
    }

    fn remove_at(btree: &mut BTree, index: usize) -> Element {
        debug_assert!(index < btree.len());
        let uid = {
            let (elt, _) = btree.get_element(index).expect("Element must exist for index!");
            elt.uid.clone()
        };
        let old_len = btree.len();
        let element = btree.remove(&uid).expect("Element must exist for UID!");
        debug_assert!(btree.len() == old_len - element.len);
        element
    }

    fn insert_random(btree: &mut BTree, text: &'static str) {
        let range = Range::new(0, btree.len() + 1);
        let mut rng = rand::thread_rng();
        let index = range.ind_sample(&mut rng);
        insert_at(btree, index, text)
    }

    fn remove_random(btree: &mut BTree) -> Element {
        let range = Range::new(0, btree.len());
        let mut rng = rand::thread_rng();
        let index = range.ind_sample(&mut rng);
        remove_at(btree, index)
    }
}
