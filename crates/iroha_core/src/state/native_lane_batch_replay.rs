//! Re-execute native carrier input on its exact applying pre-State.
//!
//! Live proposal replay requires finalized first admissions and native Decisions,
//! not finality of the proposed applying carrier. Historical inclusion additionally
//! requires its private Kura seal. A post-closure State cannot replace the prefix.
//! Scratch replay grants no carrier validity. Source-owned candidate preparation
//! joins common global checks to the actual Network/Pipeline/Time output owner.
//! ValidBlock owns canonical pristine controls and full witness capture; the
//! retained output owner requires verified finality before publication.
//! Neither scratch replay nor candidate preparation grants voting or lane Apply.
//! TODO: complete remaining control/resource owners and connect the consuming
//! publisher to the sole production validation/Apply path, retiring the old
//! economic signers before opening native carrier admission.

use super::{
    AuthenticatedLaneAdmittedInputSourceV1, LaneDecisionGroupPreparationV1, MergeLedgerCommitError,
    State, VerifiedFirstLaneAdmittedInputV1, VerifiedLaneContexts, VerifiedLaneDecisionGroupV1,
    lane_decision_batch::{PreparedLaneDecisionBatchV1, RecordedNativeLaneBatchV1},
};
use crate::kura::FinalizedNativeLaneBatchV1;
use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, SignedBlock, lane_decision_batch::LaneDecisionBatchV1},
};

/// Both entry points retain the exact first-source recovery dependency.
/// The caller keeps its carrier and every completed private input until replay ends.
pub(crate) enum NativeLaneBatchReplayV1<'state> {
    /// Actual disposable execution; neither State nor Kura has been published.
    Ready(PreparedLaneDecisionBatchV1<'state>),
    /// Recover this exact first carrier while retaining the original carrier/batch.
    FirstInputRecoveryRequired {
        /// Original canonical execution position, never a new admission rank.
        execution_index: usize,
        /// Existing authenticated global body request authority.
        source: AuthenticatedLaneAdmittedInputSourceV1,
    },
    /// Refresh the live observation or reconstruct the historical applying prefix.
    ObservationChanged,
}

/// Prepared source authority, independent from an executed overlay.
/// Construction is private to the current/finalized carrier wrappers below.
/// The exact State reference prevents transferring this authority to another
/// same-height State, while generation/current-set checks prevent stale use.
pub(crate) struct PreparedNativeLaneBatchSourceV1<'state> {
    state: &'state State,
    observed: VerifiedLaneContexts,
    generation: u64,
    input: SignedBlock,
    groups: Vec<VerifiedLaneDecisionGroupV1>,
}

/// Source preparation retains exact indexed recovery custody without an overlay.
/// Callers retain the carrier and completed private first-input tokens on a wait.
pub(crate) enum NativeLaneBatchSourcePreparationV1<'state> {
    /// All first sources and exact native route Decisions are authenticated.
    Ready(PreparedNativeLaneBatchSourceV1<'state>),
    /// Existing global certified-body recovery owns this exact original position.
    FirstInputRecoveryRequired {
        execution_index: usize,
        source: AuthenticatedLaneAdmittedInputSourceV1,
    },
    /// Refresh the complete observation; no physical/economic owner was consumed.
    ObservationChanged,
    /// Finalized State has passed this proposed carrier before execution began.
    /// The proposal lost authority without becoming a deterministic rejection.
    Superseded,
}
impl<'state> NativeLaneBatchSourcePreparationV1<'state> {
    fn replay_scratch(self) -> Result<NativeLaneBatchReplayV1<'state>, MergeLedgerCommitError> {
        match self {
            Self::Ready(source) => source.replay_scratch(),
            Self::FirstInputRecoveryRequired {
                execution_index,
                source,
            } => Ok(NativeLaneBatchReplayV1::FirstInputRecoveryRequired {
                execution_index,
                source,
            }),
            Self::ObservationChanged => Ok(NativeLaneBatchReplayV1::ObservationChanged),
            Self::Superseded => Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native replay carrier was superseded by finalized State".into(),
            )),
        }
    }
}

impl<'state> PreparedNativeLaneBatchSourceV1<'state> {
    /// Validate the original global proposal and retain its actual execution
    /// through the common deterministic metadata tail. This unpublished owner
    /// grants neither production admission nor voting or Apply authority.
    pub(crate) fn prepare_candidate(
        self,
        context: crate::sumeragi::v2::VerifiedHeightContext,
        genesis_account: &iroha_data_model::account::AccountId,
        time_source: &iroha_primitives::time::TimeSource,
        block_cadence: std::time::Duration,
    ) -> Result<
        Option<super::PreparedCarrier<'state>>,
        crate::block::valid::NativeCandidatePreparationError,
    > {
        crate::block::ValidBlock::prepare_native_candidate(
            self,
            context,
            genesis_account,
            time_source,
            block_cadence,
        )
    }

    /// Borrow a current source and retain its original generation for preflight.
    /// A later State publication must invalidate the complete observation even
    /// when it starts between this check and the first global validation read.
    pub(crate) fn preparation_input(&self) -> Option<(&'state State, &SignedBlock, u64)> {
        self.is_current()
            .then_some((self.state, &self.input, self.generation))
    }

    /// Consume these original first-carrier/Decision owners through actual
    /// output sealing and witness capture. A changed observation returns no
    /// execution; the caller must refresh its source authority. The returned
    /// owner remains unpublished and does not authenticate the global proposal.
    pub(crate) fn record_execution(
        self,
        context: crate::sumeragi::v2::VerifiedHeightContext,
    ) -> Result<Option<RecordedNativeLaneBatchV1<'state>>, MergeLedgerCommitError> {
        // A recorder-owning caller must not wait for a State writer which may
        // itself be waiting for that recorder. This check acquires no locks.
        crate::sumeragi::witness::ensure_exec_witness_capture_available()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        if !self.is_current() {
            return Ok(None);
        }
        // Source authentication and any global preflight inspect this exact frozen
        // input. Move it into recording; there is no replacement carrier argument.
        let recorded =
            self.state
                .record_native_lane_decision_batch(self.input, self.groups, context);
        if !super::is_stable_state_view_generation(
            self.generation,
            self.state.state_view_generation(),
        ) {
            return Ok(None);
        }
        recorded.map(Some)
    }

    /// Inspect original source allocations in custody qualification only.
    #[cfg(test)]
    pub(super) fn groups_for_test(&self) -> &[VerifiedLaneDecisionGroupV1] {
        &self.groups
    }

    fn is_current(&self) -> bool {
        self.observed.is_current(self.state)
            && super::is_stable_state_view_generation(
                self.generation,
                self.state.state_view_generation(),
            )
    }

    fn replay_scratch(self) -> Result<NativeLaneBatchReplayV1<'state>, MergeLedgerCommitError> {
        self.stage_with_start_hooks()
    }

    /// Consume exact pre-State source authority through the SAME ordered
    /// constructor as scratch: shared start hooks, native metadata and common
    /// Network/Pipeline/Time output ownership, retaining the original verified groups.
    /// A capacity refusal preserves its typed fitting prefix for proposal selection;
    /// it cannot be flattened into a terminal input error.
    /// Scratch execution remains disposable: its native seal alone never grants
    /// the complete captured witness or exact durable finality needed to commit.
    pub(crate) fn stage_with_start_hooks(
        self,
    ) -> Result<NativeLaneBatchReplayV1<'state>, MergeLedgerCommitError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        let batch = crate::block::native_lane_batch_for_scratch(&self.input)
            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
        if !self.is_current() {
            return Ok(NativeLaneBatchReplayV1::ObservationChanged);
        }
        // Hash before the constructor takes its MV writer; never reread State
        // or perform Kura I/O while the returned overlay owns its guards.
        let actual_base = match self.state.lane_execution_state_hash() {
            Ok(hash) => hash,
            Err(error) if error.is_observation_changed() => {
                return Ok(NativeLaneBatchReplayV1::ObservationChanged);
            }
            Err(error) => return Err(error.into()),
        };
        if !self.is_current() {
            return Ok(NativeLaneBatchReplayV1::ObservationChanged);
        }
        if actual_base != batch.base_state_hash {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "native prepared source no longer matches its exact applying base".into(),
            ));
        }
        let prepared =
            self.state
                .replay_lane_decision_batch(&self.input.header(), batch, self.groups);
        if !super::is_stable_state_view_generation(
            self.generation,
            self.state.state_view_generation(),
        ) {
            return Ok(NativeLaneBatchReplayV1::ObservationChanged);
        }
        Ok(NativeLaneBatchReplayV1::Ready(prepared?))
    }
}

impl State {
    /// Observe whether finalized State has already passed a proposed Native
    /// carrier's height. `None` means a concurrent publication must be retried.
    pub(crate) fn native_proposal_superseded(
        &self,
        proposal_height: u64,
    ) -> Result<Option<bool>, String> {
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Ok(None);
        }
        let Some(view) = self.try_view_once().map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        let finalized_height = view.block_hashes.len() as u64;
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Ok(None);
        }
        Ok(Some(finalized_height >= proposal_height))
    }

    /// Recover one globally finalized native source and, once its carrier is
    /// applied, rejoin the exact retained World registry and replay membership.
    /// Cold recovery authenticates inclusion without publishing future effects.
    /// No State view or execution writer remains held during Kura I/O.
    pub(crate) fn read_finalized_native_lane_batch(
        &self,
        height: std::num::NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<crate::kura::NativeLaneBatchCarrierReadV1, MergeLedgerCommitError> {
        super::lane_decision_batch::with_stable_observation(self, || {
            self.read_finalized_native_lane_batch_at_observation(height, expected_hash)
        })
    }

    /// Validate one finite captured observation; the outer fence also covers
    /// failures and changes after the complete local view has been acquired.
    fn read_finalized_native_lane_batch_at_observation(
        &self,
        height: std::num::NonZeroUsize,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<crate::kura::NativeLaneBatchCarrierReadV1, MergeLedgerCommitError> {
        use super::{
            QueuePlanAdmissionRegistryMatch, TransactionsReadOnly as _, WorldReadOnly as _,
        };
        use mv::storage::StorageReadOnly as _;
        use std::str::FromStr as _;

        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Err(MergeLedgerCommitError::ExecutionObservationChanged);
        }
        let read = self
            .kura
            .read_finalized_native_lane_batch(height, expected_hash)
            .map_err(invalid)?;
        let Some(view) = self
            .try_view_once()
            .map_err(|error| invalid(error.to_string()))?
        else {
            return Err(MergeLedgerCommitError::ExecutionObservationChanged);
        };
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err(MergeLedgerCommitError::ExecutionObservationChanged);
        }
        let finality = match &read {
            crate::kura::NativeLaneBatchCarrierReadV1::Ready(included) => included.finality(),
            crate::kura::NativeLaneBatchCarrierReadV1::CanonicalBodyRecoveryRequired(source) => {
                source.finality()
            }
        };
        if finality.height_context.network_id != self.network_id {
            return Err(invalid("native history belongs to another network".into()));
        }
        if height.get() > view.block_hashes.len() {
            return Ok(read);
        }
        if view.block_hashes.get(height.get() - 1).copied() != Some(expected_hash) {
            return Err(invalid(
                "native history differs from the applied carrier".into(),
            ));
        }
        let crate::kura::NativeLaneBatchCarrierReadV1::Ready(included) = &read else {
            return Ok(read);
        };
        let batch = included.batch();
        let identity = super::lane_decision_batch::native_application_identity(
            included.carrier_header(),
            batch.canonical_hash().map_err(invalid)?,
        );
        let key = iroha_model_base::state_path::StatePath::from_str(&format!(
            "native_lane_application_{}",
            hex::encode(identity.as_ref()),
        ))
        .map_err(|error| invalid(error.to_string()))?;
        let encoded =
            norito::encode_canonical(&identity).map_err(|error| invalid(error.to_string()))?;
        if view.world.smart_contract_state().get(&key) != Some(&encoded) {
            return Err(invalid(
                "applied native history has no exact application marker".into(),
            ));
        }
        for group in &batch.groups {
            let input = &group.payload.input;
            if view.transactions.get(&input.entrypoint.hash()) != Some(height) {
                return Err(invalid(
                    "applied native history has no exact carrier membership".into(),
                ));
            }
            if Self::queue_plan_admission_registry_match_in_view(
                &view,
                input.entrypoint.hash(),
                input.certificate.binding.canonical_hash(),
            )
            .map_err(invalid)?
                != QueuePlanAdmissionRegistryMatch::Exact
            {
                return Err(invalid(
                    "applied native source lacks its exact retained admission registry binding"
                        .into(),
                ));
            }
        }
        Ok(read)
    }

    /// Authenticate all sources then recompute an included finalized input.
    ///
    /// `self` must be the applying carrier's exact pre-State (normally isolated
    /// replay State). An already-applied/closed State is rejected before context
    /// lookup. Retained recovered inputs have private canonical first-source
    /// tokens and are rejoined through the unchanged all-route group boundary.
    /// Retain all completed inputs across a second missing source; partial recovery
    /// must not restart the first request forever. Positions are strict and unique.
    pub(crate) fn replay_finalized_native_lane_batch(
        &self,
        included: &FinalizedNativeLaneBatchV1,
        recovered: &[(usize, VerifiedFirstLaneAdmittedInputV1)],
    ) -> Result<NativeLaneBatchReplayV1<'_>, MergeLedgerCommitError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        self.prepare_finalized_native_lane_batch_source(included, recovered)
            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?
            .replay_scratch()
    }

    /// Authenticate/re-execute one unfinalized proposed native economic carrier.
    ///
    /// The enclosing global proposal context, signatures, time/policy/DA bounds,
    /// full execution witness and commit gate remain ValidBlock responsibilities.
    /// SignedBlock itself has no network field: every source is joined to this
    /// State's exact network through finalized first-carrier and current-set proof.
    /// The returned overlay is private/disposable, not acceptance or publication.
    pub(crate) fn replay_proposed_native_lane_batch(
        &self,
        carrier: &SignedBlock,
        recovered: &[(usize, VerifiedFirstLaneAdmittedInputV1)],
    ) -> Result<NativeLaneBatchReplayV1<'_>, MergeLedgerCommitError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()
            .map_err(MergeLedgerCommitError::ExecutionRecorderConflict)?;
        self.prepare_proposed_native_lane_batch_source(carrier, recovered)
            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?
            .replay_scratch()
    }

    /// Prepare exact finalized source authority without holding an execution overlay.
    pub(crate) fn prepare_finalized_native_lane_batch_source(
        &self,
        included: &FinalizedNativeLaneBatchV1,
        recovered: &[(usize, VerifiedFirstLaneAdmittedInputV1)],
    ) -> Result<NativeLaneBatchSourcePreparationV1<'_>, String> {
        self.prepare_native_lane_batch_from_pre_state(
            included.carrier(),
            included.batch(),
            included.finality().height_context.network_id,
            recovered,
        )
    }

    /// Prepare an unfinalized applying carrier from finalized admissions/native Decisions.
    /// Raw carrier bytes still never construct a verified roster or group directly.
    pub(crate) fn prepare_proposed_native_lane_batch_source(
        &self,
        carrier: &SignedBlock,
        recovered: &[(usize, VerifiedFirstLaneAdmittedInputV1)],
    ) -> Result<NativeLaneBatchSourcePreparationV1<'_>, String> {
        if !carrier.is_resultless_proposal() {
            return Err("live native replay requires an exact resultless proposal".into());
        }
        let batch = crate::block::native_lane_batch_for_execution(carrier)?;
        self.prepare_native_lane_batch_from_pre_state(carrier, batch, self.network_id, recovered)
    }

    /// Authenticate native sources for the sole global execution owner. Carrier
    /// controls are shape-checked here and authenticated separately by ValidBlock.
    pub(crate) fn prepare_canonical_native_lane_batch_source(
        &self,
        carrier: &SignedBlock,
    ) -> Result<NativeLaneBatchSourcePreparationV1<'_>, String> {
        let batch = crate::block::native_lane_batch_for_execution(carrier)?;
        self.prepare_native_lane_batch_from_pre_state(carrier, batch, self.network_id, &[])
    }

    /// Sole source authentication kernel. Raw arguments to this
    /// private method grant no authority: the wrapper establishes its carrier
    /// shape/inclusion, and each source still traverses the private input/group join.
    fn prepare_native_lane_batch_from_pre_state(
        &self,
        carrier: &SignedBlock,
        batch: &LaneDecisionBatchV1,
        expected_network: NetworkId,
        recovered: &[(usize, VerifiedFirstLaneAdmittedInputV1)],
    ) -> Result<NativeLaneBatchSourcePreparationV1<'_>, String> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness()?;
        let header = carrier.header();
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
        }
        let (height, hash, network, expected_policy_hash) = {
            let Some(view) = self.try_view_once().map_err(|error| error.to_string())? else {
                return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
            };
            (
                view.block_hashes.len() as u64,
                view.block_hashes.last().copied(),
                view.network_id,
                HashOf::new(&crate::da::active_proof_policy_bundle_at_height(
                    &view.nexus,
                    header.height().get(),
                )),
            )
        };
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
        }
        if network == expected_network && height >= header.height().get() {
            return Ok(NativeLaneBatchSourcePreparationV1::Superseded);
        }
        // Both wrappers bind the source bytes to this actual carrier header.
        // The active DA policy is authenticated from the exact applying pre-State.
        if header.da_proof_policies_hash() != Some(expected_policy_hash) {
            return Err(
                "native carrier DA proof-policy snapshot differs from active pre-State policy"
                    .into(),
            );
        }
        let base_hash = match self.lane_execution_state_hash() {
            Ok(hash) => hash,
            Err(error) if error.is_observation_changed() => {
                return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
            }
            Err(error) => return Err(error.to_string()),
        };
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
        }
        if network != expected_network
            || height != batch.base_state_height
            || height.checked_add(1) != Some(header.height().get())
            || hash != header.prev_block_hash()
            || base_hash != batch.base_state_hash
            || batch.groups.iter().any(|execution| {
                execution
                    .payload
                    .input
                    .certificate
                    .binding
                    .network_id_digest
                    != crate::torii_proxy::queue_plan_admission_network_id_digest(&network)
            })
        {
            return Err(
                "native replay requires its exact applying-carrier pre-State and network".into(),
            );
        }
        if recovered.len() > batch.groups.len()
            || recovered
                .iter()
                .any(|(index, _)| *index >= batch.groups.len())
            || recovered.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err("recovered native inputs repeat or lack exact execution positions".into());
        }
        // This reader drops all State/MV guards before authenticating Kura proof.
        let observation = self.verified_lane_consensus_contexts();
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
        }
        let observed = observation?
            .ok_or_else(|| "native replay pre-State has no published context proof".to_owned())?;
        let mut groups = Vec::with_capacity(batch.groups.len());
        for (index, execution) in batch.groups.iter().enumerate() {
            let result = match recovered.iter().find(|(position, _)| *position == index) {
                Some((_, input)) => {
                    self.import_recovered_lane_decision_group(&observed, execution, input)
                }
                _ => self.import_lane_decision_group(&observed, execution),
            };
            if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
                return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
            }
            match result? {
                LaneDecisionGroupPreparationV1::Ready(group) => groups.push(group),
                LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(source) => {
                    return Ok(
                        NativeLaneBatchSourcePreparationV1::FirstInputRecoveryRequired {
                            execution_index: index,
                            source,
                        },
                    );
                }
                LaneDecisionGroupPreparationV1::ObservationChanged => {
                    return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
                }
                // A proposed or included batch cannot wait on a lane which did
                // not own this exact pre-State head. The source is invalid for
                // this prefix; no local timeout or cancellation is inferred.
                _ => {
                    return Err(
                        "native batch contradicts its authenticated pre-State route heads".into(),
                    );
                }
            }
        }
        if !observed.is_current(self)
            || !super::is_stable_state_view_generation(generation, self.state_view_generation())
        {
            return Ok(NativeLaneBatchSourcePreparationV1::ObservationChanged);
        }
        Ok(NativeLaneBatchSourcePreparationV1::Ready(
            PreparedNativeLaneBatchSourceV1 {
                state: self,
                observed,
                generation,
                input: carrier.clone(),
                groups,
            },
        ))
    }
}
