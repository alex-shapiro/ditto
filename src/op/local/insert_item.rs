use Value;

pub struct InsertItem {
    pub path: String,
    pub index: usize,
    pub value: Value,
}

impl InsertItem {
    pub fn new(index: usize, value: Value) -> InsertItem {
        InsertItem{path: String::new(), index: index, value: value}
    }
}
