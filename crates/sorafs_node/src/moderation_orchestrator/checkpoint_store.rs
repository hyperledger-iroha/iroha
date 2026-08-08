//! Deployment-owned sealed checkpoint authority for moderation orchestration.

use super::*;

/// Canonical sealed-record schema version.
pub const MODERATION_CHECKPOINT_STORE_RECORD_VERSION_V1: u16 = 1;

const MODERATION_CHECKPOINT_NAMESPACE_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.checkpoint-namespace.v1";
const MODERATION_CHECKPOINT_RECORD_REVISION_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.checkpoint-record-revision.v1";

/// Fixed, payload-free failures from the external checkpoint authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationCheckpointStoreExternalErrorV1 {
    /// No authoritative result is available and no write was committed.
    Unavailable,
    /// The request was rejected, including a failed compare-and-swap.
    Rejected,
    /// The write may have committed and requires authoritative readback.
    Ambiguous,
}

/// One sealed, predecessor-bound moderation checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationCheckpointStoreRecordV1 {
    /// Record schema version.
    pub version: u16,
    /// Chain-bound service namespace.
    pub namespace_digest: [u8; 32],
    /// Monotonic checkpoint generation.
    pub checkpoint_generation: u64,
    /// Revision of the exact predecessor record, absent only at genesis.
    pub predecessor_revision: Option<[u8; 32]>,
    /// Digest of the exact predecessor checkpoint bytes, absent only at genesis.
    pub predecessor_checkpoint_digest: Option<[u8; 32]>,
    /// Digest of `checkpoint_bytes`.
    pub checkpoint_digest: [u8; 32],
    /// Canonical moderation checkpoint bytes.
    pub checkpoint_bytes: Vec<u8>,
    /// Exact stable checkpoint-provider handle.
    pub checkpoint_store_handle: String,
    /// Exact adapter/public-policy revision.
    pub checkpoint_store_revision: u64,
    /// Exact public-policy digest.
    pub checkpoint_store_policy_digest: [u8; 32],
    /// Deterministic revision of every preceding field.
    pub revision: [u8; 32],
}

impl ModerationCheckpointStoreRecordV1 {
    /// Verify the canonical envelope fields visible at a runtime-provider boundary.
    ///
    /// Chain namespace and exact predecessor ancestry require orchestrator
    /// context and are checked separately when the record is opened or
    /// committed.
    #[must_use]
    pub fn has_valid_provider_envelope(
        &self,
        expected_handle: &str,
        expected_qualification: ModerationRuntimeProviderQualificationV1,
        checkpoint_max_bytes: u64,
    ) -> bool {
        let predecessor_shape_is_valid = if self.checkpoint_generation == 0 {
            self.predecessor_revision.is_none() && self.predecessor_checkpoint_digest.is_none()
        } else {
            self.predecessor_revision
                .is_some_and(|revision| revision != [0; 32])
                && self
                    .predecessor_checkpoint_digest
                    .is_some_and(|digest| digest != [0; 32])
        };
        let checkpoint_digest = domain_hash(
            b"sorafs.moderation.checkpoint-bytes.v1",
            &[self.checkpoint_bytes.as_slice()],
        );
        self.version == MODERATION_CHECKPOINT_STORE_RECORD_VERSION_V1
            && self.namespace_digest != [0; 32]
            && predecessor_shape_is_valid
            && self.checkpoint_digest != [0; 32]
            && self.checkpoint_digest == checkpoint_digest
            && !self.checkpoint_bytes.is_empty()
            && u64::try_from(self.checkpoint_bytes.len()).unwrap_or(u64::MAX)
                <= checkpoint_max_bytes
            && self.checkpoint_store_handle == expected_handle
            && self.checkpoint_store_revision == expected_qualification.revision()
            && self.checkpoint_store_policy_digest == expected_qualification.policy_digest()
            && self.revision != [0; 32]
            && self.revision == record_revision(self)
    }
}

/// Deployment-owned linearizable sealed checkpoint authority.
///
/// Implementations must durably reject a different record for an already
/// committed predecessor. Credentials and sealing keys remain inside the
/// provider and must never appear in records, configuration, or diagnostics.
pub trait ModerationCheckpointStoreV1: ModerationRuntimeProviderV1 {
    /// Return the archive-lifetime-stable Ed25519 trust anchor authenticating
    /// terminal-set attestations. HSM-internal rotation must preserve it in V1.
    fn attestation_public_key(&self) -> [u8; 32];

    /// Load the latest committed record for this configured moderation namespace.
    fn load_latest(
        &self,
    ) -> Result<Option<ModerationCheckpointStoreRecordV1>, ModerationCheckpointStoreExternalErrorV1>;

    /// Atomically commit `next` only when the latest revision equals `expected_revision`.
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &ModerationCheckpointStoreRecordV1,
    ) -> Result<(), ModerationCheckpointStoreExternalErrorV1>;

    /// Sign a terminal-set statement only when its source record is the exact
    /// currently committed record in this checkpoint namespace.
    fn attest_terminal_set(
        &self,
        statement: &ModerationPanelNotificationSourceAttestationV1,
    ) -> Result<[u8; 64], ModerationCheckpointStoreExternalErrorV1>;
}

pub(super) struct QualifiedModerationCheckpointStoreV1 {
    handle: String,
    qualification: ModerationRuntimeProviderQualificationV1,
    attestation_public_key: [u8; 32],
    store: Arc<dyn ModerationCheckpointStoreV1>,
}

impl QualifiedModerationCheckpointStoreV1 {
    pub(super) fn try_new(
        handle: &str,
        qualification: ModerationRuntimeProviderQualificationV1,
        expected_attestation_public_key: [u8; 32],
        store: Arc<dyn ModerationCheckpointStoreV1>,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        qualify_moderation_runtime_provider_v1(handle, qualification, store.as_ref())?;
        let observed_public_key = store.attestation_public_key();
        revalidate_moderation_runtime_provider_v1(handle, qualification, store.as_ref())?;
        if observed_public_key != expected_attestation_public_key
            || PublicKey::from_bytes(Algorithm::Ed25519, &observed_public_key).is_err()
        {
            return Err(ModerationRuntimeProviderQualificationErrorV1::ArchivePublicKeyChanged);
        }
        Ok(Self {
            handle: handle.to_owned(),
            qualification,
            attestation_public_key: expected_attestation_public_key,
            store,
        })
    }

    fn revalidate(&self) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(
            &self.handle,
            self.qualification,
            self.store.as_ref(),
        )
    }

    fn load_latest(
        &self,
    ) -> Result<Option<ModerationCheckpointStoreRecordV1>, ModerationCheckpointStoreExternalErrorV1>
    {
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Unavailable)?;
        let result = self.store.load_latest();
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Ambiguous)?;
        result
    }

    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &ModerationCheckpointStoreRecordV1,
    ) -> Result<(), ModerationCheckpointStoreExternalErrorV1> {
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Unavailable)?;
        let result = self.store.compare_and_swap_latest(expected_revision, next);
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Ambiguous)?;
        result
    }

    pub(super) fn attest_terminal_set(
        &self,
        statement: &ModerationPanelNotificationSourceAttestationV1,
    ) -> Result<[u8; 64], ModerationCheckpointStoreExternalErrorV1> {
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Unavailable)?;
        if self.store.attestation_public_key() != self.attestation_public_key {
            return Err(ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        let latest = self
            .store
            .load_latest()?
            .ok_or(ModerationCheckpointStoreExternalErrorV1::Rejected)?;
        let expected_chain_id = statement
            .chain_id
            .parse::<iroha_data_model::ChainId>()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Rejected)?;
        if validate_moderation_panel_notification_source_attestation_for_broker_v1(
            statement,
            &expected_chain_id,
            &self.handle,
            self.qualification,
            self.attestation_public_key,
            &latest,
        )
        .is_err()
        {
            return Err(ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        let result = self.store.attest_terminal_set(statement);
        self.revalidate()
            .map_err(|_| ModerationCheckpointStoreExternalErrorV1::Ambiguous)?;
        if self.store.attestation_public_key() != self.attestation_public_key {
            return Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous);
        }
        if self.store.load_latest()?.as_ref() != Some(&latest) {
            return Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous);
        }
        result
    }
}

impl fmt::Debug for QualifiedModerationCheckpointStoreV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationCheckpointStoreV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("attestation_public_key", &self.attestation_public_key)
            .field("store", &"<runtime-only>")
            .finish()
    }
}

pub(super) fn open_authoritative_checkpoint(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    store: &QualifiedModerationCheckpointStoreV1,
) -> Result<
    (
        ModerationOrchestratorCheckpointV1,
        ModerationCheckpointStoreRecordV1,
    ),
    ModerationOrchestratorError,
> {
    ensure_secure_parent(&config.checkpoint_path)?;
    let local_cache = read_bounded_file(&config.checkpoint_path, config.checkpoint_max_bytes)?;
    let observed = store
        .load_latest()
        .map_err(map_checkpoint_store_read_error)?;
    let (state, record) = match observed {
        Some(record) => decode_validated_record(config, chain_id, &record, None)?,
        None => {
            if local_cache.is_some() {
                return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
            }
            let state = ModerationOrchestratorCheckpointV1::new(chain_id);
            let record = build_record(config, chain_id, &state, None)?;
            match store.compare_and_swap_latest(None, &record) {
                Ok(()) => (state, record),
                Err(ModerationCheckpointStoreExternalErrorV1::Unavailable) => {
                    return Err(ModerationOrchestratorError::CheckpointStoreUnavailable);
                }
                Err(ModerationCheckpointStoreExternalErrorV1::Rejected)
                | Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous) => {
                    let authoritative = store
                        .load_latest()
                        .map_err(map_checkpoint_store_read_error)?
                        .ok_or(ModerationOrchestratorError::CheckpointStoreAmbiguous)?;
                    decode_validated_record(config, chain_id, &authoritative, None)?
                }
            }
        }
    };
    if let Some(bytes) = local_cache {
        let cache = decode_checkpoint(config, chain_id, &bytes)?;
        if cache.generation > state.generation
            || (cache.generation == state.generation && bytes != record.checkpoint_bytes)
        {
            return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
        }
    }
    persist_local_cache(config, &record.checkpoint_bytes)?;
    Ok((state, record))
}

pub(super) fn persist_authoritative_checkpoint(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    store: &QualifiedModerationCheckpointStoreV1,
    previous: &mut ModerationCheckpointStoreRecordV1,
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<(), ModerationOrchestratorError> {
    state.generation = state
        .generation
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
    refresh_panel_notification_outbox_digest(state);
    validate_checkpoint(state, config, chain_id)?;
    let next = build_record(config, chain_id, state, Some(previous))?;
    let expected_revision = Some(previous.revision);
    match store.compare_and_swap_latest(expected_revision, &next) {
        Ok(()) => {}
        Err(ModerationCheckpointStoreExternalErrorV1::Unavailable) => {
            return Err(ModerationOrchestratorError::CheckpointStoreUnavailable);
        }
        Err(ModerationCheckpointStoreExternalErrorV1::Rejected)
        | Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous) => {
            let authoritative = store
                .load_latest()
                .map_err(map_checkpoint_store_read_error)?
                .ok_or(ModerationOrchestratorError::CheckpointStoreAmbiguous)?;
            validate_record(config, chain_id, &authoritative, None)?;
            if authoritative != next {
                return Err(ModerationOrchestratorError::CheckpointStoreFenced);
            }
        }
    }
    persist_local_cache(config, &next.checkpoint_bytes)?;
    *previous = next;
    Ok(())
}

fn build_record(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    state: &ModerationOrchestratorCheckpointV1,
    previous: Option<&ModerationCheckpointStoreRecordV1>,
) -> Result<ModerationCheckpointStoreRecordV1, ModerationOrchestratorError> {
    let checkpoint_bytes = norito::to_bytes(state).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!("encode checkpoint: {error}"))
    })?;
    if u64::try_from(checkpoint_bytes.len()).unwrap_or(u64::MAX) > config.checkpoint_max_bytes {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "checkpoint bytes",
            limit: usize::try_from(config.checkpoint_max_bytes).unwrap_or(usize::MAX),
        });
    }
    let mut record = ModerationCheckpointStoreRecordV1 {
        version: MODERATION_CHECKPOINT_STORE_RECORD_VERSION_V1,
        namespace_digest: checkpoint_namespace(chain_id),
        checkpoint_generation: state.generation,
        predecessor_revision: previous.map(|record| record.revision),
        predecessor_checkpoint_digest: previous.map(|record| record.checkpoint_digest),
        checkpoint_digest: domain_hash(
            b"sorafs.moderation.checkpoint-bytes.v1",
            &[checkpoint_bytes.as_slice()],
        ),
        checkpoint_bytes,
        checkpoint_store_handle: config.checkpoint_store_handle.clone(),
        checkpoint_store_revision: config.expected_checkpoint_store_qualification.revision(),
        checkpoint_store_policy_digest: config
            .expected_checkpoint_store_qualification
            .policy_digest(),
        revision: [0; 32],
    };
    record.revision = record_revision(&record);
    validate_record(config, chain_id, &record, previous)?;
    Ok(record)
}

fn decode_validated_record(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    record: &ModerationCheckpointStoreRecordV1,
    previous: Option<&ModerationCheckpointStoreRecordV1>,
) -> Result<
    (
        ModerationOrchestratorCheckpointV1,
        ModerationCheckpointStoreRecordV1,
    ),
    ModerationOrchestratorError,
> {
    validate_record(config, chain_id, record, previous)?;
    let state = decode_checkpoint(config, chain_id, &record.checkpoint_bytes)?;
    if state.generation != record.checkpoint_generation {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "sealed record generation does not match checkpoint".to_owned(),
        ));
    }
    Ok((state, record.clone()))
}

fn validate_record(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    record: &ModerationCheckpointStoreRecordV1,
    previous: Option<&ModerationCheckpointStoreRecordV1>,
) -> Result<(), ModerationOrchestratorError> {
    let lineage_valid = if let Some(previous) = previous {
        let expected_generation = previous
            .checkpoint_generation
            .checked_add(1)
            .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
        record.checkpoint_generation == expected_generation
            && record.predecessor_revision == Some(previous.revision)
            && record.predecessor_checkpoint_digest == Some(previous.checkpoint_digest)
    } else if record.checkpoint_generation == 0 {
        record.predecessor_revision.is_none() && record.predecessor_checkpoint_digest.is_none()
    } else {
        record
            .predecessor_revision
            .is_some_and(|revision| revision != [0; 32])
            && record
                .predecessor_checkpoint_digest
                .is_some_and(|digest| digest != [0; 32])
    };
    if !record.has_valid_provider_envelope(
        &config.checkpoint_store_handle,
        config.expected_checkpoint_store_qualification,
        config.checkpoint_max_bytes,
    ) || record.namespace_digest != checkpoint_namespace(chain_id)
        || !lineage_valid
    {
        return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
    }
    Ok(())
}

fn decode_checkpoint(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    bytes: &[u8],
) -> Result<ModerationOrchestratorCheckpointV1, ModerationOrchestratorError> {
    let limits = checkpoint_decode_limits(config.checkpoint_max_bytes)?;
    let checkpoint =
        decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(bytes, limits)
            .map_err(|error| {
                ModerationOrchestratorError::CheckpointCorrupt(format!(
                    "decode checkpoint: {error}"
                ))
            })?;
    let canonical = norito::to_bytes(&checkpoint).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!("re-encode checkpoint: {error}"))
    })?;
    if canonical != bytes {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "checkpoint is not canonical Norito".to_owned(),
        ));
    }
    validate_checkpoint(&checkpoint, config, chain_id)?;
    Ok(checkpoint)
}

fn persist_local_cache(
    config: &ModerationOrchestratorConfigV1,
    bytes: &[u8],
) -> Result<(), ModerationOrchestratorError> {
    write_atomic(&config.checkpoint_path, bytes)?;
    let persisted = read_bounded_file(&config.checkpoint_path, config.checkpoint_max_bytes)?
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(
                "checkpoint cache disappeared after atomic rename".to_owned(),
            )
        })?;
    if persisted != bytes {
        return Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
            "checkpoint cache bytes changed after atomic rename".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn checkpoint_namespace(chain_id: &iroha_data_model::ChainId) -> [u8; 32] {
    domain_hash(
        MODERATION_CHECKPOINT_NAMESPACE_DOMAIN_V1,
        &[chain_id.as_str().as_bytes()],
    )
}

pub(super) fn record_revision(record: &ModerationCheckpointStoreRecordV1) -> [u8; 32] {
    let predecessor_revision = record.predecessor_revision.unwrap_or([0; 32]);
    let predecessor_checkpoint_digest = record.predecessor_checkpoint_digest.unwrap_or([0; 32]);
    domain_hash(
        MODERATION_CHECKPOINT_RECORD_REVISION_DOMAIN_V1,
        &[
            &record.version.to_be_bytes(),
            &record.namespace_digest,
            &record.checkpoint_generation.to_be_bytes(),
            &predecessor_revision,
            &predecessor_checkpoint_digest,
            &record.checkpoint_digest,
            record.checkpoint_store_handle.as_bytes(),
            &record.checkpoint_store_revision.to_be_bytes(),
            &record.checkpoint_store_policy_digest,
        ],
    )
}

fn map_checkpoint_store_read_error(
    error: ModerationCheckpointStoreExternalErrorV1,
) -> ModerationOrchestratorError {
    match error {
        ModerationCheckpointStoreExternalErrorV1::Unavailable => {
            ModerationOrchestratorError::CheckpointStoreUnavailable
        }
        ModerationCheckpointStoreExternalErrorV1::Rejected
        | ModerationCheckpointStoreExternalErrorV1::Ambiguous => {
            ModerationOrchestratorError::CheckpointStoreAmbiguous
        }
    }
}
