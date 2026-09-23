//! Membership commitments issued only by actual original publication owners.
//!
//! Cold capture visits the actual committed and rollback cuts. Hot preparation consumes
//! its actual borrowed net changes and rejects stale/foreign identities before
//! I/O. These capabilities do not establish carrier admission, durable storage,
//! complete State authority, or snapshot authentication by themselves.
//! TODO: retain these roots and the original funded store through complete State
//! preparation, authenticated restore and incremental checkpoint publication;
//! never use cold capture as a per-height fallback for a mismatched baseline.

use super::*;
use iroha_crypto::{
    Hash as Digest, MerkleMapEdit, MerkleMapNodeStore, MerkleMapReadError, MerkleMapRoot,
    MerkleMapUpdateError, MerkleMapUpdateWorkspace, MerkleMapValueRef,
};

#[path = "membership_record.rs"]
pub(in crate::state) mod record;

const KEY_DOMAIN: &[u8] = b"iroha:transaction-membership:key:v1\0";
const VALUE_DOMAIN: &[u8] = b"iroha:transaction-membership:height:v1\0";
const ROOT_DOMAIN: &[u8] = b"iroha:transaction-membership:root:v1\0";

/// Original node storage plus immutable canonical height preimages.
///
/// Heights use the domain-separated digest of their nonzero u64 little-endian
/// representation. The physical owner must keep values available for every
/// retained current/rollback root and old reader, just like its tree nodes.
/// Reads may return untrusted local data: the root reader authenticates it.
/// Successful writes make the exact value readable and preserve old bindings,
/// including across errors/unwind. Repeated writes may return different locations;
/// earlier references remain valid. Allocation, physical Norito framing, durability
/// and retention remain the store owner's responsibility; this interface cannot
/// authorize State publication.
/// A write at a new location does not repair retained roots referring to a lost
/// earlier location. Such repair requires the physical owner's exact-slot custody.
pub(in crate::state) trait MembershipStore: MerkleMapNodeStore {
    /// Load an untrusted canonical height at the authenticated leaf's location.
    fn read_height(
        &mut self,
        reference: &MerkleMapValueRef<Self::ValueLocation>,
    ) -> Result<Option<u64>, Self::Error>;

    /// Retain a canonical height before any new leaf can reference it.
    fn write_height(
        &mut self,
        reference: Digest,
        height: u64,
    ) -> Result<Self::ValueLocation, Self::Error>;
}

/// Operational read failure, distinct from authenticated nonmembership.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(in crate::state) enum MembershipReadError<E> {
    /// The authenticated tree path could not be read.
    #[error(transparent)]
    Tree(#[from] MerkleMapReadError<E>),
    /// The original local value reader failed.
    #[error("membership height read failed: {0}")]
    ValueSource(E),
    /// A present authenticated leaf has lost its physical value.
    #[error("membership height preimage is unavailable: {0}")]
    MissingValue(Digest),
    /// The value does not match the leaf, or lies outside this exact State cut.
    #[error("membership height preimage is invalid: {0}")]
    InvalidValue(Digest),
    /// A valid canonical height cannot be represented on this local platform.
    #[error("membership height exceeds the local platform: {0}")]
    HeightOverflow(u64),
}

/// Authenticated current and rollback roots bound to the actual storage identity.
/// The rollback root authenticates older values shadowed by the latest set.
/// No constructor accepts caller-selected roots or inventory; cloning retains
/// one old version without retaining any resident node graph. Node locations
/// remain explicit and are excluded from the logical commitment; the caller
/// must retain the original store generation backing both cuts.
#[derive(Clone, Debug)]
pub(in crate::state) struct CommittedMembershipRoot<N: Copy> {
    identity: Identity,
    height: u64,
    root: MerkleMapRoot<N>,
    predecessor: MerkleMapRoot<N>,
}

/// An unpublished root issued for exactly one original prepared transition.
/// It survives detachment/reacquisition of that same prepared owner, but cannot
/// authorize another preparation even if its height and payload are equal.
#[derive(Debug)]
pub(in crate::state) struct PreparedMembershipRoot<N: Copy> {
    preparation: Identity,
    after: CommittedMembershipRoot<N>,
}

/// Local preparation failure; none authorizes deterministic transaction rejection.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum MembershipRootError<E> {
    /// The root does not belong to this exact original committed predecessor.
    #[error("membership root belongs to a different publication identity")]
    PredecessorChanged,
    /// A local platform height cannot be represented canonically.
    #[error("membership height exceeds its canonical u64 representation")]
    HeightOverflow,
    /// The original storage transition failed its existing semantic validation.
    #[error(transparent)]
    Membership(#[from] TransactionsBlockError),
    /// External authenticated lookup or immutable path preparation failed.
    #[error(transparent)]
    Update(#[from] MerkleMapUpdateError<E>),
    /// The immutable height must be retained before its referencing leaf.
    #[error("membership height write failed: {0}")]
    ValueWrite(E),
}

fn key_digest(key: &Key) -> Digest {
    Digest::new_from_chunks(&[KEY_DOMAIN, key.as_ref()])
}

fn value_digest<E>(height: Value) -> Result<Digest, MembershipRootError<E>> {
    let height = u64::try_from(height.get()).map_err(|_| MembershipRootError::HeightOverflow)?;
    Ok(canonical_height_digest(height))
}

fn canonical_height_digest(height: u64) -> Digest {
    Digest::new_from_chunks(&[VALUE_DOMAIN, &height.to_le_bytes()])
}

fn retain_height<S: MembershipStore>(
    height: Value,
    store: &mut S,
) -> Result<MerkleMapValueRef<S::ValueLocation>, MembershipRootError<S::Error>> {
    let height = u64::try_from(height.get()).map_err(|_| MembershipRootError::HeightOverflow)?;
    let reference = canonical_height_digest(height);
    let location = store
        .write_height(reference, height)
        .map_err(MembershipRootError::ValueWrite)?;
    Ok(MerkleMapValueRef {
        hash: reference,
        location,
    })
}

fn insert_captured_member<S: MembershipStore>(
    root: &mut MerkleMapRoot<S::NodeLocation>,
    key: &Key,
    value: Value,
    store: &mut S,
    workspace: &mut MerkleMapUpdateWorkspace<S::NodeLocation, S::ValueLocation>,
) -> Result<(), MembershipRootError<S::Error>> {
    let value = retain_height(value, store)?;
    *root = root.replace(
        &root.hash(),
        MerkleMapEdit {
            key: key_digest(key),
            expected: None,
            after: Some(value),
        },
        workspace,
        store,
    )?;
    Ok(())
}

impl<N: Copy> CommittedMembershipRoot<N> {
    /// Bind current membership, its exact rollback cut and committed frontier.
    /// The predecessor frontier is deterministically height minus one, or zero
    /// for the empty genesis boundary. No physical layout enters this commitment.
    pub(in crate::state) fn commitment(&self) -> Digest {
        Digest::new_from_chunks(&[
            ROOT_DOMAIN,
            &self.height.to_le_bytes(),
            self.root.hash().as_ref(),
            self.predecessor.hash().as_ref(),
        ])
    }

    /// Read the exact prior cut, including heights shadowed by the committed tip.
    /// A missing/corrupt node or value remains a typed local failure.
    pub(in crate::state) fn read_predecessor<S: MembershipStore<NodeLocation = N>>(
        &self,
        key: &Key,
        store: &mut S,
    ) -> Result<Option<Value>, MembershipReadError<S::Error>> {
        read_at(&self.predecessor, self.height.saturating_sub(1), key, store)
    }

    /// Authenticate the tree path and height against this original trusted cut.
    /// Only proven tree absence returns `None`; a missing height never does.
    pub(in crate::state) fn read<S: MembershipStore<NodeLocation = N>>(
        &self,
        key: &Key,
        store: &mut S,
    ) -> Result<Option<Value>, MembershipReadError<S::Error>> {
        // Unlike a caller-selected root envelope, this root is issued only by
        // actual cold State capture or the original consuming publication below.
        read_at(&self.root, self.height, key, store)
    }
}

fn read_at<S: MembershipStore>(
    root: &MerkleMapRoot<S::NodeLocation>,
    frontier: u64,
    key: &Key,
    store: &mut S,
) -> Result<Option<Value>, MembershipReadError<S::Error>> {
    let Some(reference) = root.lookup(&root.hash(), &key_digest(key), |node| store.read(node))?
    else {
        return Ok(None);
    };
    let height = store
        .read_height(&reference)
        .map_err(MembershipReadError::ValueSource)?
        .ok_or(MembershipReadError::MissingValue(reference.hash))?;
    if height == 0 || height > frontier || canonical_height_digest(height) != reference.hash {
        return Err(MembershipReadError::InvalidValue(reference.hash));
    }
    let local = usize::try_from(height).map_err(|_| MembershipReadError::HeightOverflow(height))?;
    let height =
        NonZeroUsize::new(local).ok_or(MembershipReadError::InvalidValue(reference.hash))?;
    Ok(Some(height))
}

impl TransactionsBlock<'_> {
    /// Cold-capture the actual committed membership under its original writer.
    /// The caller retains the admitted store/workspace, including through failure
    /// or unwind. Errors leave only unreachable immutable provisional nodes.
    pub(in crate::state) fn capture_committed_root<S: MembershipStore>(
        &self,
        store: &mut S,
        workspace: &mut MerkleMapUpdateWorkspace<S::NodeLocation, S::ValueLocation>,
    ) -> Result<CommittedMembershipRoot<S::NodeLocation>, MembershipRootError<S::Error>> {
        let identity = self._guard.identity();
        let height = self
            .latest_block_ref
            .load()
            .as_ref()
            .map_or(0, |tip| tip.height.get());
        let height = u64::try_from(height).map_err(|_| MembershipRootError::HeightOverflow)?;
        let mut root = MerkleMapRoot::from_parts(0, None);
        self.visit_committed_membership(|key, value| {
            insert_captured_member(&mut root, key, value, store, workspace)
        })?;
        let mut predecessor = MerkleMapRoot::from_parts(0, None);
        self.visit_committed_predecessor_membership(|key, value| {
            insert_captured_member(&mut predecessor, key, value, store, workspace)
        })?;
        Ok(CommittedMembershipRoot {
            identity: identity.clone(),
            height,
            root,
            predecessor,
        })
    }
}

impl PreparedTransactionsBlock<'_> {
    /// Prepare only this admitted owner's actual net membership changes.
    /// No current or historical key set is cloned. The capability's original
    /// identity authenticates untouched entries, not merely touched preimages.
    pub(in crate::state) fn prepare_membership_root<S: MembershipStore>(
        &self,
        baseline: &CommittedMembershipRoot<S::NodeLocation>,
        store: &mut S,
        workspace: &mut MerkleMapUpdateWorkspace<S::NodeLocation, S::ValueLocation>,
    ) -> Result<PreparedMembershipRoot<S::NodeLocation>, MembershipRootError<S::Error>> {
        self.assert_unpublished();
        if !Identity::ptr_eq(self.block._guard.identity(), &baseline.identity) {
            return Err(MembershipRootError::PredecessorChanged);
        }
        let transition = self.block.membership_transition()?;
        let height = u64::try_from(transition.staged_height().get())
            .map_err(|_| MembershipRootError::HeightOverflow)?;
        let mut root = baseline.root;
        transition.visit_committed_changes(|key, before, after| {
            let after = after
                .map(|height| retain_height(height, store))
                .transpose()?;
            root = root.replace(
                &root.hash(),
                MerkleMapEdit {
                    key: key_digest(key),
                    expected: before.map(value_digest).transpose()?,
                    after,
                },
                workspace,
                store,
            )?;
            Ok::<_, MembershipRootError<S::Error>>(())
        })?;
        let identity = if matches!(self.publication, MembershipPublication::Repeated) {
            self.block._guard.identity().clone()
        } else {
            self.next_identity.clone()
        };
        Ok(PreparedMembershipRoot {
            preparation: self.next_identity.clone(),
            after: CommittedMembershipRoot {
                identity,
                height,
                root,
                predecessor: if matches!(&self.publication, MembershipPublication::Advance { .. }) {
                    baseline.root
                } else {
                    baseline.predecessor
                },
            },
        })
    }

    /// Consume exactly the preparation that issued this candidate root.
    /// The committed capability is exposed only after canonical membership
    /// publication completes. Cleanup remains in its original retirement owner.
    /// A mismatched or physically unprepared owner returns both originals without
    /// publishing. The separate fallible physical phase must complete first;
    /// this kernel only consumes already-retained physical authority. The caller
    /// retains refusal/retirement custody beyond every enclosing State fence.
    pub(in crate::state) fn publish_with_membership_root<N: Copy>(
        self,
        root: PreparedMembershipRoot<N>,
    ) -> Result<
        (
            CommittedMembershipRoot<N>,
            TransactionsPublicationRetirement,
        ),
        (Self, PreparedMembershipRoot<N>),
    > {
        if !Identity::ptr_eq(&self.next_identity, &root.preparation)
            || !self.is_physically_prepared()
        {
            return Err((self, root));
        }
        let retirement = self.publish_prepared();
        Ok((root.after, retirement))
    }
}

impl<Installation> PreparedDetachedTransactionsBlock<'_, Installation> {
    /// Publish the same root after the original detached writer was reacquired.
    /// Installation admission and cleanup keep their existing aggregate owner.
    pub(in crate::state) fn publish_with_membership_root<N: Copy>(
        self,
        root: PreparedMembershipRoot<N>,
    ) -> Result<
        (
            CommittedMembershipRoot<N>,
            PublishedTransactions<Installation>,
        ),
        (Self, PreparedMembershipRoot<N>),
    > {
        if !Identity::ptr_eq(&self.prepared.next_identity, &root.preparation) {
            return Err((self, root));
        }
        let retirement = self.publish();
        Ok((root.after, retirement))
    }
}

#[cfg(test)]
#[path = "membership_root_tests.rs"]
mod tests;
