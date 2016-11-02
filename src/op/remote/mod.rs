mod increment_number;
mod update_array;
mod update_attributed_string;
mod update_object;

pub use self::increment_number::IncrementNumber;
pub use self::update_array::UpdateArray;
pub use self::update_attributed_string::UpdateAttributedString;
pub use self::update_object::UpdateObject;

#[derive(PartialEq)]
pub enum RemoteOp {
    IncrementNumber(IncrementNumber),
    UpdateArray(UpdateArray),
    UpdateAttributedString(UpdateAttributedString),
    UpdateObject(UpdateObject),
}

impl RemoteOp {
    pub fn reverse(&self) -> Self {
        match *self {
            RemoteOp::IncrementNumber(ref op) =>
                RemoteOp::IncrementNumber(op.reverse()),
            RemoteOp::UpdateArray(ref op) =>
                RemoteOp::UpdateArray(op.reverse()),
            RemoteOp::UpdateAttributedString(ref op) =>
                RemoteOp::UpdateAttributedString(op.reverse()),
            RemoteOp::UpdateObject(ref op) =>
                RemoteOp::UpdateObject(op.reverse()),
        }
    }
}

pub trait Reverse {
    fn reverse(&self) -> Self;
}
