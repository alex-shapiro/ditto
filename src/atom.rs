use Replica;

use std::fmt::Debug;
use std::mem;

#[derive(Debug, Clone)]
pub struct Atom<T>(Vec<Element<T>>);

pub struct RemoteOp<T> {
    deletes: Vec<Element<T>>,
    insert: Element<T>,
}

pub struct LocalOp<T> {
    pub value: T,
}

type Element<T> = (Replica, T);

impl<T> Atom<T> where T: Debug + Clone  {
    /// Returns a newly-constructed atom.
    pub fn new(value: T) -> Self {
        let replica = Replica{site: 1, counter: 0};
        let element = (replica, value);
        Atom(vec![element])
    }

    /// Returns the atom's value.
    pub fn value(&self) -> &T {
        &self.0[0].1
    }

    /// Consumes the atom and returns its value.
    pub fn into(self) -> T {
        let mut vec = self.0;
        vec.swap_remove(0).1
    }

    /// Updates the atom's value and returns a remote op
    /// that can be sent to remote sites for replication.
    pub fn update(&mut self, new_value: T, replica: &Replica) -> RemoteOp<T> {
        let insert = (replica.clone(), new_value);
        let deletes = mem::replace(&mut self.0, vec![insert.clone()]);
        RemoteOp{ deletes, insert }
    }

    /// Updates the atom with a remote op and returns a
    /// local op with the new value.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> LocalOp<T> {
        for delete in &op.deletes {
            if let Ok(index) = self.0.binary_search_by(|e| e.0.cmp(&delete.0)) {
                let _ = self.0.remove(index);
            }
        }

        if let Err(index) = self.0.binary_search_by(|e| e.0.cmp(&op.insert.0)) {
            let _ = self.0.insert(index, op.insert.clone());
        }

        LocalOp{value: self.0[0].1.clone()}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let atom: Atom<i64> = Atom::new(8142);
        assert!(atom.value().clone() == 8142);
        assert!(atom.0.len() == 1);
        assert!(atom.0[0].0 == Replica{site: 1, counter: 0});
        assert!(atom.0[0].1.clone() == 8142);
    }

    #[test]
    fn test_into() {
        let atom: Atom<i64> = Atom::new(789);
        assert!(atom.into() == 789);
    }

    #[test]
    fn test_update() {
        let mut atom: Atom<i64> = Atom::new(8142);
        let op = atom.update(42, &Replica{site: 1, counter: 1});

        assert!(atom.value().clone() == 42);
        assert!(op.deletes.len() == 1);
        assert!(op.deletes[0].0 == Replica{site: 1, counter: 0});
        assert!(op.deletes[0].1.clone() == 8142);
        assert!(op.insert.0 == Replica{site: 1, counter: 1});
        assert!(op.insert.1.clone() == 42);
    }

    #[test]
    fn test_execute_remote() {
        let mut atom1: Atom<&'static str> = Atom::new("a");
        let mut atom2: Atom<&'static str> = Atom::new("a");

        let remote_op = atom1.update("b", &Replica{site: 1, counter: 0});
        let local_op = atom2.execute_remote(&remote_op);

        assert!(atom2.value().clone() == "b");
        assert!(atom2.0.len() == 1);
        assert!(local_op.value.clone() == "b");
    }

    #[test]
    fn test_execute_remote_concurrent() {
        let mut atom1: Atom<&'static str> = Atom::new("a");
        let mut atom2: Atom<&'static str> = Atom::new("a");
        let mut atom3: Atom<&'static str> = Atom::new("a");

        let remote_op1 = atom1.update("b", &Replica{site: 1, counter: 1});
        let remote_op2 = atom2.update("c", &Replica{site: 2, counter: 0});
        let local_op1 = atom3.execute_remote(&remote_op1);
        let local_op2 = atom3.execute_remote(&remote_op2);

        assert!(atom3.value().clone() == "b");
        assert!(atom3.0.len() == 2);
        assert!(local_op1.value.clone() == "b");
        assert!(local_op2.value.clone() == "b");
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut atom1: Atom<&'static str> = Atom::new("a");
        let mut atom2: Atom<&'static str> = Atom::new("a");

        let remote_op = atom1.update("b", &Replica{site: 1, counter: 0});
        let local_op1 = atom2.execute_remote(&remote_op);
        let local_op2 = atom2.execute_remote(&remote_op);

        assert!(atom2.value().clone() == "b");
        assert!(atom2.0.len() == 1);
        assert!(local_op1.value.clone() == "b");
        assert!(local_op2.value.clone() == "b");
    }
}
