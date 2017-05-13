use Error;

/// Required functions for CRDT implementation.
pub trait Crdt: Sized {
    type Value: CrdtValue;

    /// Returns the CRDT's site.
    fn site(&self) -> u32;

    /// Returns a reference to the CRDT's inner value
    fn value(&self) -> &Self::Value;

    /// Clones the CRDT's inner value.
    fn clone_value(&self) -> Self::Value;

    /// Constructs a new CRDT from an inner value and a site.
    fn from_value(value: Self::Value, site: u32) -> Self;

    /// Executes a remote op and returns the equivalent local op.
    fn execute_remote(&mut self, op: &<Self::Value as CrdtValue>::RemoteOp) -> Option<<Self::Value as CrdtValue>::LocalOp>;

    /// Consumes the CRDT and returns the equivalent local value.
    fn local_value(&self) -> <Self::Value as CrdtValue>::LocalValue;

    /// Called after any successful local op.
    fn after_op(&mut self, op: <Self::Value as CrdtValue>::RemoteOp) -> Result<<Self::Value as CrdtValue>::RemoteOp, Error>;

    /// Updates the CRDT's site and executes any awaiting ops.
    fn add_site(&mut self, site: u32) -> Result<Vec<<Self::Value as CrdtValue>::RemoteOp>, Error>;
}

/// The standard implementation for a CRDT.
macro_rules! crdt_impl {
    ($tipe:ident, $value:ty) => {
        type Value = $value;

        fn site(&self) -> u32 {
            self.replica.site
        }

        fn value(&self) -> &Self::Value {
            &self.value
        }

        fn clone_value(&self) -> Self::Value {
            self.value.clone()
        }

        fn from_value(value: Self::Value, site: u32) -> Self {
            $tipe{replica: Replica::new(site, 0), value: value, awaiting_site: vec![]}
        }

        fn local_value(&self) -> <Self::Value as CrdtValue>::LocalValue {
            self.value.local_value()
        }

        fn execute_remote(&mut self, op: &<Self::Value as CrdtValue>::RemoteOp) -> Option<<Self::Value as CrdtValue>::LocalOp> {
            self.value.execute_remote(op)
        }

        fn after_op(&mut self, op: <Self::Value as CrdtValue>::RemoteOp) -> Result<<Self::Value as CrdtValue>::RemoteOp, Error> {
            self.replica.counter += 1;
            if self.replica.site != 0 { return Ok(op) }
            self.awaiting_site.push(op);
            Err(Error::AwaitingSite)
        }

        fn add_site(&mut self, site: u32) -> Result<Vec<<Self::Value as CrdtValue>::RemoteOp>, Error> {
            use std::mem;

            if self.replica.site != 0 { return Err(Error::AlreadyHasSite) }
            let mut ops = mem::replace(&mut self.awaiting_site, vec![]);

            for mut op in ops.iter_mut() {
                self.value.add_site(op, site);
                op.add_site(site);
            }

            Ok(ops)
        }
    };
}

/// Required functions for CRDT values.
pub trait CrdtValue {
    type RemoteOp: CrdtRemoteOp;
    type LocalOp;
    type LocalValue;

    /// Returns the equivalent LocalValue.
    fn local_value(&self) -> Self::LocalValue;

    /// Adds a site to the elements affected by the remote op.
    fn add_site(&mut self, op: &Self::RemoteOp, site: u32);

}

/// Required functions for CRDT remote ops.
pub trait CrdtRemoteOp {
    /// Adds a site to all UIDs with site 0.
    fn add_site(&mut self, site: u32);
}

pub trait AddSiteToAll {
    /// Adds a site to all elements in the CRDT.
    fn add_site_to_all(&mut self, site: u32);
}
