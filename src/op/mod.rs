pub mod local;
pub mod remote;

pub use self::local::LocalOp;
pub use self::remote::RemoteOp;

#[derive(Serialize, Deserialize)]
pub struct NestedLocalOp {
    pub pointer: String,
    pub op: local::LocalOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NestedRemoteOp {
    pub pointer: String,
    pub op: remote::RemoteOp,
}

impl NestedRemoteOp {
    pub fn reverse(&self) -> Self {
        NestedRemoteOp{pointer: self.pointer.clone(), op: self.op.reverse()}
    }
}
