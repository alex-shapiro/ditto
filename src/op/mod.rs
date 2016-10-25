pub mod local;
pub mod remote;

pub use self::local::LocalOp;
pub use self::remote::RemoteOp;

pub struct NestedLocalOp {
    pub pointer: String,
    pub op: local::LocalOp,
}

pub struct NestedRemoteOp {
    pub pointer: String,
    pub op: remote::RemoteOp,
}
