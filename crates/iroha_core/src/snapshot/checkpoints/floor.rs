//! Exact retained-checkpoint restore and certified replay, with original custody through signing.

use std::{num::NonZeroUsize, sync::Arc};

use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_config::parameters::actual::SnapshotResourcePolicy;
use iroha_crypto::{Hash, KeyPair, Signature};
use iroha_data_model::{
    block::SharedSignedBlock,
    runtime::checkpoint::{
        STARTUP_FLOOR_ATTESTATION_DOMAIN, STARTUP_FLOOR_DOMAIN, SignedStartupFloorAttestationV1,
        StartupFloorAttestationV1, StartupFloorV1,
    },
};

use super::SnapshotCheckpoints;
use crate::{
    execution_attempt::ExecutionAttemptError,
    kura::Kura,
    snapshot::{
        BoundSnapshotGeneration, SnapshotLocation, SnapshotRestorePurpose, TryReadError,
        bind_snapshot_location_with_mode, checkpoint_store, try_read_snapshot_bundle,
    },
    state::{State, StateReadOnly as _},
    sumeragi::{
        certified_chain::{CertifiedChain, ChainReadError},
        node::replay_check::{self, MatchedReplay, ReplayCheckError, ReplayCheckInputs},
    },
};

/// An original restore, storage, execution or signing refusal. No report grants authority.
#[derive(Debug, thiserror::Error)]
pub enum FloorRehearsalError {
    /// The requested checkpoint or source owner does not satisfy the contract.
    #[error("checkpoint floor rehearsal: {0}")]
    Invalid(&'static str),
    /// Original bounded snapshot admission or restore failure.
    #[error(transparent)]
    Snapshot(#[from] TryReadError),
    /// Original durable storage observation failure.
    #[error(transparent)]
    Storage(#[from] crate::kura::Error),
    /// Original funded native frame read refusal.
    #[error(transparent)]
    Read(#[from] ExecutionAttemptError<crate::kura::Error>),
    /// Ordinary restored-State preparation or certified suffix replay failed.
    #[error(transparent)]
    Replay(#[from] ReplayCheckError),
    /// Original certified history verification refusal.
    #[error(transparent)]
    Chain(#[from] ExecutionAttemptError<ChainReadError>),
    /// Original signed genesis execution-context decoding refusal.
    #[error(transparent)]
    Genesis(#[from] ExecutionAttemptError<iroha_data_model::sumeragi_finality::GenesisReadError>),
    /// The same replay State could not be observed.
    #[error(transparent)]
    View(#[from] crate::state::StateViewError),
    /// Original materialized restored policy and geometry preparation refusal.
    #[error(transparent)]
    Configuration(#[from] crate::state::RestoredReplayConfigurationError),
    /// Original canonical statement encoding refusal.
    #[error(transparent)]
    Codec(#[from] norito::core::DecodeAttemptError),
    /// The original finite execution pool refused the exact statement backing.
    #[error(transparent)]
    Buffer(#[from] ChargedBufferError),
    /// Original signing refusal, including unavailable local cryptography.
    #[error(transparent)]
    Crypto(#[from] iroha_crypto::Error),
}

/// A stopped source and its exact terminal original frames, retained until signing ends.
struct StoppedSource {
    kura: Arc<Kura>,
    journals: crate::kura::StoppedReplayJournals,
    budget: AllocationBudget,
    through: usize,
    genesis: SharedSignedBlock,
    tip: SharedSignedBlock,
}

fn read(
    kura: &Kura,
    budget: &AllocationBudget,
    height: usize,
) -> Result<SharedSignedBlock, FloorRehearsalError> {
    let height =
        NonZeroUsize::new(height).ok_or(FloorRehearsalError::Invalid("zero source height"))?;
    kura.get_block(height, budget)?
        .ok_or(FloorRehearsalError::Invalid(
            "original certified body is absent",
        ))
}

impl StoppedSource {
    fn capture(
        kura: Arc<Kura>,
        copy: &Arc<Kura>,
        budget: AllocationBudget,
    ) -> Result<Self, FloorRehearsalError> {
        if !kura.is_locked_read_only_source() {
            return Err(FloorRehearsalError::Invalid(
                "original source must own the stopped store's read-only root lock",
            ));
        }
        if !copy.is_temporary_staging()
            || copy.emergency_fast_startup_enabled()
            || Arc::ptr_eq(&kura, copy)
            || kura.store_root() == copy.store_root()
        {
            return Err(FloorRehearsalError::Invalid(
                "replay requires a distinct temporary Strict Kura",
            ));
        }
        let through = kura.exact_durable_blocks_count()?;
        let source = Self {
            journals: kura.retain_stopped_replay_journals()?,
            genesis: read(&kura, &budget, 1)?,
            tip: read(&kura, &budget, through)?,
            kura,
            budget,
            through,
        };
        source.revalidate(copy)?;
        Ok(source)
    }

    fn revalidate(&self, copy: &Kura) -> Result<(), FloorRehearsalError> {
        self.journals.revalidate()?;
        if self.kura.exact_durable_blocks_count()? != self.through
            || copy.exact_durable_blocks_count()? != self.through
            || read(&self.kura, &self.budget, 1)? != self.genesis
            || read(&self.kura, &self.budget, self.through)? != self.tip
            || read(copy, &self.budget, 1)? != self.genesis
            || read(copy, &self.budget, self.through)? != self.tip
        {
            return Err(FloorRehearsalError::Invalid(
                "original stopped source and replay copy no longer name the exact same tip",
            ));
        }
        Ok(())
    }
}

/// The exact requested checkpoint decoded from its retained original files.
///
/// Its State stays private; only ordinary H+1 authentication and replay can advance this owner.
/// This is neither a startup-floor statement nor permission to delete history.
pub struct RestoredFloorCheckpoint {
    generation: BoundSnapshotGeneration,
    source: StoppedSource,
    floor: StartupFloorV1,
    state: Arc<State>,
}

/// Actual successful H+1 authentication and complete suffix replay of one exact checkpoint.
///
/// No codec, public constructor, clone, mutable State accessor or report conversion exists.
/// Original stopped-store and checkpoint handles remain alive through the consuming signer.
pub struct ReplayedFloorCheckpoint {
    generation: BoundSnapshotGeneration,
    source: StoppedSource,
    floor: StartupFloorV1,
    replay: MatchedReplay,
}

impl SnapshotCheckpoints {
    /// Restore exactly `height` from the retained checkpoint store onto an isolated copy.
    ///
    /// `original_stopped` retains the original node store's exclusive read-only lock.
    /// `configuration_source` is a configured template on the distinct temporary Strict copy;
    /// only its installed settings, query handle and original pool are borrowed. The explicit
    /// SORA Nexus configuration and frozen manifest/compliance sources follow ordinary
    /// restored startup.
    /// No World rows are copied from the template, no newest-checkpoint fallback exists, and
    /// no block executes before the returned owner's ordinary complete-State authentication.
    ///
    /// # Errors
    /// Missing/ambiguous exact checkpoint, invalid custody or signature, missing H+1,
    /// mismatched source/copy, or the original restore/resource refusal.
    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter retains an independent original restoration or configuration owner"
    )]
    pub fn restore_exact_for_floor(
        &self,
        height: u64,
        original_stopped: Arc<Kura>,
        configuration_source: &State,
        configured_nexus: &iroha_config::parameters::actual::Nexus,
        lane_manifests: &crate::governance::manifest::LaneManifestRegistryHandle,
        compliance: Option<Arc<crate::compliance::LaneComplianceEngine>>,
        resource_policy: SnapshotResourcePolicy,
    ) -> Result<RestoredFloorCheckpoint, FloorRehearsalError> {
        let copy = configuration_source.kura_handle();
        let budget = configuration_source.ivm_execution_budget();
        let source = StoppedSource::capture(original_stopped, &copy, budget.clone())?;
        if height == 1 {
            // The ordinary native-tip decoder requires actual genesis replay for H1;
            // it cannot grant complete-State restore authority to a genesis snapshot.
            return Err(TryReadError::NativeExecutionReplayRequired.into());
        }
        if height == 0
            || u64::try_from(source.through)
                .ok()
                .is_none_or(|tip| height >= tip)
        {
            return Err(FloorRehearsalError::Invalid(
                "requested checkpoint requires its actual certified successor",
            ));
        }
        if iroha_data_model::NetworkId::from_genesis_hash(source.genesis.hash())
            != *configuration_source.network_id_ref()
        {
            return Err(FloorRehearsalError::Invalid(
                "configured network differs from the original signed genesis",
            ));
        }
        let entries = checkpoint_store::entries(&self.store_dir).map_err(|error| {
            TryReadError::IO(error, checkpoint_store::checkpoints_dir(&self.store_dir))
        })?;
        let mut selected = entries
            .iter()
            .filter(|(candidate, _, _)| *candidate == height);
        let (_, committed_ms, path) = selected.next().ok_or(FloorRehearsalError::Invalid(
            "requested height has no retained checkpoint",
        ))?;
        if selected.next().is_some() {
            return Err(FloorRehearsalError::Invalid(
                "requested height has ambiguous retained checkpoints",
            ));
        }
        let digest_path = path.join(super::super::SNAPSHOT_DIGEST_FILE_NAME);
        let (digest_file, bytes) = super::super::bind_required_snapshot_file(
            &digest_path,
            super::super::SNAPSHOT_DIGEST_MAX_BYTES,
        )?;
        let digest = super::super::parse_snapshot_current_pointer(&bytes, &digest_path)?;
        let generation = bind_snapshot_location_with_mode(
            &self.store_dir,
            self.max_payload_bytes.get() as u64,
            self.merkle_chunk_size,
            false,
            Some(&SnapshotLocation::Checkpoint {
                name: format!("{height}-{committed_ms}"),
                digest,
            }),
        )?;
        super::super::verify_bound_snapshot_file(&digest_file)?;
        let mut outcome = try_read_snapshot_bundle(
            &budget,
            &generation,
            SnapshotRestorePurpose::Startup,
            &copy,
            lane_manifests,
            configured_nexus,
            &configuration_source.query_handle,
            source.through,
            self.merkle_chunk_size,
            resource_policy,
            &self.verification_key,
            configuration_source.chain_id_ref(),
            configuration_source.network_id_ref(),
            &|state| {
                state
                    .install_snapshot_reconciliation_configuration(configuration_source)
                    .map_err(TryReadError::ReconciliationConfiguration)
            },
            #[cfg(feature = "telemetry")]
            crate::telemetry::StateTelemetry::default(),
            &self.read_buffer_budget,
        )?;
        outcome.state.prepare_restored_replay_configuration(
            configured_nexus.clone(),
            lane_manifests,
            compliance,
        )?;
        let state: Arc<State> = Arc::from(outcome.state);
        if state.committed_height() as u64 != height
            || !state.ivm_execution_budget().same_pool(&budget)
            || !Arc::ptr_eq(&state.kura_handle(), &copy)
        {
            return Err(FloorRehearsalError::Invalid(
                "restored checkpoint owner changed",
            ));
        }
        let hash = state
            .latest_block_hash_fast()
            .ok_or(FloorRehearsalError::Invalid(
                "restored checkpoint has no native block hash",
            ))?;
        let floor = StartupFloorV1 {
            network: *state.network_id_ref(),
            height,
            block_hash: *hash.as_ref(),
            // NativeExecutionTipSnapshot::restore reconstructs this time from the
            // authenticated native block or portable prefix header. It is not a
            // timestamp claim decoded from the signed snapshot or its directory.
            committed_ms: state
                .try_view_once()?
                .native_execution_tip()
                .ok_or(FloorRehearsalError::Invalid(
                    "checkpoint lacks authenticated native time",
                ))?
                .creation_time_ms(),
        };
        source.revalidate(&copy)?;
        Ok(RestoredFloorCheckpoint {
            generation,
            source,
            floor,
            state,
        })
    }
}

impl RestoredFloorCheckpoint {
    /// Authenticate the exact complete State against H+1 and replay every later certified body.
    ///
    /// This uses the same prepared executor and final cold commitment check as normal startup.
    /// Genesis authority and consensus mode come only from the retained signed genesis.
    /// Progress is an observation only; it cannot supply a result or authorize a signature.
    ///
    /// # Errors
    /// Original certificate/resource refusal, missing history, divergence, or changed custody.
    pub fn replay(
        self,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<ReplayedFloorCheckpoint, FloorRehearsalError> {
        let Self {
            generation,
            source,
            floor,
            state,
        } = self;
        generation.verify_generation_unchanged()?;
        source.revalidate(state.kura())?;
        let genesis_account = source
            .genesis
            .external_transactions()
            .next()
            .ok_or(FloorRehearsalError::Invalid(
                "original signed genesis has no authority",
            ))?
            .authority()
            .clone();
        let consensus_mode =
            iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(&source.genesis)
                .map_err(|error| {
                    crate::execution_attempt::genesis_read_attempt_error(error, |original| original)
                })?
                .mode
                .into();
        let source_lanes = state.kura().store_root().join("lanes");
        let replay = replay_check::check_replay_retained(
            ReplayCheckInputs {
                state,
                source_lanes,
                genesis_account,
                consensus_mode,
            },
            progress,
        )?;
        let completed = ReplayedFloorCheckpoint {
            generation,
            source,
            floor,
            replay,
        };
        completed.revalidate()?;
        Ok(completed)
    }
}

impl ReplayedFloorCheckpoint {
    /// Borrow the original completed State for the candidate-linked deletion rehearsal.
    /// The retained stopped source and checkpoint must still be unchanged.
    #[cfg(test)]
    pub(crate) fn checked_replayed_state(&self) -> Result<&State, FloorRehearsalError> {
        self.revalidate()?;
        Ok(self.replay.state())
    }

    fn revalidate(&self) -> Result<(), FloorRehearsalError> {
        self.generation.verify_generation_unchanged()?;
        let state = self.replay.state();
        self.source.revalidate(state.kura())?;
        if state.committed_height() != self.source.through {
            return Err(FloorRehearsalError::Invalid(
                "replay did not reach the original tip",
            ));
        }
        let view = state.try_view_once()?;
        let history = CertifiedChain::new(&view)?;
        let terminal = history.certified(self.source.through as u64)?;
        if terminal.committed().block() != &self.source.tip
            || view.network_id() != &self.floor.network
        {
            return Err(FloorRehearsalError::Invalid(
                "replayed terminal certificate changed",
            ));
        }
        // The snapshot binds H and its time; H+1 is the complete-State authority. Its
        // parent hash is also available when H's native body is correctly pruned.
        let successor = history.certified(self.floor.height + 1)?;
        let native = successor.committed().block().header();
        if native.prev_block_hash().map(|hash| *hash.as_ref()) != Some(self.floor.block_hash) {
            return Err(FloorRehearsalError::Invalid(
                "checkpoint is not the certified parent",
            ));
        }
        Ok(())
    }

    pub(crate) fn sign(
        self,
        key: &KeyPair,
    ) -> Result<(StartupFloorV1, SignedStartupFloorAttestationV1), FloorRehearsalError> {
        self.revalidate()?;
        let view = self.replay.state().try_view_once()?;
        if !view
            .commit_topology()
            .iter()
            .any(|peer| peer.public_key() == key.public_key())
        {
            return Err(FloorRehearsalError::Invalid(
                "signer is not in the replayed current committee",
            ));
        }
        drop(view);
        // Same model domains and canonical wire as StartupFloorV1/its statement. Keep
        // original codec and signing failures before the historical model String boundary.
        let floor_id = norito::core::classify_decode_attempt(|| {
            let mut encoded = Ok(());
            let hash = Hash::new_from_writer(|writer| {
                writer.write_all(STARTUP_FLOOR_DOMAIN)?;
                encoded = norito::core::write_canonical_to_writer(&self.floor, writer);
                Ok(())
            })
            .map_err(norito::Error::Io)?;
            encoded?;
            Ok(<[u8; 32]>::from(hash))
        })?;
        let attestation = StartupFloorAttestationV1 {
            floor: floor_id,
            validator: key.public_key().clone(),
        };
        let frame_len =
            norito::core::classify_decode_attempt(|| norito::canonical_frame_len(&attestation))?;
        let length = frame_len
            .checked_add(STARTUP_FLOOR_ATTESTATION_DOMAIN.len())
            .ok_or(FloorRehearsalError::Invalid("statement extent overflow"))?;
        let mut payload = ChargedBuffer::new(length, &self.source.budget)?;
        for _ in 0..length {
            payload.push_reserved(0_u8);
        }
        let (domain, frame) = payload
            .as_mut_slice()
            .split_at_mut(STARTUP_FLOOR_ATTESTATION_DOMAIN.len());
        domain.copy_from_slice(STARTUP_FLOOR_ATTESTATION_DOMAIN);
        norito::core::classify_decode_attempt(|| {
            let mut writer = frame;
            norito::core::write_canonical_to_writer(&attestation, &mut writer)?;
            if !writer.is_empty() {
                return Err(norito::Error::LengthMismatch);
            }
            Ok(())
        })?;
        self.revalidate()?;
        let signature = Signature::try_new(key.private_key(), payload.as_slice())?;
        self.revalidate()?;
        Ok((
            self.floor,
            SignedStartupFloorAttestationV1 {
                attestation,
                signature,
            },
        ))
    }
}

#[cfg(test)]
mod tests;
