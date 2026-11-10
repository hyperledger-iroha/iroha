//! Startup verifies the State reconstructed by executing the original Kura history.
use super::*;
use crate::state::authority_registry::original_stream::capture_committed_keyed_image;
use crate::state::canonical_runtime::CanonicalRuntimeAcquisitionError;
use crate::{execution_attempt::ExecutionAttemptError, sumeragi::certified_chain::CertifiedChain};

impl State {
    /// Verify the selected State commitment at the end of native replay. This is an
    /// audit of an already executed/published cut; a snapshot cannot initialize it.
    ///
    /// # Errors
    /// Incomplete history, a different native tip or unavailable original storage;
    /// a cold State capture that disagrees with replayed publication.
    pub(crate) fn verify_replayed_state_commitment(
        &self,
    ) -> Result<KeyedStateRoot, WorldStateVerificationError> {
        use WorldStateVerificationError as Error;
        let view = self.view();
        if !runtime::keyed_state::selected(view.world().runtime_selection()) {
            drop(view);
            return self.verify_world_state_accumulator().map(Into::into);
        }
        let tip = view
            .native_execution_tip()
            .ok_or_else(|| Error::Capture("missing replayed keyed State execution".into()))?;
        let cut = self
            .keyed_versions
            .lock()
            .published_at(tip)
            .cloned()
            .ok_or_else(|| Error::Capture("missing original keyed State publication".into()))?;
        // The original worker replayed this exact result. At genesis the signed body
        // authenticates only the input, so matching the locally reproduced R is essential.
        // At later heights the native chain also verifies the CommitQC and its committee.
        let chain = CertifiedChain::new(&view).map_err(history_error)?;
        let block = if tip.height() == 1 {
            chain.committed(1).map_err(history_error)?
        } else {
            // Native replay has already authenticated the prefix. The terminal QC
            // can be independently checked from its original parent in bounded work.
            let index = usize::try_from(tip.height())
                .ok()
                .and_then(std::num::NonZeroUsize::new)
                .ok_or_else(|| {
                    Error::Capture("keyed replay height exceeds host capacity".into())
                })?;
            chain
                .certified_from_execution(index, |_, _| Ok(()))
                .map_err(history_error)?
                .into_committed()
        };
        if view.latest_block_hash() != Some(tip.iroha_hash())
            || block.block_hash() != tip.iroha_hash()
            || block.core_hash() != tip.core_hash()
            || block.result() != tip.result()
            || block.root_scope() != tip.root_scope()
            || block.commitment().execution.world_state_root.payload() != &cut.execution.root()
        {
            return Err(Error::Capture(
                "keyed execution differs from original native replay history".into(),
            ));
        }
        let header = block.block().header();
        drop(block);
        drop(chain);
        drop(view);
        // Acquire a pristine version containing the exact committed State. Ordinary
        // block-start hooks (including due work) would already mutate the captured state.
        let acquired =
            self.acquire_canonical_runtime_block(false)
                .map_err(|error| match error {
                    CanonicalRuntimeAcquisitionError::Admission(original) => {
                        Error::KeyedAdmission(Box::new(original))
                    }
                    CanonicalRuntimeAcquisitionError::Runtime(
                        crate::state::LaneLifecycleError::ParameterDeferred(original),
                    ) => Error::KeyedDeferred(original),
                    CanonicalRuntimeAcquisitionError::Runtime(other) => {
                        Error::Capture(other.to_string())
                    }
                })?;
        let snapshot = self.construct_acquired_block(acquired, header, Box::new);
        let fields = snapshot
            .fields
            .as_ref()
            .expect("pristine original acquisition");
        if *fields.native_execution_tip.get() != Some(tip)
            || fields.world.runtime_selection.get()
                != fields.world.runtime_selection.get_before_block()
        {
            return Err(Error::Capture(
                "State changed while validating its replayed publication".into(),
            ));
        }
        let schema = schema_for(fields.world.runtime_selection.get()).map_err(Error::Capture)?;
        if cut.schema.digest != schema.digest
            || cut.execution.schema_digest() != schema.digest
            || cut.published.schema_digest() != schema.digest
        {
            return Err(Error::Capture(
                "replayed keyed schema differs from the selected publication".into(),
            ));
        }
        let cold =
            capture_committed_keyed_image(fields, &schema).map_err(|error| match capture_error(
                error,
            ) {
                CutError::Invalid(reason) => Error::Capture(reason),
                CutError::Deferred(reason) => Error::KeyedDeferred(reason),
            })?;
        let actual = KeyedStateRoot(cold.root());
        let retained = KeyedStateRoot(cut.published.root());
        if actual != retained {
            return Err(Error::KeyedMismatch {
                retained,
                captured: actual,
            });
        }
        Ok(actual)
    }
}

fn history_error<E: std::fmt::Display>(
    error: ExecutionAttemptError<E>,
) -> WorldStateVerificationError {
    match error {
        ExecutionAttemptError::Rejected(reason) => {
            WorldStateVerificationError::Capture(reason.to_string())
        }
        ExecutionAttemptError::Deferred(reason) => {
            WorldStateVerificationError::KeyedDeferred(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    #[test]
    fn keyed_execution_requires_the_policy_at_its_published_predecessor() {
        let mut world = World::new();
        runtime::keyed_state::install_for_test(&mut world);
        let chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
        let state = chain.state();
        let genesis = chain.committed(1);
        let next = BlockHeader::new(
            std::num::NonZeroU64::new(2).unwrap(),
            Some(genesis.block_hash()),
            None,
            2000,
            0,
        );
        let cut = state
            .keyed_versions
            .lock()
            .current
            .as_ref()
            .unwrap()
            .clone();
        let mut foreign = cut.clone();
        let policy = StateKeyV1 {
            table: "state.gov".into(),
            key: Vec::new(),
        };
        foreign
            .published
            .replace(&policy, Some(b"foreign policy"))
            .unwrap();
        state.keyed_versions.lock().current = Some(foreign);
        assert!(matches!(
            state.try_block(next),
            Err(StateBlockStartError::Storage(StateStorageAdmissionError::UnsupportedRuntime { reason, .. }))
                if reason.contains("state.gov")
        ));
        state.keyed_versions.lock().current = Some(cut);
        assert!(state.try_block(next).is_ok());
    }

    #[test]
    fn keyed_replay_checks_terminal_native_prefix_and_recomputes_publication() {
        let mut world = World::new();
        runtime::keyed_state::install_for_test(&mut world);
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
        // A signed genesis is independently replayed here; no successor is available.
        let genesis = chain.state().verify_replayed_state_commitment().unwrap();
        assert_eq!(
            genesis.payload(),
            &chain
                .state()
                .keyed_versions
                .lock()
                .current
                .as_ref()
                .unwrap()
                .published
                .root()
        );
        chain.commit(Vec::new());
        let state = chain.state();
        let native = state.view().native_execution_tip().unwrap();
        assert_eq!(native.height(), 2);
        let validated = state.verify_replayed_state_commitment().unwrap();
        let cut = state
            .keyed_versions
            .lock()
            .published_at(native)
            .unwrap()
            .clone();
        assert_eq!(&cut.published.root(), validated.payload());
        let bad_key = StateKeyV1 {
            table: "state.network_id".into(),
            key: Vec::new(),
        };
        let mut bad = cut.clone();
        bad.published.replace(&bad_key, Some(&[0xba])).unwrap();
        state.keyed_versions.lock().current = Some(bad);
        assert!(matches!(
            state.verify_replayed_state_commitment(),
            Err(WorldStateVerificationError::KeyedMismatch { .. })
        ));
        let mut bad = cut.clone();
        bad.execution.replace(&bad_key, Some(&[0xba])).unwrap();
        state.keyed_versions.lock().current = Some(bad);
        assert!(matches!(
            state.verify_replayed_state_commitment(),
            Err(WorldStateVerificationError::Capture(_))
        ));
        state.keyed_versions.lock().current = None;
        assert!(state.verify_replayed_state_commitment().is_err());
    }
}
