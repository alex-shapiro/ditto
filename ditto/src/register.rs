//! A CRDT that stores a replaceable value

use {Error, Replica, Tombstones};
use traits::*;
use std::borrow::Cow;
use std::mem;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register<T: Clone> {
    value: RegisterValue<T>,
    replica: Replica,
    tombstones: Tombstones,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterState<'a, T: Clone + 'a> {
    value: Cow<'a, RegisterValue<T>>,
    tombstones: Cow<'a, Tombstones>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterValue<T: Clone>(Vec<Element<T>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp<T: Clone> {
    remove: Vec<Replica>,
    insert: Element<T>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalOp<T> {
    pub new_value: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Element<T: Clone>(Replica, T);

impl<T: Clone> PartialEq for Element<T> {
    fn eq(&self, other: &Element<T>) -> bool {
        self.0 == other.0
    }
}

/// A Register is a replaceable value that can be updated
/// via the [`update`](#method.update) function.
///
/// Internally, Register is both a CmRDT and a CvRDT - it
/// can provide eventual consistency via both operations and
/// state merges. This flexibility comes with a set of tradeoffs:
///
/// * It is larger than a pure CmRDT, which does not require tombstones,
///   but it can perform stateful merges, which a pure CmRDT cannot do.
///
/// * Unlike a pure CvRDT, it requires each site to replicate its ops
///   in their order of generation. In some cases replicating a
///   Register via an op requires less data than replicating a CvRDT,
///   but in practice this is only true if the Register is being used
///   in a highly-concurrent, high-latency environemtn.
///
/// Register is offered here for the sake of library completeness. It
/// is probably not as ideal as a pure CvRDT, but differences are minimal.
/// If you need a pure CvRDT register, let the maintainers know and
/// they will work on making improvements.
///
impl<T: Clone> Register<T> {

    /// Constructs and returns a new Register with site 1.
    pub fn new(value: T) -> Self {
        let mut replica = Replica::new(1, 0);
        let value = RegisterValue::new(value, &replica);
        let tombstones = Tombstones::new();
        replica.counter += 1;
        Register{value, replica, tombstones, awaiting_site: vec![]}
    }

    /// Returns a reference to the register's value.
    pub fn get(&self) -> &T {
        self.value.get()
    }

    /// Updates the register's value and returns a remote op
    /// that can be sent to remote sites for replication.
    /// If the register does not have a site allocated, it
    /// caches the op and returns an `AwaitingSite` error.
    pub fn update(&mut self, new_value: T) -> Result<RemoteOp<T>, Error> {
        let op = self.value.update(new_value, &self.replica);
        self.after_op(op)
    }

    crdt_impl!(Register, RegisterState, RegisterState<T>, RegisterState<'static, T>, RegisterValue<T>);
}

impl<T: Clone> RegisterValue<T> {

    /// Returns a new register value.
    pub fn new(value: T, replica: &Replica) -> Self {
        let element = Element(replica.clone(), value);
        RegisterValue(vec![element])
    }

    /// Returns a reference to the register's value.
    pub fn get(&self) -> &T {
        &self.0[0].1
    }

    /// Updates the register's value and returns a remote op
    /// that can be sent to remote sites for replication.
    pub fn update(&mut self, new_value: T, replica: &Replica) -> RemoteOp<T> {
        let insert = Element(replica.clone(), new_value);
        let removed_elements = mem::replace(&mut self.0, vec![insert.clone()]);
        let remove = removed_elements.into_iter().map(|e| e.0).collect();
        RemoteOp{ remove, insert }
    }

    /// Executes a remote op and returns the equivalent local op.
    /// If the op's insert does not become the register's locally-visible
    /// value, returns None.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        for replica in &op.remove {
            if let Ok(index) = self.0.binary_search_by(|e| e.0.cmp(&replica)) {
                let _ = self.0.remove(index);
            }
        }

        if let Err(index) = self.0.binary_search_by(|e| e.0.cmp(&op.insert.0)) {
            self.0.insert(index, op.insert.clone());
            if index == 0 {
                let new_value = self.0[0].1.clone();
                return Some(LocalOp{new_value})
            }
        }

        None
    }
}

impl<T: Clone> CrdtValue for RegisterValue<T> {
    type LocalValue = T;
    type RemoteOp = RemoteOp<T>;
    type LocalOp = LocalOp<T>;

    fn local_value(&self) -> T {
        self.0[0].1.clone()
    }

    fn add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        let index = some!(self.0.binary_search_by(|e| e.0.cmp(&op.insert.0)).ok());
        self.0[index].0.site = site;
    }

    fn add_site_to_all(&mut self, site: u32) {
        for element in &mut self.0 {
            element.0.site = site;
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        for element in &self.0 {
            try_assert!(element.0.site == site, Error::InvalidRemoteOp);
        }
        Ok(())
    }

    fn merge(&mut self, other: RegisterValue<T>, self_tombstones: &Tombstones, other_tombstones: &Tombstones) {
        self.0 =
            mem::replace(&mut self.0, vec![])
            .into_iter()
            .filter(|e| other.0.contains(e) || !other_tombstones.contains(&e.0))
            .collect();

        for element in other.0 {
            if let Err(index) = self.0.binary_search_by(|e| e.0.cmp(&element.0)) {
                if !self_tombstones.contains(&element.0) {
                    self.0.insert(index, element);
                }
            }
        }
    }
}

impl<T: Clone> CrdtRemoteOp for RemoteOp<T> {
    fn deleted_replicas(&self) -> Vec<Replica> {
        self.remove.clone()
    }

    fn add_site(&mut self, site: u32) {
        self.insert.0.site = site;
        for replica in &mut self.remove {
            if replica.site == 0 { replica.site = site };
        }
    }

    fn validate_site(&self, site: u32) -> Result<(), Error> {
        try_assert!(self.insert.0.site == site, Error::InvalidRemoteOp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use rmp_serde;

    #[test]
    fn test_new() {
        let register: Register<i64> = Register::new(8142);
        assert!(register.get() == &8142);
        assert!(register.replica.counter == 1);
        assert!(register.value.0.len() == 1);
        assert!(register.value.0[0].0 == Replica{site: 1, counter: 0});
        assert!(register.value.0[0].1.clone() == 8142);
    }

    #[test]
    fn test_update() {
        let mut register: Register<i64> = Register::new(8142);
        let op = register.update(42).unwrap();

        assert!(register.get() == &42);
        assert!(register.replica.counter == 2);
        assert!(op.remove.len() == 1);
        assert!(op.remove[0] == Replica{site: 1, counter: 0});
        assert!(op.insert.0 == Replica{site: 1, counter: 1});
        assert!(op.insert.1.clone() == 42);
    }

    #[test]
    fn test_execute_remote() {
        let mut register1: Register<&'static str> = Register::new("a");
        let mut register2: Register<&'static str> = Register::new("a");

        let remote_op = register1.update("b").unwrap();
        let local_op = register2.execute_remote(&remote_op).unwrap();

        assert!(register2.get() == &"b");
        assert!(register2.value.0.len() == 1);
        assert!(local_op.new_value == "b");
    }

    #[test]
    fn test_execute_remote_concurrent() {
        let mut register1: Register<&'static str> = Register::new("a");
        let mut register2: Register<&'static str> = Register::from_state(register1.clone_state(), Some(2)).unwrap();
        let mut register3: Register<&'static str> = Register::from_state(register1.clone_state(), Some(3)).unwrap();

        let remote_op1 = register1.update("b").unwrap();
        let remote_op2 = register2.update("c").unwrap();
        let local_op = register3.execute_remote(&remote_op1).unwrap();

        assert!(register3.execute_remote(&remote_op2).is_none());
        assert!(register3.get() == &"b");
        assert!(register3.value.0.len() == 2);
        assert!(local_op.new_value == "b");
    }

    #[test]
    fn test_execute_remote_dupe() {
        let mut register1: Register<&'static str> = Register::new("a");
        let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();

        let remote_op = register1.update("b").unwrap();
        let local_op = register2.execute_remote(&remote_op).unwrap();

        assert!(register2.execute_remote(&remote_op).is_none());
        assert!(register2.get() == &"b");
        assert!(register2.value.0.len() == 1);
        assert!(local_op.new_value == "b");
    }

    #[test]
    fn test_merge() {
        let mut register1 = Register::new(123);
        let mut register2 = Register::from_state(register1.clone_state(), Some(2)).unwrap();
        let _ = register1.update(456);
        let _ = register2.update(789);
        register1.merge(register2.clone_state());

        assert!(register1.value.0.len() == 2);
        assert!(register1.value.0[0].1 == 456);
        assert!(register1.value.0[1].1 == 789);
    }

    #[test]
    fn test_add_site() {
        let mut register1 = Register::new(123);
        let mut register2 = Register::from_state(register1.clone_state(), None).unwrap();
        assert!(register2.update(456).unwrap_err() == Error::AwaitingSite);

        let remote_ops = register2.add_site(2).unwrap();
        let _ = register1.execute_remote(&remote_ops[0]);
        assert!(register1.get() == &456);
        assert!(register2.get() == &456);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut register = Register::from_state(Register::new(123).clone_state(), Some(42)).unwrap();
        assert!(register.add_site(44).unwrap_err() == Error::AlreadyHasSite);
    }

    #[test]
    fn test_serialize() {
        let register1 = Register::new("hello".to_owned());
        let s_json = serde_json::to_string(&register1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&register1).unwrap();
        let register2: Register<String> = serde_json::from_str(&s_json).unwrap();
        let register3: Register<String> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(register1 == register2);
        assert!(register1 == register3);
    }

    #[test]
    fn test_serialize_value() {
        let register1 = Register::new("hello".to_owned());
        let s_json = serde_json::to_string(register1.value()).unwrap();
        let s_msgpack = rmp_serde::to_vec(register1.value()).unwrap();
        let value2: RegisterValue<String> = serde_json::from_str(&s_json).unwrap();
        let value3: RegisterValue<String> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(*register1.value() == value2);
        assert!(*register1.value() == value3);
    }

    #[test]
    fn test_serialize_remote_op() {
        let mut register = Register::new(123);
        let remote_op1 = register.update(456).unwrap();
        let s_json = serde_json::to_string(&remote_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&remote_op1).unwrap();
        let remote_op2: RemoteOp<u32> = serde_json::from_str(&s_json).unwrap();
        let remote_op3: RemoteOp<u32> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(remote_op1 == remote_op2);
        assert!(remote_op1 == remote_op3);
    }

    #[test]
    fn test_serialize_local_op() {
        let local_op1 = LocalOp{new_value: 456};
        let s_json = serde_json::to_string(&local_op1).unwrap();
        let s_msgpack = rmp_serde::to_vec(&local_op1).unwrap();
        let local_op2: LocalOp<u32> = serde_json::from_str(&s_json).unwrap();
        let local_op3: LocalOp<u32> = rmp_serde::from_slice(&s_msgpack).unwrap();

        assert!(local_op1 == local_op2);
        assert!(local_op1 == local_op3);
    }
}
