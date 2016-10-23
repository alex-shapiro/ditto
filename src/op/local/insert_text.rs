pub struct InsertText {
    pub path: String,
    pub index: usize,
    pub text: String,
}

impl InsertText {
    pub fn new(index: usize, text: String) -> InsertText {
        InsertText{path: String::new(), index: index, text: text}
    }
}
