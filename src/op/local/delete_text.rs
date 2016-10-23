pub struct DeleteText {
    pub path: String,
    pub index: usize,
    pub len: usize,
}

impl DeleteText {
    pub fn new(index: usize, len: usize) -> DeleteText {
        DeleteText{path: String::new(), index: index, len: len}
    }
}
