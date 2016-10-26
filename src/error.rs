#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    InvalidRemoteOp,
    Noop,
    OutOfIndex,
    VLQNoTerminatingByte,
}
