pub mod remote;
use Value;

pub enum Local {
    Put{path: Vec<i64>, key: String, value: Value},
    Delete{path: Vec<i64>, key: String},
    InsertItem{path: Vec<i64>, item: Value, index: usize},
    DeleteItem{path: Vec<i64>, index: usize},
    InsertText{path: Vec<i64>, text: String, index: usize},
    DeleteText{path: Vec<i64>, index: usize, length: usize},
    Increment{path: Vec<i64>, amount: f64},
}

impl Local {
    pub fn put(key: String, value: Value) -> Local {
        Local::Put{path: vec![], key: key, value: value}
    }

    pub fn delete(key: String) -> Local {
        Local::Delete{path: vec![], key: key}
    }
}
