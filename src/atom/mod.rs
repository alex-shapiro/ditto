//! An Atom CRDT is a container for a single value. The container
//! can update what value it holds, but the value itself is immutable.

mod inner;

pub use self::inner::RemoteOp;
pub use self::inner::LocalOp;
use self::inner::Atom as AtomValue;
use Error;
use Replica;
use std::fmt::Debug;
use std::mem;
use traits::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Atom<T> {
    value: AtomValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

impl<T: Debug + Clone> Atom<T> {

    /// Constructs and returns a new atom CRDT. Like all
    /// new CRDTs, the atom has site 1 and counter 0.
    pub fn new(value: T) -> Self {
        let mut replica = Replica::new(1, 0);
        let value = AtomValue::new(value, &replica);
        replica.counter += 1;
        Atom{value, replica, awaiting_site: vec![]}
    }

    /// Returns the atom's site.
    /// Returns a reference to the atom's value.
    pub fn get(&self) -> &T {
        self.value.value()
    }

    /// Updates the atom's value and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the atom does not have a site allocated, it
    /// caches the op and returns an `AwaitingSite` error.
    pub fn update(&mut self, new_value: T, replica: &Replica) -> Result<RemoteOp<T>, Error> {
        let op = self.value.update(new_value, replica);
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl<T: Debug + Clone> Crdt for Atom<T> {
    type Value = AtomValue<T>;

    fn site(&self) -> u32 {
        self.replica.site
    }

    fn into_value(self) -> AtomValue<T> {
        self.value
    }

    /// Executes a remote operation on the atom and
    /// returns the equivalent local op.
    fn execute_remote(&mut self, op: &RemoteOp<T>) -> LocalOp<T> {
        self.value.execute_remote(op)
    }

    fn add_site(&mut self, site: u32) -> Result<Vec<RemoteOp<T>>, Error> {
        if self.site() != 0 { return Err(Error::AlreadyHasSite) }

        let mut ops = mem::replace(&mut self.awaiting_site, vec![]);
        for op in &mut ops {
            let _ = { self.value.add_site(op, site) };
            op.add_site(site);
        }

        Ok(ops)
    }
}
