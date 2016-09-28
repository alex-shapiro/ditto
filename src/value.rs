#[derive(PartialEq)]
pub enum Value {
    Obj,
    Arr,
    MutStr,
    Str(String),
    Num(f64),
    Bool(bool),
}
