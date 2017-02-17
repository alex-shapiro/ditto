use Value;

#[derive(Serialize, Deserialize)]
pub struct InsertItem {
    pub index: usize,
    pub value: Value,
}

impl InsertItem {
    pub fn new(index: usize, value: Value) -> InsertItem {
        InsertItem{index: index, value: value}
    }
}
