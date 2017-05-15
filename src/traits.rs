use Error;

/// The standard implementation for a CRDT. It is implemented
/// as a macro rather than a trait because (1) the implementation
/// is identical for all CRDTs, and (2) it frees the user from
/// having to import a trait whenever they use a CRDT.
macro_rules! crdt_impl {
    ($tipe:ident, $value:ty) => {

        /// Returns the CRDT's site
        pub fn site(&self) -> u32 {
            self.replica.site
        }

        /// Returns a reference to the CRDT's inner value
        pub fn value(&self) -> &$value {
            &self.value
        }

        /// Clones the CRDT's inner value.
        pub fn clone_value(&self) -> $value {
            self.value.clone()
        }

        /// Constructs a new CRDT from an inner value and a site.
        pub fn from_value(value: $value, site: u32) -> Self {
            $tipe{replica: Replica::new(site, 0), value: value, awaiting_site: vec![]}
        }

        /// Returns the CRDT value's equivalent local value.
        pub fn local_value(&self) -> <$value as CrdtValue>::LocalValue {
            self.value.local_value()
        }

        /// Executes a remote op and returns the equivalent local op.
        /// This function assumes that the op only inserts values from the
        /// correct site; for untrusted ops use `validate_and_execute_remote`.
        pub fn execute_remote(&mut self, op: &<$value as CrdtValue>::RemoteOp) -> Option<<$value as CrdtValue>::LocalOp> {
            self.value.execute_remote(op)
        }

        /// Validates a remote op's site, then executes it and returns
        /// the equivalent local op.
        pub fn validate_and_execute_remote(&mut self, op: &<$value as CrdtValue>::RemoteOp, site: u32) -> Result<Option<<$value as CrdtValue>::LocalOp>, Error> {
            let _ = op.validate_site(site)?;
            Ok(self.value.execute_remote(op))
        }

        /// Updates the CRDT's site and returns any cached ops.
        pub fn add_site(&mut self, site: u32) -> Result<Vec<<$value as CrdtValue>::RemoteOp>, Error> {
            use std::mem;

            if self.replica.site != 0 { return Err(Error::AlreadyHasSite) }
            self.replica.site = site;
            let mut ops = mem::replace(&mut self.awaiting_site, vec![]);

            for mut op in ops.iter_mut() {
                self.value.add_site(op, site);
                op.add_site(site);
            }

            Ok(ops)
        }

        fn after_op(&mut self, op: <$value as CrdtValue>::RemoteOp) -> Result<<$value as CrdtValue>::RemoteOp, Error> {
            self.replica.counter += 1;
            if self.replica.site != 0 { return Ok(op) }
            self.awaiting_site.push(op);
            Err(Error::AwaitingSite)
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

    /// Adds a site to all elements affected by the remote op.
    fn add_site(&mut self, op: &Self::RemoteOp, site: u32);
}

/// Required functions for CRDT remote ops.
pub trait CrdtRemoteOp {
    /// Adds a site to all elements in the op with site 0.
    fn add_site(&mut self, site: u32);

    /// Validates that all inserted elements in the op
    /// have the given site.
    fn validate_site(&self, site: u32) -> Result<(), Error>;
}

pub trait AddSiteToAll {
    /// Adds a site to all elements in the CRDT.
    fn add_site_to_all(&mut self, site: u32);

    /// Validates that all elements have the given site.
    fn validate_site_for_all(&self, site: u32) -> Result<(), Error>;
}
