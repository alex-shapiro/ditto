use Object;

#[derive(PartialEq,Clone)]
pub enum Value {
    Obj(Object),
    Arr,
    MutStr,
    Str(String),
    Num(f64),
    Bool(bool),
}
