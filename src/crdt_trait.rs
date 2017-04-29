use Error;
use std::mem;

/// Required functions for CRDT implementation.
pub trait Crdt {
    type LocalValue;
    type Value: CrdtValue;
    type RemoteOp: CrdtRemoteOp;
    type LocalOp;

    /// Returns a mutable reference to the CRDT's value.
    fn value(&mut self) -> &mut Self::Value;

    /// Returns the CRDT's site.
    fn site(&self) -> u32;

    /// Returns a mutable reference to the list of ops
    /// that can be sent after the CRDT receives a site.
    fn awaiting_site(&mut self) -> &mut Vec<Self::RemoteOp>;

    /// Consumes the CRDT and returns the equivalent local value.
    fn into_local(self) -> Self::LocalValue;

    /// This function should be executed after any local update.
    /// If the crdt has a site allocated, it returns the op.
    /// Otherwise, it caches the op and returns an error.
    fn return_or_cache_op(&mut self, op: Self::RemoteOp) -> Result<Self::RemoteOp, Error> {
        if self.site() != 0 { return Ok(op) }
        self.awaiting_site().push(op);
        Err(Error::AwaitingSite)
    }

    /// Updates the CRDT's site and executes any awaiting ops.
    fn update_site(&mut self, site: u32) -> Result<Vec<Self::RemoteOp>, Error> {
        if self.site() != 0 { return Err(Error::AlreadyHasSite) }
        let mut ops = mem::replace(self.awaiting_site(), vec![]);

        for op in &mut ops {
            let _ = {
                let mut value = self.value();
                value.add_site(op, site);
            };
            op.add_site(site);
        }

        Ok(ops)
    }
}

/// Required functions for CRDT values.
pub trait CrdtValue {
    fn add_site<R: CrdtRemoteOp>(&mut self, op: &R, site: u32);
}

/// Required functions for CRDT remote ops.
pub trait CrdtRemoteOp {
    fn add_site(&mut self, site: u32);
}

/// Trait for converting a type into a CRDTs
pub trait IntoCrdt<C: Crdt> {
    fn into_crdt(self, site: &u32) -> C;
}
