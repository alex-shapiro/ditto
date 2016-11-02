pub struct ReplaceText {
    pub index: usize,
    pub len: usize,
    pub text: String,
}

impl ReplaceText {
    pub fn new(index: usize, len: usize, text: String) -> ReplaceText {
        ReplaceText{index: index, len: len, text: text}
    }
}
