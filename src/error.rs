#[derive(Clone,PartialEq,Debug)]
pub enum Error {
    ValueMismatch(&'static str),
    DecodeCompact,
    DeserializeObjectUID,
    DeserializeSequenceUID,
    InvalidRemoteOp,
    Noop,
    OutOfBounds,
    VLQNoTerminatingByte,
}
