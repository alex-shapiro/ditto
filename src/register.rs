//! A `Register` is a container that stores a single value.
//! The container may update the value it holds, but the
//! value itself is immutable.

use Error;
use Replica;
use traits::*;
use std::fmt::Debug;
use std::mem;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register<T: Debug + Clone> {
    value: RegisterValue<T>,
    replica: Replica,
    awaiting_site: Vec<RemoteOp<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterValue<T: Debug + Clone>(Vec<Element<T>>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteOp<T: Debug + Clone> {
    remove: Vec<Element<T>>,
    insert: Element<T>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalOp<T> {
    pub new_value: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Element<T: Debug + Clone>(Replica, T);

impl<T: Debug + Clone> Register<T> {

    /// Constructs and returns a new register CRDT.
    /// The register has site 1 and counter 0.
    pub fn new(value: T) -> Self {
        let mut replica = Replica::new(1, 0);
        let value = RegisterValue::new(value, &replica);
        replica.counter += 1;
        Register{value, replica, awaiting_site: vec![]}
    }

    /// Returns the register's site.
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
        self.replica.counter += 1;
        if self.replica.site != 0 { return Ok(op) }
        self.awaiting_site.push(op);
        Err(Error::AwaitingSite)
    }
}

impl<T: Debug + Clone> Crdt for Register<T> {
    type Value = RegisterValue<T>;

    fn site(&self) -> u32 {
        self.replica.site
    }

    fn value(&self) -> &RegisterValue<T> {
        &self.value
    }

    fn clone_value(&self) -> RegisterValue<T> {
        self.value.clone()
    }

    fn from_value(value: RegisterValue<T>, site: u32) -> Self {
        let replica = Replica::new(site, 0);
        Register{value, replica, awaiting_site: vec![]}
    }

    fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        self.value.execute_remote(op)
    }

    fn add_site(&mut self, site: u32) -> Result<Vec<RemoteOp<T>>, Error> {
        if self.replica.site != 0 { return Err(Error::AlreadyHasSite) }
        let mut ops = mem::replace(&mut self.awaiting_site, vec![]);
        for op in &mut ops {
            let _ = { self.value.add_site(op, site) };
            op.add_site(site);
        }
        Ok(ops)
    }
}

impl<T: Debug + Clone> RegisterValue<T> {

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
        let remove = mem::replace(&mut self.0, vec![insert.clone()]);
        RemoteOp{ remove, insert }
    }

    /// Executes a remote op and returns the equivalent local op.
    /// If the op's insert does not become the register's locally-visible
    /// value, returns None.
    pub fn execute_remote(&mut self, op: &RemoteOp<T>) -> Option<LocalOp<T>> {
        for element in &op.remove {
            if let Ok(index) = self.0.binary_search_by(|e| e.0.cmp(&element.0)) {
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

impl<T: Debug + Clone> CrdtValue for RegisterValue<T> {
    type LocalValue = T;
    type RemoteOp = RemoteOp<T>;
    type LocalOp = LocalOp<T>;

    fn local_value(&self) -> T {
        self.0[0].1.clone()
    }

    fn add_site(&mut self, op: &RemoteOp<T>, site: u32) {
        if let Ok(index) = self.0.binary_search_by(|e| e.0.cmp(&op.insert.0)) {
            self.0[index].0.site = site;
        }
    }
}

impl <T: Debug + Clone> CrdtRemoteOp for RemoteOp<T> {
    fn add_site(&mut self, site: u32) {
        for element in &mut self.remove {
            if element.0.site == 0 { element.0.site = site; }
        }
        if self.insert.0.site == 0 { self.insert.0.site = site; }
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
        assert!(op.remove[0].0 == Replica{site: 1, counter: 0});
        assert!(op.remove[0].1.clone() == 8142);
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
        let mut register2: Register<&'static str> = Register::from_value(register1.clone_value(), 2);
        let mut register3: Register<&'static str> = Register::from_value(register1.clone_value(), 3);

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
        let mut register2 = Register::from_value(register1.clone_value(), 2);

        let remote_op = register1.update("b").unwrap();
        let local_op = register2.execute_remote(&remote_op).unwrap();

        assert!(register2.execute_remote(&remote_op).is_none());
        assert!(register2.get() == &"b");
        assert!(register2.value.0.len() == 1);
        assert!(local_op.new_value == "b");
    }

    #[test]
    fn test_add_site() {
        let mut register1 = Register::new(123);
        let mut register2 = Register::from_value(register1.clone_value(), 0);
        assert!(register2.update(456).unwrap_err() == Error::AwaitingSite);

        let remote_ops = register2.add_site(2).unwrap();
        let _ = register1.execute_remote(&remote_ops[0]);
        assert!(register1.get() == &456);
        assert!(register2.get() == &456);
    }

    #[test]
    fn test_add_site_already_has_site() {
        let mut register = Register::from_value(RegisterValue::new(123, &Replica{site: 42, counter: 0}), 42);
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
