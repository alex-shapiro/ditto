mod delete;
mod put;
mod delete_item;
mod insert_item;
mod delete_text;
mod insert_text;
mod increment_number;

pub use self::delete::Delete;
pub use self::put::Put;
pub use self::delete_item::DeleteItem;
pub use self::insert_item::InsertItem;
pub use self::delete_text::DeleteText;
pub use self::insert_text::InsertText;
pub use self::increment_number::IncrementNumber;

pub enum LocalOp {
    Put(Put),
    Delete(Delete),
    InsertItem(InsertItem),
    DeleteItem(DeleteItem),
    InsertText(InsertText),
    DeleteText(DeleteText),
    IncrementNumber(IncrementNumber),
}

impl LocalOp {
    pub fn put(&self) -> Option<&Put> {
        match *self {
            LocalOp::Put(ref op) => Some(op),
            _ => None,
        }
    }

    pub fn delete(&self) -> Option<&Delete> {
        match *self {
            LocalOp::Delete(ref op) => Some(op),
            _ => None,
        }
    }

    pub fn insert_item(&self) -> Option<&InsertItem> {
        match *self {
            LocalOp::InsertItem(ref op) => Some(op),
            _ => None,
        }
    }

    pub fn delete_item(&self) -> Option<&DeleteItem> {
        match * self {
            LocalOp::DeleteItem(ref op) => Some(op),
            _ => None,
        }
    }
    pub fn insert_text(&self) -> Option<&InsertText> {
        match *self {
            LocalOp::InsertText(ref op) => Some(op),
            _ => None,
        }
    }

    pub fn delete_text(&self) -> Option<&DeleteText> {
        match * self {
            LocalOp::DeleteText(ref op) => Some(op),
            _ => None,
        }
    }

    pub fn increment_number(&self) -> Option<&IncrementNumber> {
        match *self {
            LocalOp::IncrementNumber(ref op) => Some(op),
            _ => None,
        }
    }
}
