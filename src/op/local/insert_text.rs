#[derive(Serialize, Deserialize)]
pub struct InsertText {
    pub index: usize,
    pub text: String,
}

impl InsertText {
    pub fn new(index: usize, text: String) -> InsertText {
        InsertText{index: index, text: text}
    }
}
