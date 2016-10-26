pub struct IncrementNumber {
    pub amount: f64,
}

impl IncrementNumber {
    pub fn new(amount: f64) -> Self {
        IncrementNumber{amount: amount}
    }
}
