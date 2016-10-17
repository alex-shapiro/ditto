use Object;
use Array;
use AttributedString;
use std::fmt;
use std::fmt::Debug;

#[derive(PartialEq,Clone)]
pub enum Value {
    Obj(Object),
    Arr(Array),
    AttrStr(AttributedString),
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Value::Obj(_) =>
                write!(f, "<object>"),
            &Value::Arr(_) =>
                write!(f, "<array>"),
            &Value::AttrStr(_) =>
                write!(f, "<attributed string>"),
            &Value::Str(ref str) =>
                write!(f, "\"{}\">", str),
            &Value::Num(n) =>
                write!(f, "{}", n),
            &Value::Bool(b) =>
                write!(f, "{}", b),
            &Value::Null =>
                write!(f, "null"),
        }
    }
}
