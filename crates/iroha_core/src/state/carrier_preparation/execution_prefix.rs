//! Actual validated execution custody through the deterministic carrier tail.
//!
//! Only the original validator handoff enters this consuming seam. Native
//! preparation retains its actual source and verified context; the production
//! gate remains closed. Raw State keeps a closed transfer marker.

use super::{super::*, PreparedCarrier};
use crate::{
    block::{ValidBlock, valid::ValidatedCarrierPreparationInput},
    kura::KuraPublicationLease,
    sumeragi::exec,
};
use iroha_data_model::block::consensus_v2::{ExecutionCommitment, HeightContext};

/// A local durable-source refusal, never a deterministic proposal rejection.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CarrierSourceAuthenticationError {
    /// Original execution, retained source, or exact durable carrier differ.
    #[error("retained carrier source differs from durable evidence: {0}")]
    Identity(String),
    /// Authenticated local absence requires recovery of this exact executed body.
    #[error(
        "result-bearing carrier body at height {height} ({block_hash}) is unavailable; exact body recovery is required"
    )]
    CarrierBodyUnavailable {
        /// Exact decided carrier height whose complete result image is required.
        height: u64,
        /// Original decided block identity; a different body cannot satisfy it.
        block_hash: HashOf<BlockHeader>,
    },
    /// Required evidence is missing or failed its exact guarded storage read.
    #[error("retained carrier source requires storage recovery: {0}")]
    Storage(#[from] crate::kura::Error),
}

/// Complete actual prefix owners, moved once after their exact attachment checks.
/// This is execution custody only; decision, durability and publication remain
/// independent owners. There is no public constructor or mutable source access.
pub(crate) struct ValidatedExecutionPrefix {
    sealed: output_capacity::SealedExecutionOutputs,
    authority: PrefixSourceAuthority,
    inventory: Arc<FastpqSourceInventoryV1>,
    witness: ExecWitness,
    fastpq_witness_context: Option<crate::fastpq::FastpqWitnessContext>,
    parliament_timed_ovn_casting_bindings: Option<
        Vec<iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1>,
    >,
}

/// Exhaustive source custody: Native preparation cannot use an ordinary/empty
/// merge projection as its authority. All original Native owners survive capture.
enum PrefixSourceAuthority {
    Ordinary,
    Native(Box<lane_decision_batch::NativeExecutionCustody>),
}

impl ValidatedExecutionPrefix {
    /// Authenticate original execution and immutable sources under the held Kura
    /// boundary. The caller retains this prefix and lease together; no capability
    /// escapes, no World delta is resealed and no source is released. Installation
    /// admission must cover the guarded reads, encoding and witness verification.
    pub(in crate::state) fn authenticate_durable_carrier(
        &self,
        block: &SignedBlock,
        context: &HeightContext,
        commitment: &ExecutionCommitment,
        lease: &KuraPublicationLease<'_>,
    ) -> Result<(), CarrierSourceAuthenticationError> {
        use CarrierSourceAuthenticationError::Identity;

        self.sealed.verify_wire_binding(block).map_err(Identity)?;
        let source_context = self.sources().source_context();
        if source_context.network_id != context.network_id
            || source_context.height != context.height
            || context.height != block.header().height().get()
            || self.sources().proposal() != block.hash()
            || self.sources().is_native()
                != matches!(&self.authority, PrefixSourceAuthority::Native(_))
        {
            return Err(Identity(
                "execution source lost its retained context".into(),
            ));
        }
        let manifest =
            exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
                block, None,
            )
            .map_err(Identity)?;
        let lanes =
            exec::LaneFinalityManifestV1::from_result_bearing_block(block).map_err(Identity)?;
        let actual = exec::execution_commitment_from_validated_block(
            &self.witness,
            &manifest,
            &lanes,
            block,
        )
        .map_err(|error| Identity(error.into()))?;
        if actual != *commitment {
            return Err(Identity(
                "execution commitment differs from its original witness".into(),
            ));
        }
        let height = usize::try_from(context.height)
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .ok_or_else(|| Identity("carrier height is not representable".into()))?;
        let durable = lease.read_first_admission_carrier(height, block.hash())?;
        if durable.finality.commit_qc.round.context_id != context.id()
            || durable.finality.commit_qc.execution_commitment != *commitment
        {
            return Err(Identity(
                "durable finality differs from the retained execution or context".into(),
            ));
        }
        let durable_body =
            durable
                .body
                .ok_or(CarrierSourceAuthenticationError::CarrierBodyUnavailable {
                    height: context.height,
                    block_hash: block.hash(),
                })?;
        self.sealed
            .verify_wire_binding(&durable_body)
            .map_err(Identity)?;
        match &self.authority {
            PrefixSourceAuthority::Ordinary => {
                if block.execution_context().is_some_and(|bundle| {
                    bundle.native_lane_decisions.is_some() || bundle.merge_entry.is_some()
                }) {
                    return Err(Identity(
                        "ordinary execution cannot substitute another source family".into(),
                    ));
                }
            }
            PrefixSourceAuthority::Native(native) => {
                if !native.retains_carrier(block, context) {
                    return Err(Identity(
                        "Native execution lost its original stage or complete sources".into(),
                    ));
                }
                for group in native.sources() {
                    let original = group.body().source();
                    let rank = original.priority();
                    let height = usize::try_from(rank.carrier_height)
                        .ok()
                        .and_then(std::num::NonZeroUsize::new)
                        .ok_or_else(|| {
                            Identity("Native first-carrier height is not representable".into())
                        })?;
                    if rank.carrier_height >= context.height {
                        return Err(Identity(
                            "Native first admission is not an applying predecessor".into(),
                        ));
                    }
                    let read =
                        lease.read_first_admission_carrier(height, original.carrier_hash())?;
                    if &read.finality != original.source().finality()
                        || read.finality.height_context.network_id != context.network_id
                    {
                        return Err(Identity(
                            "Native first-carrier finality differs from its original source".into(),
                        ));
                    }
                    if let Some(body) = read.body {
                        let index = usize::try_from(rank.admission_index).map_err(|_| {
                            Identity("Native first-admission index is not representable".into())
                        })?;
                        let control = body
                            .execution_context()
                            .and_then(|bundle| bundle.queue_plan_admissions.get(index));
                        if control.map(Vec::as_slice) != Some(original.canonical_control_bytes()) {
                            return Err(Identity(
                                "Native first-admission bytes differ from their retained position"
                                    .into(),
                            ));
                        }
                    }
                    // Authenticated local absence (eviction or imported prefix)
                    // may use these same privately verified source bytes. Missing
                    // or corrupt finality/occupied body already failed above.
                }
            }
        }
        Ok(())
    }

    /// Match the exact immutable source owner at the sole publication boundary.
    pub(in crate::state::carrier_preparation) fn retains_carrier(
        &self,
        block: &iroha_data_model::block::SignedBlock,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
    ) -> bool {
        self.sealed.proposal() == block.hash()
            && self.sealed.sources().proposal() == block.hash()
            && match &self.authority {
                PrefixSourceAuthority::Ordinary => {
                    !self.sealed.sources().is_native()
                        && !block
                            .execution_context()
                            .is_some_and(|bundle| bundle.native_lane_decisions.is_some())
                }
                PrefixSourceAuthority::Native(native) => {
                    self.sealed.sources().is_native() && native.retains_carrier(block, context)
                }
            }
    }
    /// Borrow the exact source custody without granting publication authority.
    pub(in crate::state) fn native(&self) -> Option<&lane_decision_batch::NativeExecutionCustody> {
        match &self.authority {
            PrefixSourceAuthority::Ordinary => None,
            PrefixSourceAuthority::Native(native) => Some(native),
        }
    }

    #[cfg(test)]
    pub(in crate::state) fn native_for_test(
        &self,
    ) -> Option<&lane_decision_batch::NativeExecutionCustody> {
        match &self.authority {
            PrefixSourceAuthority::Ordinary => None,
            PrefixSourceAuthority::Native(native) => Some(native),
        }
    }

    /// Actual finalized inventory, including rejected and zero-transcript calls.
    pub(in crate::state) fn inventory(&self) -> &Arc<FastpqSourceInventoryV1> {
        &self.inventory
    }

    /// The original witness which produced the retained execution commitment.
    pub(in crate::state) fn witness(&self) -> &ExecWitness {
        &self.witness
    }

    /// Actual invocation owners retained through sealing, never regenerated rows.
    pub(in crate::state) fn sources(&self) -> &output_capacity::OwnedExecutionSources {
        self.sealed.sources()
    }

    /// Original optional proving context, retained even when no work is scheduled.
    pub(in crate::state) fn fastpq_witness_context(
        &self,
    ) -> Option<&crate::fastpq::FastpqWitnessContext> {
        self.fastpq_witness_context.as_ref()
    }

    /// Actual timed-casting bindings captured by execution.
    pub(in crate::state) fn parliament_timed_ovn_casting_bindings(
        &self,
    ) -> Option<&[iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1]>
    {
        self.parliament_timed_ovn_casting_bindings.as_deref()
    }

    // Structural custody check only. The closed marker grants no authority and
    // cannot substitute the actual prefix held by the private consuming owner.
    fn retains_closed_state(&self, state: &StateBlock<'_>) -> bool {
        self.sealed.proposal() == state._curr_block.hash()
            && matches!(
                state.execution_output_plan,
                Some(output_capacity::ExecutionOutputPlanState::Captured)
            )
            && state.exec_witness.is_none()
            && state.fastpq_source_inventory.is_none()
            && state.fastpq_witness_context.is_none()
            && state.parliament_timed_ovn_casting_bindings.is_none()
            && match &self.authority {
                PrefixSourceAuthority::Ordinary => state.native_lane_stage.is_none(),
                PrefixSourceAuthority::Native(native) => native.retains_state(state),
            }
    }
}

/// The only mutable metadata-tail scope. Neither field escapes independently
/// until the complete tail succeeds, so a different prefix/State pair cannot be
/// supplied to World preparation, including one with an equal proposal header.
struct PrefixPreparation<'state> {
    state: Box<StateBlock<'state>>,
    prefix: ValidatedExecutionPrefix,
}

impl<'state> PrefixPreparation<'state> {
    fn capture(
        mut state: Box<StateBlock<'state>>,
        valid: &ValidBlock,
        native: Option<lane_decision_batch::NativeExecutionCustody>,
    ) -> Result<
        (
            Self,
            exec::NativeAmxApplicationManifestV1,
            iroha_data_model::block::consensus_v2::ExecutionCommitment,
        ),
        String,
    > {
        let block = valid.as_ref();
        // Certified merge retains its separate, unfinished source consumer.
        // A Native source must arrive from the exact globally checked recorded
        // constructor, retaining the actual stage, groups and verified context.
        let authority = match native {
            Some(native)
                if native.retains_state(&state)
                    && block
                        .execution_context()
                        .is_some_and(|context| context.native_lane_decisions.is_some()) =>
            {
                PrefixSourceAuthority::Native(Box::new(native))
            }
            None if state.native_lane_stage.is_none()
                && !block
                    .execution_context()
                    .is_some_and(|context| context.native_lane_decisions.is_some())
                && state.merge_carrier_entrypoints.is_empty() =>
            {
                PrefixSourceAuthority::Ordinary
            }
            _ => return Err("carrier prefix has no exact Native or ordinary source owner".into()),
        };
        if state.staged_merge_entry.is_some()
            || state.canonical_wsv_merge_commit_authorization.is_some()
            || state
                .canonical_carrier_commit_metadata_authorization
                .is_some()
        {
            return Err("carrier prefix has no active certified-merge source owner".into());
        }
        // These checks must precede the permitted metadata/World tail: that tail
        // intentionally changes the World delta authenticated by the attachment.
        state.verify_execution_output_seal(block)?;
        let verified_inventory = state.verified_fastpq_source_inventory_for_capture()?;
        state.verify_cached_ordinary_witness_content(&verified_inventory)?;
        let witness = state
            .exec_witness
            .as_ref()
            .ok_or("carrier preparation requires its retained execution witness")?;
        let manifest =
            exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
                block, None,
            )?;
        let lanes = exec::LaneFinalityManifestV1::from_result_bearing_block(block)?;
        let commitment =
            exec::execution_commitment_from_validated_block(witness, &manifest, &lanes, block)
                .map_err(str::to_owned)?;
        let Some(output_capacity::ExecutionOutputPlanState::Sealed(sealed)) = state
            .execution_output_plan
            .replace(output_capacity::ExecutionOutputPlanState::Captured)
        else {
            return Err("carrier prefix lost its sealed output owner".into());
        };
        if sealed.sources().is_native() != matches!(&authority, PrefixSourceAuthority::Native(_))
            || sealed.sources().proposal() != block.hash()
        {
            return Err("carrier prefix differs from its actual source owner".into());
        }
        let inventory = state
            .fastpq_source_inventory
            .take()
            .ok_or("carrier prefix lost its owned source inventory")??;
        if !Arc::ptr_eq(&inventory, &verified_inventory) {
            return Err("carrier prefix source inventory changed during capture".into());
        }
        let witness = state
            .exec_witness
            .take()
            .ok_or("carrier prefix lost its original execution witness")?;
        let prefix = ValidatedExecutionPrefix {
            sealed,
            authority,
            inventory,
            witness,
            fastpq_witness_context: state.fastpq_witness_context.take(),
            parliament_timed_ovn_casting_bindings: state
                .parliament_timed_ovn_casting_bindings
                .take(),
        };
        Ok((Self { state, prefix }, manifest, commitment))
    }

    fn prepare_world_effects(&mut self) -> Result<world_commit::PreparedWorldEffects, String> {
        let state = &mut *self.state;
        if !self.prefix.retains_closed_state(state)
            || state.block_hashes.pending.as_slice() != [state._curr_block.hash()]
        {
            return Err(
                "World carrier preparation lost its original prefix or exact staged metadata"
                    .into(),
            );
        }
        state.validate_canonical_runtime_projection()?;
        state.verify_lane_consensus_contexts_publication()?;
        state
            .validate_merge_carrier_entrypoint_binding()
            .map_err(|error| error.to_string())?;
        state
            .finalize_axt_asset_incarnations()
            .map_err(|error| error.to_string())?;
        state
            .finalize_axt_policy_transition_ratchets()
            .map_err(|error| error.to_string())?;
        state.prune_axt_replay_ledger(
            current_axt_slot_from_block(&state._curr_block, state.nexus.axt.slot_length_ms),
            state.nexus.axt.replay_retention_slots.get(),
        );
        let height = state._curr_block.height().get();
        state
            .validate_owned_runtime_catalog_overlay()
            .map_err(|error| error.to_string())?;
        if let Some(pending) = &state.pending_autoscale_lifecycle {
            let predecessor = state
                .canonical_runtime
                .get_before_block()
                .nexus_projection(&state.nexus)
                .map_err(|error| error.to_string())?;
            ensure_pending_autoscale_lifecycle_staking_is_safe(
                &state.world,
                &predecessor,
                pending,
                height,
            )
            .map_err(|error| error.to_string())?;
        }
        world_commit::PreparedWorldCommit::prepare_overlay(
            &mut state.world,
            height,
            &state.nexus,
            &state.lane_incarnation_activation_heights,
            state.pending_da_pin_intents.as_ref(),
            state.pending_autoscale_lifecycle.as_ref(),
        )
    }
}

pub(super) fn prepare<'state>(
    input: ValidatedCarrierPreparationInput<'state>,
) -> Result<PreparedCarrier<'state>, (Box<iroha_data_model::block::SignedBlock>, String)> {
    let (valid, state, context, native) = input.into_parts();
    let result = (|| {
        let (mut preparation, native_amx_manifest, execution_prefix) =
            PrefixPreparation::capture(state, &valid, native)?;
        // Both authenticated input constructors finish the common typed tail
        // before moving this owner. Metadata capture must not become another
        // fallible local autoscale evaluation hidden behind a String result.
        if !preparation.state.autoscale_lifecycle_evaluated {
            return Err("carrier prefix lost its completed autoscale evaluation".to_owned());
        }
        let block = valid.as_ref();
        preparation
            .state
            .prepare_deterministic_carrier_metadata(
                block,
                context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect(),
                ApplyTopologyAuthority::V2Finality,
            )
            .map_err(|error| error.to_string())?;
        let world_effects = preparation.prepare_world_effects()?;
        let publication_events = preparation
            .state
            .prepare_carrier_publication_events(block.header())
            .map_err(|error| error.to_string())?;
        Ok((
            preparation,
            native_amx_manifest,
            execution_prefix,
            world_effects,
            publication_events,
        ))
    })();
    match result {
        Ok((
            preparation,
            native_amx_manifest,
            execution_prefix,
            world_effects,
            publication_events,
        )) => {
            let PrefixPreparation {
                state,
                prefix: source_prefix,
            } = preparation;
            Ok(PreparedCarrier {
                valid,
                state,
                source_prefix,
                context,
                execution_prefix,
                native_amx_manifest,
                _world_effects: world_effects,
                _publication_events: publication_events,
            })
        }
        Err(error) => Err((Box::new(valid.into()), error)),
    }
}

#[cfg(test)]
#[path = "execution_prefix_tests.rs"]
mod tests;
