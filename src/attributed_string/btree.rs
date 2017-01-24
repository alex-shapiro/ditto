use super::element::Element;
use sequence::uid::UID;
use error::Error;
use std::mem;
use std::iter::IntoIterator;

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
    pub fn new() -> Self {
        BTree{
            root: Node{
                len: 0,
                elements: vec![Element::start_marker(), Element::end_marker()],
                children: vec![]
            }
        }
    }

    pub fn insert(&mut self, element: Element) {
        if self.root.is_full() {
            let new_root = Node{len: self.root.len, elements: vec![], children: vec![]};
            let old_root = mem::replace(&mut self.root, new_root);
            self.root.len = old_root.len;
            self.root.children.push(old_root);
            self.root.split_child(0);
        }
        self.root.insert(element);
    }

    pub fn delete(&mut self, uid: &UID) -> Option<Element> {
        match self.root.elements.is_empty() {
            true => None,
            false => self.root.delete(uid),
        }
    }

    pub fn search(&self, index: usize) -> Result<(&Element, usize), Error> {
        self.root.search(index)
    }

    pub fn index_of(&self, uid: &UID) -> Result<usize, Error> {
        let mut node = &self.root;
        let mut char_index = 0;

        loop {
            let (contains_element, index) =
                match node.elements.binary_search_by(|elt| elt.uid.cmp(uid)) {
                    Ok(index) => (true, index),
                    Err(index) => (false, index),
                };

            char_index += node.elements[..index].iter().fold(0, |acc, elt| acc+elt.len);
            if node.is_leaf() && contains_element {
                return Ok(char_index)
            } else if node.is_leaf() {
                return Err(Error::OutOfBounds)
            } else if contains_element {
                char_index += node.children[..index+1].iter().fold(0, |acc, node| acc+node.len);
                return Ok(char_index)
            } else {
                char_index += node.children[..index].iter().fold(0, |acc, node| acc+node.len);
                node = &node.children[index];
            }
        }
    }

    pub fn len(&self) -> usize {
        self.root.len
    }
}

impl Node {
    /// Find the element that contains `index`. Returns a reference
    /// to the element and the offset of the index inside the element.
    /// If the index is out of bounds, it returns an OutOfBounds error.
    fn search(&self, mut i: usize) -> Result<(&Element, usize), Error> {
        if i > self.len { return Err(Error::OutOfBounds) }

        if self.is_leaf() {
            for e in &self.elements {
                if i < e.len { return Ok((e, i)) } else { i -= e.len }
            }
            return Ok((self.elements.last().unwrap(), 0))
        } else {
            let mut elements = self.elements.iter();
            for c in &self.children {
                if i < c.len { return c.search(i) } else { i -= c.len }
                let e = elements.next().expect("Element must exist!");
                if i < e.len { return Ok((e, i)) } else { i -= e.len }
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
            let elements = child.elements.split_off(B);
            let median = child.elements.pop().expect("Element must exist!");
            let children = match child.is_leaf() {
                true  => vec![],
                false => child.children.split_off(B),
            };

            let mut new_child_len = elements.iter().map(|e| e.len).sum();
            new_child_len += children.iter().map(|e| e.len).sum();
            let new_child = Node{
                len: new_child_len,
                elements: elements,
                children: children,
            };

            child.len -= new_child.len + median.len;
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
    fn insert(&mut self, elt: Element) {
        let mut pos = self.elements.binary_search(&elt).err().expect("Duplicate UID!");
        self.len += elt.len;
        if self.is_leaf() {
            self.elements.insert(pos, elt);
        } else {
            if self.children[pos].is_full() {
                self.split_child(pos);
                if elt > self.elements[pos] { pos += 1 }
            }
            self.children[pos].insert(elt)
        }
    }

    /// Delete an element from a tree, returning the deleted element.
    /// The root node must contain at least MIN_LEN + 1 elements.
    fn delete(&mut self, uid: &UID) -> Option<Element> {
        let (contains_element, index) =
            match self.elements.binary_search_by(|elt| elt.uid.cmp(uid)) {
                Ok(index) => (true, index),
                Err(index) => (false, index),
            };

        // if the parent is a leaf and it contains the element,
        // simply remove the element.
        if self.is_leaf() && contains_element {
            let deleted_element = self.elements.remove(index);
            self.len -= deleted_element.len;
            Some(deleted_element)

        // if the parent is a leaf and does not contain the element
        // then the element does not exist in the BTree.
        } else if self.is_leaf() {
            None

        // if the parent is internal and it contains the element,
        // remove the element from the parent and rebalance from
        // either the child node to either the left or right of
        // the element.
        } else if contains_element {
            if self.children[index].has_spare_element() {
                let ref mut prev = self.children[index];
                let predecessor_uid = prev.last_uid();
                let e = prev.delete(&predecessor_uid).expect("Element must exist!");
                let deleted_element = mem::replace(&mut self.elements[index], e);
                self.len -= deleted_element.len;
                Some(deleted_element)

            } else if self.children[index+1].has_spare_element() {
                let ref mut next = self.children[index+1];
                let successor_uid = next.first_uid();
                let e = next.delete(&successor_uid).expect("Element must exist!");
                let deleted_element = mem::replace(&mut self.elements[index], e);
                self.len -= deleted_element.len;
                Some(deleted_element)

            } else {
                self.merge_children(index);
                match self.is_leaf() {
                    true  => self.delete(uid),
                    false => self.children[index].delete(uid),
                }
            }

        // if the parent is internal and does not contain the element
        // then call recursively on the correct child node. Before
        // the call, check that child has MIN_LEN + 1 elements. If not,
        // rebalance from the child's left and right siblings.
        } else {
            if !self.children[index].has_spare_element() {
                if index > 0 && self.children.get(index-1).map_or(false, Self::has_spare_element) {
                    let (sibling_elt, sibling_child) = self.children[index-1].pop_last();
                    let parent_elt = mem::replace(&mut self.elements[index-1], sibling_elt);
                    let child = &mut self.children[index];
                    child.elements.insert(0, parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.insert(0, c);
                    }
                }
                else if self.children.get(index+1).map_or(false, Self::has_spare_element) {
                    let (sibling_elt, sibling_child) = self.children[index+1].pop_first();
                    let parent_elt = mem::replace(&mut self.elements[index], sibling_elt);
                    let child = &mut self.children[index];
                    child.elements.push(parent_elt);
                    if let Some(c) = sibling_child {
                        child.len += c.len;
                        child.children.push(c);
                    }
                }
                else {
                    self.merge_children(index);
                    if self.is_leaf() { return self.delete(uid) }
                }
            }
            let element = self.children[index].delete(uid);
            element.map(|e| { self.len -= e.len; e })
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
        let element = self.elements.pop().expect("Element must exist!");
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
    fn has_spare_element(&self) -> bool {
        self.elements.len() > MIN_LEN
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
        while self.is_internal() { node = &node.children.last().expect("Child must exist!") }
        node.elements.last().expect("Element must exist!").uid.clone()
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
        let mut iterator = BTreeIter{stack: stack, node: node, next_index: 0};
        let _ = iterator.next();
        iterator
    }
}

impl<'a> Iterator for BTreeIter<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ref element) = self.node.elements.get(self.next_index) {
                if element.is_end_marker() { return None }
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

#[cfg(test)]
mod tests {
    use super::*;
    use attributed_string::element;
    use Replica;

    #[test]
    fn test_new() {
        let btree = BTree::new();
        assert!(btree.len() == 0);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].is_start_marker());
        assert!(btree.root.elements[1].is_end_marker());
    }

    #[test]
    fn test_insert_into_empty() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "the", &Replica{site: 1, counter: 1});
        assert!(btree.len() == 3);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[0].is_start_marker());
        assert!(btree.root.elements[1].text == "the");
        assert!(btree.root.elements[1].text == "the");
        assert!(btree.root.elements[2].is_end_marker());
    }

    #[test]
    fn test_insert_emoji() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hello", &Replica{site: 1, counter: 1});
        insert_at(&mut btree, 0, "😀🇦🇽", &Replica{site: 1, counter: 2});
        assert!(btree.len() == 8);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[1].text == "😀🇦🇽");
        assert!(btree.root.elements[2].text == "hello");
    }

    #[test]
    fn test_delete() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "hello", &Replica{site: 1, counter: 1});
        insert_at(&mut btree, 5, "howareyou", &Replica{site: 1, counter: 1});
        insert_at(&mut btree, 14, "goodbye", &Replica{site: 1, counter: 2});
        assert!(btree.len() == 21);

        let e = delete_at(&mut btree, 5);
        assert!(e.text == "howareyou");
        assert!(btree.len() == 12);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[1].text == "hello");
        assert!(btree.root.elements[2].text == "goodbye");
    }

    #[test]
    fn test_delete_emoji() {
        let mut btree = BTree::new();
        insert_at(&mut btree, 0, "'sup", &Replica{site: 1, counter: 1});
        insert_at(&mut btree, 4, "🤣➔🥅", &Replica{site: 1, counter: 1});
        insert_at(&mut btree, 7, "goodbye", &Replica{site: 1, counter: 2});
        assert!(btree.len() == 14);

        let e = delete_at(&mut btree, 4);
        assert!(e.text == "🤣➔🥅");
        assert!(btree.len() == 11);
        assert!(btree.root.is_leaf());
        assert!(btree.root.elements[1].text == "'sup");
        assert!(btree.root.elements[2].text == "goodbye");
    }

    #[test]
    fn test_insert_and_delete() {
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

        let replica = Replica{site: 1, counter: 1};
        for word in paragraph.split_whitespace().rev() {
            insert_at(&mut btree, 0, word, &replica);
        }

        assert!(btree.len() == 543);
        assert!(btree.root.is_internal());
        let mut words = paragraph.split_whitespace();
        for element in btree.into_iter() {
            let word = words.next().unwrap();
            assert!(element.text == word);
        }

        let e1 = delete_at(&mut btree, 0);
        let e2 = delete_at(&mut btree, 2);
        let e3 = delete_at(&mut btree, 9);
        assert!(btree.len() == 525);
        assert!(e1.text == "a");
        assert!(e2.text == "adipiscing");
        assert!(e3.text == "aliquet");

        while btree.len() > 0 {
            let old_len = btree.len();
            let e1 = delete_at(&mut btree, 0);
            assert!(btree.len() == old_len - e1.len);
        }
    }

    fn insert_at(btree: &mut BTree, index: usize, text: &'static str, replica: &Replica) {
        debug_assert!(index <= btree.len());
        let element = {
            let (next, offset) = btree.search(index).unwrap();
            debug_assert!(offset == 0);
            let prev = match index {
                0 => &*element::START,
                _ => { let (prev, _) = btree.search(index-1).unwrap(); prev },
            };
            Element::between(prev, next, text.to_owned(), replica)
        };
        btree.insert(element);
    }

    fn delete_at(btree: &mut BTree, index: usize) -> Element {
        debug_assert!(index < btree.len());
        let uid = {
            let (elt, offset) = btree.search(index).expect("Element must exist for index!");
            debug_assert!(offset == 0);
            elt.uid.clone()
        };
        btree.delete(&uid).expect("Element must exist for UID!")
    }
}
