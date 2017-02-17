#[derive(Serialize, Deserialize)]
pub struct DeleteItem {
    pub index: usize,
}

impl DeleteItem {
    pub fn new(index: usize) -> DeleteItem {
        DeleteItem{index: index}
    }
}
