use std::any::Any;
use op::LocalOp;

pub struct InsertText {
    pub path: Vec<i64>,
    pub index: usize,
    pub text: String,
}

impl InsertText {
    pub fn new(index: usize, text: String) -> InsertText {
        InsertText{path: vec![], index: index, text: text}
    }
}

impl LocalOp for InsertText {
    fn path(&self) -> &Vec<i64> { &self.path }
    fn as_any(&self) -> &Any { self }
}
