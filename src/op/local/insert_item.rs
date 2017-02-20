use LocalValue;

#[derive(Serialize, Deserialize)]
pub struct InsertItem {
    pub index: usize,
    pub value: LocalValue,
}

impl InsertItem {
    pub fn new(index: usize, value: LocalValue) -> InsertItem {
        InsertItem{index: index, value: value}
    }
}
