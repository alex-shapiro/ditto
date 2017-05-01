use Error;

/// Required functions for CRDT implementation.
pub trait Crdt: Sized {
    type Value: CrdtValue;

    /// Returns the CRDT's site.
    fn site(&self) -> u32;

    /// Consumes the CRDT and returns its inner value.
    fn into_value(self) -> Self::Value;

    /// Executes a remote op and returns the equivalent local op.
    fn execute_remote(&mut self, op: &<Self::Value as CrdtValue>::RemoteOp) -> <Self::Value as CrdtValue>::LocalOp;

    /// Consumes the CRDT and returns the equivalent local value.
    fn into_local(self) -> <Self::Value as CrdtValue>::LocalValue {
        self.into_value().into_local()
    }

    /// Updates the CRDT's site and executes any awaiting ops.
    fn add_site(&mut self, site: u32) -> Result<Vec<<Self::Value as CrdtValue>::RemoteOp>, Error>;
}

/// Required functions for CRDT values.
pub trait CrdtValue {
    type RemoteOp: CrdtRemoteOp;
    type LocalOp;
    type LocalValue;

    /// Consumes the CrdtValue and returns its equivalent
    /// LocalValue.
    fn into_local(self) -> Self::LocalValue;

    /// Adds a site to all elements of the Crdt that are
    /// affected by the provided op.
    fn add_site(&mut self, op: &Self::RemoteOp, site: u32);
}

/// Required functions for CRDT remote ops.
pub trait CrdtRemoteOp {
    /// Adds a site to all UIDs with site 0.
    fn add_site(&mut self, site: u32);
}

/// Trait for converting a type into a CRDTs
pub trait IntoCrdt<C: Crdt> {
    fn into_crdt(self, site: &u32) -> C;
}
