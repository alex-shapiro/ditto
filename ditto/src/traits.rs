use {Error, Replica, Tombstones};

/// The standard implementation for a CRDT. It is implemented
/// as a macro rather than a trait because (1) the implementation
/// is identical for all CRDTs, and (2) it frees the user from
/// having to import a trait whenever they use a CRDT.
macro_rules! crdt_impl {
    ($tipe:ident, $state_ident:ident, $state:ty, $state_static:ty, $value:ty) => {

        #[doc(hidden)]
        /// Returns the CRDT's site id
        pub fn site_id(&self) -> u32 {
            self.replica.site_id
        }

        #[doc(hidden)]
        /// Returns the CRDT's counter
        pub fn counter(&self) -> u32 {
            self.replica.counter
        }

        #[doc(hidden)]
        /// Returns a reference to the CRDT's inner value
        pub fn value(&self) -> &$value {
            &self.value
        }

        #[doc(hidden)]
        /// Returns a reference to the remote ops which are
        /// awaiting a site before being returned
        pub fn awaiting_site(&self) -> &[<$value as CrdtValue>::RemoteOp] {
            &self.awaiting_site
        }

        #[doc(hidden)]
        /// Returns a reference to the CRDT's tombstones
        pub fn tombstones(&self) -> &Tombstones {
            &self.tombstones
        }

        /// Returns a borrowed CRDT state.
        pub fn state(&self) -> $state {
            $state_ident{
                value: Cow::Borrowed(&self.value),
                tombstones: Cow::Borrowed(&self.tombstones),
            }
        }

        /// Clones the CRDT's state.
        pub fn clone_state(&self) -> $state_static {
            $state_ident{
                value: Cow::Owned(self.value.clone()),
                tombstones: Cow::Owned(self.tombstones.clone())
            }
        }

        /// Consumes the CRDT and returns its state.
        pub fn into_state(self) -> $state_static {
            $state_ident {
                value: Cow::Owned(self.value),
                tombstones: Cow::Owned(self.tombstones),
            }
        }

        /// Constructs a new CRDT from a state and optional site.
        /// If the site is present, it must be nonzero.
        pub fn from_state(state: $state, site_id: Option<u32>) -> Result<Self, Error> {
            let site_id = match site_id {
                None => 0,
                Some(0) => return Err(Error::InvalidSiteId),
                Some(s) => s,
            };

            Ok($tipe{
                replica: Replica{site_id, counter: 0},
                value: state.value.into_owned(),
                tombstones: state.tombstones.into_owned(),
                awaiting_site: vec![],
            })
        }

        /// Returns the CRDT value's equivalent local value.
        pub fn local_value(&self) -> <$value as CrdtValue>::LocalValue {
            self.value.local_value()
        }

        /// Executes a remote op and returns the equivalent local op.
        /// This function assumes that the op only inserts values from the
        /// correct site; for untrusted ops use `validate_and_execute_remote`.
        pub fn execute_remote(&mut self, op: &<$value as CrdtValue>::RemoteOp) -> Option<<$value as CrdtValue>::LocalOp> {
            for replica in op.deleted_replicas() { self.tombstones.insert(&replica) };
            self.value.execute_remote(op)
        }

        /// Validates a remote op's site, then executes it and returns
        /// the equivalent local op.
        pub fn validate_and_execute_remote(&mut self, op: &<$value as CrdtValue>::RemoteOp, site: u32) -> Result<Option<<$value as CrdtValue>::LocalOp>, Error> {
            let _ = op.validate_site(site)?;
            Ok(self.execute_remote(op))
        }

        /// Merges remote CRDT state into the CRDT.
        pub fn merge(&mut self, other: $state) {
            self.value.merge(other.value.into_owned(), &self.tombstones, &other.tombstones);
            self.tombstones.merge(&other.tombstones);
        }

        /// Assigns a site and returns any cached ops.
        pub fn add_site(&mut self, site_id: u32) -> Result<Vec<<$value as CrdtValue>::RemoteOp>, Error> {
            use std::mem;

            if self.replica.site_id != 0 { return Err(Error::AlreadyHasSite) }
            self.replica.site_id = site_id;
            let mut ops = mem::replace(&mut self.awaiting_site, vec![]);

            for op in ops.iter_mut() {
                self.value.add_site(op, site_id);
                op.add_site(site_id);
            }

            Ok(ops)
        }

        fn after_op(&mut self, op: <$value as CrdtValue>::RemoteOp) -> Result<<$value as CrdtValue>::RemoteOp, Error> {
            self.replica.counter += 1;
            for replica in op.deleted_replicas() { self.tombstones.insert(&replica) };
            if self.replica.site_id != 0 { return Ok(op) }
            self.awaiting_site.push(op);
            Err(Error::AwaitingSite)
        }
    };
}

#[doc(hidden)]
/// Required functions for CRDT values.
pub trait CrdtValue {
    type RemoteOp: CrdtRemoteOp;
    type LocalOp;
    type LocalValue;

    /// Returns the equivalent LocalValue.
    fn local_value(&self) -> Self::LocalValue;

    /// Adds a site to all elements affected by the remote op.
    fn add_site(&mut self, op: &Self::RemoteOp, site: u32);

    /// Adds a site to all elements
    fn add_site_to_all(&mut self, site: u32);

    /// Validates that all elements have the given site.
    fn validate_site(&self, site: u32) -> Result<(), Error>;

    /// Merges CRDT values
    fn merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones);
}

#[doc(hidden)]
/// Functions for nested CRDT values.
pub trait NestedCrdtValue: CrdtValue {
    /// Adds a site to a value in the CRDT and all its descendants.
    fn nested_add_site(&mut self, op: &<Self as CrdtValue>::RemoteOp, site: u32);

    /// Adds a site to all values and descendants in the CRDT.
    fn nested_add_site_to_all(&mut self, site: u32);

    /// Validates a nested site.
    fn nested_validate_site(&self, site: u32) -> Result<(), Error>;

    /// Merges nested CRDT values.
    fn nested_merge(&mut self, other: Self, self_tombstones: &Tombstones, other_tombstones: &Tombstones) -> Result<(), Error>;
}

/// Required functions for CRDT remote ops.
pub trait CrdtRemoteOp {

    /// Returns a Vec of all replicas deleted by the op.
    fn deleted_replicas(&self) -> Vec<Replica>;

    /// Adds a site to all elements in the op with site 0.
    fn add_site(&mut self, site: u32);

    /// Validates that all inserted elements in the op have the given site.
    fn validate_site(&self, site: u32) -> Result<(), Error>;
}

pub trait NestedCrdtRemoteOp: CrdtRemoteOp {
    /// Add a site to all elements in the nested op.
    fn nested_add_site(&mut self, site: u32);

    /// Validates that all elements in the nested op have the given site.
    fn nested_validate_site(&self, site: u32) -> Result<(), Error>;
}
