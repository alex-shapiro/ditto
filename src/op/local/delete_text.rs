#[derive(Serialize, Deserialize)]
pub struct DeleteText {
    pub index: usize,
    pub len: usize,
}

impl DeleteText {
    pub fn new(index: usize, len: usize) -> DeleteText {
        DeleteText{index: index, len: len}
    }
}
