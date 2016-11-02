use super::Reverse;

#[derive(Clone,PartialEq,Debug)]
pub struct IncrementNumber {
    pub amount: f64,
}

impl IncrementNumber {
    pub fn new(amount: f64) -> Self {
        IncrementNumber{amount: amount}
    }
}

impl Reverse for IncrementNumber {
    fn reverse(&self) -> Self {
        IncrementNumber{amount: -self.amount}
    }
}
