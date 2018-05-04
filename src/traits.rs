use Error;
use dot::{Summary, SiteId};

macro_rules! crdt_impl2 {
    ($self_ident:ident,
     $state:ty,
     $state_static:ty,
     $state_ident:ident,
     $inner:ty,
     $op:ty,
     $local_op:ty,
     $local_value:ty,
    ) => {
        /// Returns the site id.
        pub fn site_id(&self) -> SiteId {
            self.site_id
        }

        #[doc(hidden)]
        pub fn summary(&self) -> &Summary {
            &self.summary
        }

        #[doc(hidden)]
        pub fn cached_ops(&self) -> &[$op] {
            &self.cached_ops
        }

        /// Returns a borrowed CRDT state.
        pub fn state(&self) -> $state {
            $state_ident {
                inner: Cow::Borrowed(&self.inner),
                summary: Cow::Borrowed(&self.summary),
            }
        }

        /// Returns an owned CRDT state of cloned values.
        pub fn clone_state(&self) -> $state_static {
            $state_ident {
                inner: Cow::Owned(self.inner.clone()),
                summary: Cow::Owned(self.summary.clone()),
            }
        }

        /// Consumes the CRDT and returns its state
        pub fn into_state(self) -> $state_static {
            $state_ident {
                inner: Cow::Owned(self.inner),
                summary: Cow::Owned(self.summary),
            }
        }

        /// Constructs a new CRDT from a state and optional site id.
        /// If the site id is present, it must be nonzero.
        pub fn from_state(state: $state, site_id: Option<SiteId>) -> Result<Self, Error> {
            let site_id = match site_id {
                None => 0,
                Some(0) => return Err(Error::InvalidSiteId),
                Some(s) => s,
            };

            Ok($self_ident{
                site_id,
                inner: state.inner.into_owned(),
                summary: state.summary.into_owned(),
                outoforder_ops: vec![],
                cached_ops: vec![],
            })
        }

        /// Returns the CRDT value's equivalent local value.
        pub fn local_value(&self) -> $local_value {
            self.inner.local_value()
        }

        /// Executes an op and returns the equivalent local op.
        /// This function assumes that the op always inserts values
        /// from the correct site. For untrusted ops, used `validate_and_execute_op`.
        pub fn execute_op(&mut self, op: $op) -> Vec<$local_op> {
            use traits::IntoVec;

            for dot in op.inserted_dots() {
                self.summary.insert(dot);
            }

            if Self::is_outoforder(&op, &self.summary) {
                self.outoforder_ops.push(op);
                return vec![]
            }

            let mut local_ops: Vec<$local_op> = self.inner.execute_op(op).into_vec();

            while let Some(op) = self.pop_outoforder_op() {
                local_ops.append(&mut self.inner.execute_op(op).into_vec());
            }

            local_ops
        }

        /// Validates that an op only inserts elements from a given site id,
        /// then executes the op and returns the equivalent local op.
        pub fn validate_and_execute_op(&mut self, op: $op, site_id: SiteId) -> Result<Vec<$local_op>, Error> {
            op.validate(site_id)?;
            Ok(self.execute_op(op))
        }

        /// Merges a remote CRDT state into the CRDT. The remote
        /// CRDT state must have a site id.
        pub fn merge(&mut self, other: $state) -> Result<(), Error> {
            other.inner.validate_no_unassigned_sites()?;
            other.summary.validate_no_unassigned_sites()?;
            self.inner.merge(other.inner.into_owned(), &self.summary, &other.summary);
            self.summary.merge(&other.summary);
            Ok(())
        }

        /// Assigns a site id to the CRDT and returns any cached ops.
        /// If the CRDT already has a site id, it returns an error.
        pub fn add_site_id(&mut self, site_id: SiteId) -> Result<Vec<$op>, Error> {
            if self.site_id != 0 {
                return Err(Error::AlreadyHasSiteId);
            }

            self.site_id = site_id;
            self.inner.add_site_id(site_id);
            self.summary.add_site_id(site_id);
            Ok(::std::mem::replace(&mut self.cached_ops, vec![])
                .into_iter()
                .map(|mut op| { op.add_site_id(site_id); op})
                .collect())
        }

        fn is_outoforder(op: &$op, summary: &Summary) -> bool {
            op.removed_dots().iter().any(|dot| !summary.contains(dot))
        }

        fn pop_outoforder_op(&mut self) -> Option<$op> {
            let idx = self.outoforder_ops.iter().position(|op| Self::is_outoforder(&op, &self.summary))?;
            Some(self.outoforder_ops.remove(idx))
        }

        fn after_op(&mut self, op: $op) -> Result<$op, Error> {
            if self.site_id == 0 {
                self.cached_ops.push(op);
                Err(Error::AwaitingSiteId)
            } else {
                Ok(op)
            }
        }
    }
}

pub(crate) trait IntoVec<T> {
    fn into_vec(self) -> Vec<T>;
}

impl<T> IntoVec<T> for Option<T> {
    fn into_vec(self) -> Vec<T> {
        if let Some(value) = self { vec![value] } else { vec![] }
    }
}

impl<T> IntoVec<T> for T {
    fn into_vec(self) -> Vec<T> {
        vec![self]
    }
}

impl<T> IntoVec<T> for Vec<T> {
    fn into_vec(self) -> Vec<T> { self }
}

pub(crate) trait NestedInner: Sized {
    fn nested_add_site_id(&mut self, site_id: SiteId);

    fn nested_validate_all(&self, site_id: SiteId) -> Result<(), Error>;

    fn nested_validate_no_unassigned_sites(&self) -> Result<(), Error>;

    fn nested_can_merge(&self, other: &Self) -> bool;

    fn nested_merge(&mut self, other: Self, summary: &Summary, other_summary: &Summary) -> Result<(), Error> {
        if self.nested_can_merge(&other) {
            self.nested_force_merge(other, summary, other_summary);
            Ok(())
        } else {
            Err(Error::CannotMerge)
        }
    }

    fn nested_force_merge(&mut self, other: Self, summary: &Summary, other_summary: &Summary);
}

pub(crate) trait NestedOp {
    fn nested_add_site_id(&mut self, site_id: SiteId);

    fn nested_validate(&self, site_id: SiteId) -> Result<(), Error>;
}
