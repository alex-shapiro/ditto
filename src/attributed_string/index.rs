//! An index that tracks the element, character offset,
//! byte offset, and overall character location of a
//! location in an AttributedString

#[derive(Debug, PartialEq, Eq)]
pub struct Index {
    pub eidx: usize,
    pub cidx: usize,
    pub bidx: usize,
    pub location: usize,
}
