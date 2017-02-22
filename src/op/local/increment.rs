#[derive(Serialize, Deserialize)]
pub struct Increment {
    pub amount: f64,
}

impl Increment {
    pub fn new(amount: f64) -> Self {
        Increment{amount: amount}
    }
}
