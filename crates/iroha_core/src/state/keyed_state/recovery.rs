//! Startup verifies the State reconstructed by executing the original Kura history, and
//! authenticates a State decoded from a snapshot before any block is replayed onto it.
use super::*;
use crate::state::authority_registry::original_stream::capture_committed_keyed_image;
use crate::state::canonical_runtime::CanonicalRuntimeAcquisitionError;
use crate::{execution_attempt::ExecutionAttemptError, sumeragi::certified_chain::CertifiedChain};

impl State {
    /// Authenticate a State decoded from a snapshot under the keyed commitment and install
    /// its published version (`specs/sumeragi.md` §16.5, E56). Returns whether a snapshot
    /// claim was discharged; a State that carries none is left untouched.
    ///
    /// The certified successor of the restored tip binds `root(P_h)` over the whole selected
    /// schema as its parent State root. A cold build of that schema from the restored State
    /// must reproduce it, which covers the World and every State-level field: transaction
    /// membership, commit topologies, the canonical runtime, chain and network identity,
    /// lane policy and the policy cells. The policy cells still come from node configuration
    /// (G1-D3), so the caller installs that configuration first; a node whose configuration
    /// differs from the committed policy fails here exactly as it would on a full replay.
    ///
    /// # Errors
    /// The restored tip is not the stored block the claim names, the State does not select
    /// the keyed commitment, original storage is locally unavailable, or the rebuilt root
    /// differs from the certified one. The claim survives a local refusal for a retry.
    pub(crate) fn authenticate_restored_keyed_state(
        &self,
    ) -> Result<bool, WorldStateVerificationError> {
        use WorldStateVerificationError as Error;
        let Some(claim) = self.keyed_versions.lock().restored_claim() else {
            return Ok(false);
        };
        let view = self.view();
        if view.native_execution_tip() != Some(claim.tip)
            || view.latest_block_hash() != Some(claim.tip.iroha_hash())
            || !runtime::keyed_state::selected(view.world().runtime_selection())
        {
            return Err(Error::Capture(
                "restored keyed State differs from its snapshot claim".into(),
            ));
        }
        // The native tip restore verified the certified prefix through this height and its
        // successor. Only the stored header is read here; the pinned identity binds it.
        let chain = CertifiedChain::new(&view).map_err(history_error)?;
        let block = chain.committed(claim.tip.height()).map_err(history_error)?;
        if block.block_hash() != claim.tip.iroha_hash()
            || block.core_hash() != claim.tip.core_hash()
            || block.result() != claim.tip.result()
            || block.root_scope() != claim.tip.root_scope()
        {
            return Err(Error::Capture(
                "restored keyed State tip differs from the stored certified block".into(),
            ));
        }
        let header = block.block().header();
        drop(block);
        drop(chain);
        drop(view);
        let (schema, published) = self.cold_committed_keyed_image(claim.tip, header)?;
        let captured = KeyedStateRoot(published.root());
        if captured != claim.certified {
            return Err(Error::RestoredKeyedMismatch {
                height: claim.tip.height(),
                certified: claim.certified,
                captured,
            });
        }
        let mut versions = self.keyed_versions.lock();
        // Publication cannot have run on a State without a version at its tip.
        if versions.restored_claim().map(|pending| pending.tip) != Some(claim.tip) {
            return Err(Error::Capture(
                "restored keyed State claim changed during authentication".into(),
            ));
        }
        drop(versions.install(Some(KeyedCut {
            tip: claim.tip,
            execution: None,
            published,
            schema,
        })));
        Ok(true)
    }

    /// Build the complete selected State published at `tip` from a pristine acquisition of
    /// the committed fields. `header` is the stored header of that tip.
    fn cold_committed_keyed_image(
        &self,
        tip: NativeExecutionTip,
        header: BlockHeader,
    ) -> Result<(Arc<SelectedStateSchema>, KeyedStateTreeV1), WorldStateVerificationError> {
        use WorldStateVerificationError as Error;
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
                "State changed while rebuilding its committed keyed publication".into(),
            ));
        }
        let schema = schema_for(fields.world.runtime_selection.get()).map_err(Error::Capture)?;
        let cold =
            capture_committed_keyed_image(fields, &schema).map_err(|error| match capture_error(
                error,
            ) {
                CutError::Invalid(reason) => Error::Capture(reason),
                CutError::Deferred(reason) => Error::KeyedDeferred(reason),
            })?;
        Ok((schema, cold))
    }

    /// Verify the selected State commitment at the end of native replay. This is an
    /// audit of an already executed/published cut; it never initializes one.
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
        // Replay always executes the certified successor of a restored height, so the tip
        // audited here was executed natively and retains its pre-tail version.
        let Some(execution) = cut.execution.as_ref() else {
            return Err(Error::Capture(
                "keyed State restored at the Kura tip was never replayed".into(),
            ));
        };
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
            || block.commitment().execution.world_state_root.payload() != &execution.root()
        {
            return Err(Error::Capture(
                "keyed execution differs from original native replay history".into(),
            ));
        }
        let header = block.block().header();
        drop(block);
        drop(chain);
        drop(view);
        let (schema, cold) = self.cold_committed_keyed_image(tip, header)?;
        if cut.schema.digest != schema.digest
            || execution.schema_digest() != schema.digest
            || cut.published.schema_digest() != schema.digest
        {
            return Err(Error::Capture(
                "replayed keyed schema differs from the selected publication".into(),
            ));
        }
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
        bad.execution
            .as_mut()
            .unwrap()
            .replace(&bad_key, Some(&[0xba]))
            .unwrap();
        state.keyed_versions.lock().current = Some(bad);
        assert!(matches!(
            state.verify_replayed_state_commitment(),
            Err(WorldStateVerificationError::Capture(_))
        ));
        state.keyed_versions.lock().current = None;
        assert!(state.verify_replayed_state_commitment().is_err());
    }

    use crate::{snapshot::CapturedStateSnapshot, state::deserialize::StateRestoreError};

    /// Decode a snapshot over the chain's Kura, then install the node configuration the
    /// source executed under, as the daemon does between the decode and node preparation.
    fn decode(chain: &CertifiedTestChain, snapshot: &str) -> Result<Box<State>, StateRestoreError> {
        let source = chain.state();
        let mut state = crate::state::deserialize::KuraSeed {
            execution_budget: iroha_allocation::AllocationBudget::new(
                iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
            ),
            lane_manifests: source.lane_manifests.read().clone(),
            kura: Arc::clone(chain.kura()),
            query_handle: crate::query::store::LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json_str_with_configured_nexus(snapshot, source.nexus_snapshot())?;
        state.set_gov(source.gov.clone());
        state.set_pipeline(iroha_config::parameters::actual::Pipeline::default());
        state.install_node_fraud_monitoring(
            iroha_config::parameters::actual::FraudMonitoring::default(),
        );
        Ok(state)
    }

    /// A keyed chain at height 3 and the snapshot its State had at height 2.
    fn keyed_chain_with_snapshot() -> (CertifiedTestChain, CapturedStateSnapshot) {
        let mut world = World::new();
        runtime::keyed_state::install_for_test(&mut world);
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1000)).unwrap();
        chain.commit(Vec::new());
        let snapshot = CapturedStateSnapshot::capture(chain.state()).unwrap();
        // The newest height has no certified successor to bind its published State.
        assert!(matches!(
            decode(&chain, snapshot.as_json()),
            Err(StateRestoreError::SuccessorRequired { height: 2 })
        ));
        chain.commit(Vec::new());
        (chain, snapshot)
    }

    fn header_after(chain: &CertifiedTestChain, height: u64) -> BlockHeader {
        let parent = chain.committed(height);
        BlockHeader::new(
            std::num::NonZeroU64::new(height + 1).unwrap(),
            Some(parent.block_hash()),
            None,
            parent.block_time_ms() + 1_000,
            0,
        )
    }

    #[test]
    fn keyed_snapshot_is_a_claim_until_its_complete_state_matches_the_certified_successor() {
        let (chain, snapshot) = keyed_chain_with_snapshot();
        let certified = chain
            .committed(3)
            .commitment()
            .execution
            .parent_world_state_root;
        let restored = decode(&chain, snapshot.as_json()).unwrap();
        assert_eq!(restored.committed_height(), 2);
        let tip = restored.view().native_execution_tip().unwrap();
        assert_eq!(tip.height(), 2);
        // The decoded State has no version at its tip: nothing executes on the claim.
        assert!(restored.keyed_versions.lock().published_at(tip).is_none());
        assert!(matches!(
            restored.try_block(header_after(&chain, 2)),
            Err(StateBlockStartError::Storage(StateStorageAdmissionError::UnsupportedRuntime { reason, .. }))
                if reason.contains("native replay")
        ));
        assert!(restored.authenticate_restored_keyed_state().unwrap());
        let cut = restored
            .keyed_versions
            .lock()
            .published_at(tip)
            .cloned()
            .unwrap();
        assert_eq!(KeyedStateRoot(cut.published.root()), certified);
        // The same published State that original execution retained at that height.
        let original = chain
            .state()
            .keyed_versions
            .lock()
            .published_at(tip)
            .cloned()
            .unwrap();
        assert_eq!(cut.published.root(), original.published.root());
        assert_eq!(cut.schema.digest, original.schema.digest);
        // Restoration rebuilds P_h only: the restored height serves no row proof.
        assert!(cut.execution.is_none() && original.execution.is_some());
        // The claim is discharged once, and the successor is admitted on the restored parent.
        assert!(!restored.authenticate_restored_keyed_state().unwrap());
        assert!(restored.try_block(header_after(&chain, 2)).is_ok());
    }

    #[test]
    fn keyed_restore_rejects_state_level_and_world_differences() {
        let (chain, snapshot) = keyed_chain_with_snapshot();
        // A State-level field outside World, which no LtHash root ever covered.
        let restored = decode(&chain, snapshot.as_json()).unwrap();
        {
            let mut topology = restored.commit_topology.block();
            topology.pop().expect("the fixture committee");
            topology.commit();
        }
        let tip = restored.view().native_execution_tip().unwrap();
        assert!(matches!(
            restored.authenticate_restored_keyed_state(),
            Err(WorldStateVerificationError::RestoredKeyedMismatch { height: 2, .. })
        ));
        // A refused claim stays a claim: no version, no execution.
        assert!(restored.keyed_versions.lock().published_at(tip).is_none());
        assert!(restored.try_block(header_after(&chain, 2)).is_err());

        // A policy cell the node installs from its configuration.
        let mut restored = decode(&chain, snapshot.as_json()).unwrap();
        let mut governance = restored.gov.clone();
        governance.citizenship_bond_amount = iroha_primitives::numeric::Numeric::from(7_u32)
            .try_into()
            .expect("fixture bond quantity");
        restored.set_gov(governance);
        assert!(matches!(
            restored.authenticate_restored_keyed_state(),
            Err(WorldStateVerificationError::RestoredKeyedMismatch { height: 2, .. })
        ));

        // A World row.
        let restored = decode(&chain, snapshot.as_json()).unwrap();
        {
            let mut world = restored.world.block();
            let (id, value) = iroha_data_model::IntoKeyValue::into_key_value(
                iroha_data_model::account::Account::new(iroha_test_samples::ALICE_ID.clone())
                    .build(&iroha_test_samples::ALICE_ID),
            );
            world.accounts.insert(id, value);
            world.commit();
        }
        assert!(matches!(
            restored.authenticate_restored_keyed_state(),
            Err(WorldStateVerificationError::RestoredKeyedMismatch { height: 2, .. })
        ));
    }

    #[test]
    fn keyed_restore_requires_the_exact_stored_tip() {
        let (chain, snapshot) = keyed_chain_with_snapshot();
        let restored = decode(&chain, snapshot.as_json()).unwrap();
        let claim = restored.keyed_versions.lock().restored_claim().unwrap();
        // The claim names the certified root of another height.
        let foreign = chain.committed(3).commitment().execution.world_state_root;
        assert_ne!(foreign, claim.certified);
        restored
            .keyed_versions
            .lock()
            .claim_restored(RestoredKeyedClaim {
                tip: claim.tip,
                certified: foreign,
            });
        assert!(matches!(
            restored.authenticate_restored_keyed_state(),
            Err(WorldStateVerificationError::RestoredKeyedMismatch { height: 2, certified, .. })
                if certified == foreign
        ));
        // An LtHash State carries no claim and is left untouched.
        let lthash = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
        assert!(!lthash.state().authenticate_restored_keyed_state().unwrap());
    }
}
