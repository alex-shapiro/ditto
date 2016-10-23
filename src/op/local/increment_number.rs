pub struct IncrementNumber {
    pub path: String,
    pub amount: f64,
}

impl IncrementNumber {
    pub fn new(amount: f64) -> Self {
        IncrementNumber{path: String::new(), amount: amount}
    }
}
