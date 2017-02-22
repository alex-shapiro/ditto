use Replica;
use super::Reverse;

#[derive(Clone, Debug, PartialEq)]
pub struct IncrementCounter {
    pub amount: f64,
    pub replica: Replica,
}

impl Reverse for IncrementCounter {
    fn reverse(&self) -> Self {
        IncrementCounter {
            amount: -self.amount,
            replica: self.replica.clone(),
        }
    }
}
