use op::RemoteOp;

pub struct IncrementNumber {
    pub path: Vec<i64>,
    pub amount: f64,
}

impl IncrementNumber {
    pub fn new(amount: f64) -> Self {
        IncrementNumber{path: vec![], amount: amount}
    }
}

impl RemoteOp for IncrementNumber { }
