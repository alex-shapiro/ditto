use Counter;
use super::path::Path;

#[derive(Clone, PartialEq)]
pub struct UID {
    pub path: Path,
    pub counter: Counter,
}

impl UID {
    pub fn new(path: Path, counter: Counter) -> Self {
        UID{path: path, counter: counter}
    }
}
