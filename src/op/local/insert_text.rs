use std::any::Any;
use op::LocalOp;

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

impl LocalOp for InsertText {
    fn as_any(&self) -> &Any { self }
}
