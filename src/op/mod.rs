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
    /// Returns true if the NestedRemoteOp's inserts all use
    /// the given site; otherwise returns false.
    pub fn validate(&self, site: u32) -> bool {
        self.op.validate(site)
    }

    /// Reverses the NestedRemoteOp's effect
    pub fn reverse(&self) -> Self {
        NestedRemoteOp{pointer: self.pointer.clone(), op: self.op.reverse()}
    }
}
