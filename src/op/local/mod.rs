mod delete;
mod put;
mod delete_item;
mod insert_item;
mod delete_text;
mod insert_text;

pub use self::delete::Delete;
pub use self::put::Put;
pub use self::delete_item::DeleteItem;
pub use self::insert_item::InsertItem;
pub use self::delete_text::DeleteText;
pub use self::insert_text::InsertText;

pub enum LocalOp {
    Put(Put),
    Delete(Delete),
    InsertItem(InsertItem),
    DeleteItem(DeleteItem),
    InsertText(InsertText),
    DeleteText(DeleteText),
}
