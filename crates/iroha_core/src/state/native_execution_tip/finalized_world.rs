//! Exact predecessor World authenticated by the original successor certificate.
//!
//! This is a World-only receipt. State-owned runtime, membership and semantic
//! configuration are not in this root, and no complete-State or IVM admission
//! capability is constructed here. The execution-prefix root at H is not the
//! published World at H: the latter is bound by H+1's parent World root.
//! TODO: consume this original-source check only after complete State authority,
//! bounded capture work, allocation custody and durable witness-node ownership
//! are implemented. It must never promote an IvmCompleteStateRootClaimV1.

use super::*;
use crate::state::world_projection::WorldStateAccumulator;
use crate::sumeragi::certified_chain::{
    AuthenticatedExecutionBlock, NativeExecutionReadError, NativeExecutionReadLimits,
    read_authenticated_execution,
};

/// A failed attempt returns no partially authenticated World receipt.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum PredecessorWorldError {
    /// State has no published predecessor with a native successor certificate.
    #[error("no finalized predecessor World is available")]
    Unavailable,
    /// The generation-bound State view retains its original read refusal or invalidity.
    #[error("State view: {0}")]
    View(#[from] crate::state::StateViewError),
    /// The original native tips do not match the authenticated source chain.
    #[error("original State execution identity differs from its native source")]
    Identity,
    /// Exact source reading, bounds or native finality verification failed.
    #[error(transparent)]
    Source(#[from] NativeExecutionReadError),
    /// The original State pool could not acquire its retained predecessor.
    #[error("World acquisition: {0}")]
    Acquisition(mv::storage::AdmittedStorageError),
    /// Canonical capture or encoding failed.
    #[error("World capture: {0}")]
    Capture(String),
    /// The actual predecessor rows differ from the authenticated successor root.
    #[error("predecessor World differs from the authenticated successor parent root")]
    WorldMismatch,
    /// A receipt from another State cannot authorize this consumer.
    #[error("predecessor World receipt belongs to another State")]
    ForeignState,
    /// A subsequent publication invalidates current-predecessor use.
    #[error("predecessor World receipt no longer names the current State generation")]
    Stale,
}

/// World-only source receipt with no decoder, public constructor or root promotion.
pub(in crate::state) struct PredecessorWorldReceipt<'state> {
    state: &'state State,
    generation: u64,
    predecessor: NativeExecutionTip,
    successor: AuthenticatedExecutionBlock,
    world_root: Hash,
}

impl PredecessorWorldReceipt<'_> {
    /// Check exact owner and generation before a current-predecessor consumer uses it.
    pub(in crate::state) fn require_current(
        &self,
        state: &State,
    ) -> Result<(), PredecessorWorldError> {
        if !core::ptr::eq(self.state, state) {
            return Err(PredecessorWorldError::ForeignState);
        }
        if !is_stable_state_view_generation(self.generation, state.state_view_generation()) {
            return Err(PredecessorWorldError::Stale);
        }
        Ok(())
    }

    /// Compare untrusted World-only claims after checking exact current owner and generation.
    /// A successful match still does not grant complete-State or IVM admission.
    pub(in crate::state) fn matches_world_claim(
        &self,
        state: &State,
        height: u64,
        block_hash: HashOf<BlockHeader>,
        world_root: Hash,
    ) -> Result<bool, PredecessorWorldError> {
        self.require_current(state)?;
        Ok(self.predecessor.height() == height
            && self.predecessor.iroha_hash() == block_hash
            && self.world_root == world_root)
    }
}

impl State {
    /// Resolve the latest retained predecessor through this State's own native source.
    ///
    /// Contention retains the original State view's release observation. The caller
    /// supplies finite source allowances, never a hash cut, root, committee,
    /// certificate or World snapshot. Every original native source frame is admitted before I/O.
    /// This internal prerequisite is not a complete-State root or a private-execution
    /// anchor. Cold capture remains O(World); it is not a public query entrypoint.
    pub(in crate::state) fn predecessor_world_receipt_once(
        &self,
        limits: NativeExecutionReadLimits,
    ) -> Result<Option<PredecessorWorldReceipt<'_>>, PredecessorWorldError> {
        let publication_release = self.view_publication_release();
        let generation = self.state_view_generation();
        if generation & 1 != 0 {
            return Err(crate::state::StateViewError::Busy(publication_release).into());
        }
        let view = self.try_view_once()?;
        let current = view
            .native_execution_tip()
            .ok_or(PredecessorWorldError::Unavailable)?;
        let previous = (*view.native_execution_tip_predecessor.get())
            .flatten()
            .ok_or(PredecessorWorldError::Unavailable)?;
        if previous.height().checked_add(1) != Some(current.height())
            || usize::try_from(current.height()).ok() != Some(view.block_hashes.len())
            || view.block_hashes.last().copied() != Some(current.iroha_hash())
        {
            return Err(PredecessorWorldError::Identity);
        }
        let source = read_authenticated_execution(
            &self.kura,
            view.chain_id(),
            *view.network_id(),
            view.block_hashes(),
            current.height(),
            limits,
            &self.ivm_execution_budget(),
        )?;
        let committed = source.authority.committed();
        if record(committed) != current.0
            || committed.block().header().prev_block_hash() != Some(previous.iroha_hash())
            || !committed.header().is_some_and(|header| {
                header.parent_hash == previous.core_hash()
                    && header.parent_result == previous.result()
            })
        {
            return Err(PredecessorWorldError::Identity);
        }
        let expected = committed.commitment().execution.parent_world_state_root;
        drop(view);
        // A replacement overlay exposes each original retained H-1 row. It is
        // never applied; dropping it preserves both committed current and undo.
        let world = self
            .world
            .try_block_and_revert(&self.ivm_execution_budget())
            .map_err(PredecessorWorldError::Acquisition)?;
        let captured =
            WorldStateAccumulator::capture(&world).map_err(PredecessorWorldError::Capture)?;
        let world_root = captured.root().map_err(PredecessorWorldError::Capture)?;
        let stored_root = world
            .state_accumulator
            .get()
            .root()
            .map_err(PredecessorWorldError::Capture)?;
        drop(world);
        if !is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err(crate::state::StateViewError::Busy(publication_release).into());
        }
        if world_root != stored_root || world_root != expected {
            return Err(PredecessorWorldError::WorldMismatch);
        }
        Ok(Some(PredecessorWorldReceipt {
            state: self,
            generation,
            predecessor: previous,
            successor: source.authority,
            world_root,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};

    fn chain() -> CertifiedTestChain {
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
    }

    fn limits(chain: &CertifiedTestChain) -> NativeExecutionReadLimits {
        let height = chain.state().view().height() as u64;
        NativeExecutionReadLimits {
            admitted_target_wire_bytes: chain.committed(height).block().encode_wire().unwrap().len()
                as u64,
            max_source_blocks: 16,
            max_source_wire_bytes: 128 * 1024 * 1024,
            max_frame_wire_bytes: 16 * 1024 * 1024,
        }
    }

    #[test]
    fn exact_original_successor_binds_published_predecessor_world_without_promoting_state() {
        let mut chain = chain();
        let previous = chain.state().verify_world_state_accumulator().unwrap();
        let genesis_hash = chain.genesis().hash();
        chain.commit(Vec::new());
        let current = chain.state().verify_world_state_accumulator().unwrap();
        let original_tip = chain.state().view().native_execution_tip();
        let receipt = chain
            .state()
            .predecessor_world_receipt_once(limits(&chain))
            .unwrap()
            .unwrap();
        assert!(
            receipt
                .matches_world_claim(chain.state(), 1, genesis_hash, previous)
                .unwrap()
        );
        assert_eq!(receipt.successor.committed().height(), 2);
        assert_eq!(
            receipt.world_root,
            receipt
                .successor
                .committed()
                .commitment()
                .execution
                .parent_world_state_root
        );
        assert!(receipt.require_current(chain.state()).is_ok());
        assert!(
            !receipt
                .matches_world_claim(chain.state(), 2, genesis_hash, previous)
                .unwrap()
        );
        assert!(
            !receipt
                .matches_world_claim(
                    chain.state(),
                    1,
                    HashOf::from_untyped_unchecked(Hash::new(b"substituted block")),
                    previous
                )
                .unwrap()
        );
        assert!(
            !receipt
                .matches_world_claim(
                    chain.state(),
                    1,
                    genesis_hash,
                    Hash::new(b"substituted World")
                )
                .unwrap()
        );
        assert_eq!(chain.state().view().native_execution_tip(), original_tip);
        assert_eq!(
            chain.state().verify_world_state_accumulator().unwrap(),
            current
        );
    }

    #[test]
    fn genesis_and_unfinalized_proposal_cannot_supply_a_predecessor_receipt() {
        let mut chain = chain();
        assert!(matches!(
            chain.state().predecessor_world_receipt_once(limits(&chain)),
            Err(PredecessorWorldError::Unavailable)
        ));
        let proposal = chain.proposal(None, Vec::new());
        let mut original = chain
            .begin_proposal(proposal, iroha_sumeragi::types::ControlWitness::default())
            .unwrap();
        assert!(original.prepare(Signers::BelowQuorum).is_err());
        drop(original);
        assert!(matches!(
            chain.state().predecessor_world_receipt_once(limits(&chain)),
            Err(PredecessorWorldError::Unavailable)
        ));
    }

    #[test]
    fn receipt_rejects_foreign_state_and_new_publication_generation() {
        let mut chain = chain();
        chain.commit(Vec::new());
        let state = Arc::clone(chain.state());
        let receipt = state
            .predecessor_world_receipt_once(limits(&chain))
            .unwrap()
            .unwrap();
        let other = self::chain();
        assert!(matches!(
            receipt.require_current(other.state()),
            Err(PredecessorWorldError::ForeignState)
        ));
        chain.commit(Vec::new());
        assert!(matches!(
            receipt.require_current(&state),
            Err(PredecessorWorldError::Stale)
        ));
        assert!(matches!(
            receipt.matches_world_claim(
                &state,
                receipt.predecessor.height(),
                receipt.predecessor.iroha_hash(),
                receipt.world_root
            ),
            Err(PredecessorWorldError::Stale)
        ));
    }

    #[test]
    fn source_bounds_corrupt_carrier_and_busy_publication_return_no_receipt() {
        let mut chain = chain();
        chain.commit(Vec::new());
        let original_limits = limits(&chain);
        assert!(matches!(
            chain
                .state()
                .predecessor_world_receipt_once(NativeExecutionReadLimits {
                    max_source_blocks: 1,
                    ..original_limits
                }),
            Err(PredecessorWorldError::Source(
                NativeExecutionReadError::Capacity { .. }
            ))
        ));
        let mut publication = chain.state().state_view_publication();
        let guard = publication.begin();
        assert!(matches!(
            chain
                .state()
                .predecessor_world_receipt_once(original_limits),
            Err(PredecessorWorldError::View(
                crate::state::StateViewError::Busy(_)
            ))
        ));
        drop(guard);
        drop(publication);
        let original = chain.state().latest_block_header.write();
        let expected = chain
            .state()
            .latest_block_header
            .try_read_or_wait()
            .err()
            .unwrap();
        let Err(PredecessorWorldError::View(crate::state::StateViewError::Busy(actual))) = chain
            .state()
            .predecessor_world_receipt_once(original_limits)
        else {
            panic!("predecessor read must preserve the original physical reader refusal");
        };
        assert_eq!(actual, expected);
        drop(original);
        assert!(
            chain
                .state()
                .predecessor_world_receipt_once(original_limits)
                .unwrap()
                .is_some()
        );
        chain
            .kura()
            .corrupt_native_frame_for_test(NonZeroUsize::new(2).unwrap());
        assert!(matches!(
            chain
                .state()
                .predecessor_world_receipt_once(original_limits),
            Err(PredecessorWorldError::Source(_))
        ));
    }

    #[test]
    fn actual_world_predecessor_substitution_cannot_reuse_the_native_certificate() {
        let mut chain = chain();
        chain.commit(Vec::new());
        let limits = limits(&chain);
        // An illicit direct World publication replaces its undo cut but cannot
        // manufacture the opaque NativeExecutionTip or a matching certificate.
        let mut world = chain.state().world.block();
        world.account_roles.insert(
            crate::role::RoleIdWithOwner::new(
                iroha_test_samples::ALICE_ID.clone(),
                "unrelated_role".parse().unwrap(),
            ),
            (),
        );
        world.commit();
        assert!(matches!(
            chain.state().predecessor_world_receipt_once(limits),
            Err(PredecessorWorldError::WorldMismatch)
        ));
    }
}
