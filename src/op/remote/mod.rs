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
