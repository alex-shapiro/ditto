pub struct DeleteItem {
    pub path: String,
    pub index: usize,
}

impl DeleteItem {
    pub fn new(index: usize) -> DeleteItem {
        DeleteItem{path: String::new(), index: index}
    }
}
