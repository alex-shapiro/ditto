use Object;
use Array;

#[derive(PartialEq,Clone)]
pub enum Value {
    Obj(Object),
    Arr(Array),
    MutStr,
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}
