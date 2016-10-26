#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    InvalidRemoteOp,
    VLQNoTerminatingByte,
}
