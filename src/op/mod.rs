pub mod local;
pub mod remote;

pub use self::local::LocalOp;
pub use self::remote::RemoteOp;
use raw;
use serde;

pub struct NestedLocalOp {
    pub pointer: String,
    pub op: local::LocalOp,
}

#[derive(PartialEq)]
pub struct NestedRemoteOp {
    pub pointer: String,
    pub op: remote::RemoteOp,
}

impl NestedRemoteOp {
    pub fn reverse(&self) -> Self {
        NestedRemoteOp{pointer: self.pointer.clone(), op: self.op.reverse()}
    }
}

impl serde::Serialize for NestedLocalOp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer {
        serializer.serialize_some(&raw::encode_op(self))
    }
}
