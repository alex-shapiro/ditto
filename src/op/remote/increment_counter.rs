use Replica;
use super::RemoteOpTrait;

#[derive(Clone, Debug, PartialEq)]
pub struct IncrementCounter {
    pub amount: f64,
    pub replica: Replica,
}

impl RemoteOpTrait for IncrementCounter {
    fn validate(&self, site: u32) -> bool {
        self.replica.site == site
    }

    fn update_site(&mut self, site: u32) {
        self.replica.site = site;
    }

    fn reverse(&self) -> Self {
        IncrementCounter {
            amount: -self.amount,
            replica: self.replica.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate() {
        let op = IncrementCounter{amount: 1.0, replica: Replica::new(1,1)};
        assert!(op.validate(1));
        assert!(!op.validate(2));
    }

    #[test]
    fn test_update_site() {
        let mut op = IncrementCounter{amount: 1.0, replica: Replica::new(0, 3)};
        op.update_site(132);
        assert!(op.replica.site == 132);
    }

    #[test]
    fn test_reverse() {
        let op = IncrementCounter{amount: 1.0, replica: Replica::new(1,1)};
        let op_reverse = op.reverse();
        assert!(op_reverse.amount == -1.0);
        assert!(op_reverse.replica == op.replica);
    }
}
