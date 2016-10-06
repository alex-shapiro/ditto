use Counter;
use super::path::Path;

#[derive(Clone, PartialEq)]
pub struct UID {
    pub path: Path,
    pub counter: Counter,
}
