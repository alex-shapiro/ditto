pub mod local;
pub mod remote;

use std::any::Any;

pub trait RemoteOp { }


pub trait LocalOp {
    fn as_any(&self) -> &Any;
}
