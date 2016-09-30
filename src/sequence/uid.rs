use Counter;
use super::path::Path;

pub struct UID {
    pub path: Path,
    pub counter: Counter,
}
