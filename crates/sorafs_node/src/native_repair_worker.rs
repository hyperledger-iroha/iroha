//! Finalized-ledger-driven SoraFS storage repair execution.
//!
//! This module is the only production storage-repair execution boundary. It
//! accepts an exact finalized native task, verifies the current native lease
//! before any storage I/O, and durably enqueues one deterministic native
//! terminal action. It never mutates the process-local repair scheduler.
use crate::{
    NodeHandle, RepairChunkPayload, RepairOrchestrator, RepairOrchestratorError,
    native_repair_singleflight::NativeRepairSingleflightErrorV1,
    repair_ledger_projection::validate_task,
    repair_transaction_forwarder::{
        RepairOperationV1, RepairTransactionContextV1, RepairTransactionEnqueueResultV1,
        RepairTransactionForwarderError,
    },
    store::{ChunkFileRecord, StorageBackend, StoredManifest},
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    isi::sorafs::{
        ApplySorafsRepairTaskAction, SorafsRepairCompleteV1, SorafsRepairFailV1,
        SorafsRepairTaskActionV1,
    },
    sorafs::moderation_ledger::{RepairFinalizedCursorV1, RepairFinalizedTaskV1},
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
/// Maximum chunks inspected for one native repair execution.
pub const NATIVE_REPAIR_MAX_CHUNKS_V1: usize = sorafs_car::CAR_PLAN_MAX_CHUNKS;
/// Maximum source chunk records inspected for one native repair execution.
pub const NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1: usize = 1_000_000;
const TERMINAL_IDEMPOTENCY_PREFIX_V1: &str = "native-repair-terminal-v1";
const TERMINAL_EVIDENCE_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.native-repair.terminal-evidence.v1\0";
/// Exact immutable context exposed to a runtime repair orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRepairExecutionContextV1 {
    /// Exact genesis-header-derived deployment identity.
    pub network_id: NetworkId,
    /// Finalized block anchor from which the task and lease were read.
    pub finalized_cursor: RepairFinalizedCursorV1,
    /// Immutable native repair task identity.
    pub task_id: [u8; 32],
    /// Canonical repair ticket identifier.
    pub ticket_id: String,
    /// Manifest being repaired.
    pub manifest_digest: [u8; 32],
    /// Provider whose local replica is being repaired.
    pub provider_id: [u8; 32],
    /// Canonical account string owning the finalized native lease.
    pub lease_owner_account: String,
    /// Exact finalized task revision.
    pub task_revision: u64,
    /// Exact finalized lease generation.
    pub lease_generation: u64,
}
/// Native terminal action selected after storage verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRepairTerminalKindV1 {
    /// Every manifest chunk passed length and BLAKE3 verification.
    Complete {
        /// Digest of deterministic completion evidence.
        evidence_digest: [u8; 32],
    },
    /// At least one chunk could not be restored and verified.
    Fail {
        /// Digest of deterministic payload-free failure evidence.
        failure_digest: [u8; 32],
    },
}
/// Result of one finalized native repair execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeRepairExecutionOutcomeV1 {
    /// Stable semantic forwarder operation identity.
    pub operation_id: [u8; 32],
    /// Whether the exact terminal operation was newly inserted or already retained.
    pub enqueue_result: RepairTransactionEnqueueResultV1,
    /// Exact terminal action enqueued for chain reconciliation.
    pub terminal_kind: NativeRepairTerminalKindV1,
    /// Invalid chunks observed before rehydration.
    pub invalid_chunks_before: usize,
    /// Chunks atomically restored during this execution.
    pub rehydrated_chunks: usize,
    /// Invalid chunks remaining after final verification.
    pub invalid_chunks_after: usize,
}
/// Failure before a deterministic native terminal action can be enqueued.
#[derive(Debug, Error)]
pub enum NativeRepairExecutionErrorV1 {
    /// Native repair execution is disabled by operational configuration.
    #[error("native repair execution is disabled")]
    Disabled,
    /// Storage is not configured for this node.
    #[error("native repair storage is unavailable")]
    StorageUnavailable,
    /// Local durable storage cannot currently prove healthy state.
    #[error("native repair storage durability is unavailable")]
    StorageDurabilityUnavailable,
    /// The finalized network context is invalid.
    #[error("native repair network context is invalid")]
    InvalidNetworkContext,
    /// The supplied task is not anchored to the exact current finalized cursor.
    #[error("native repair task finalized cursor is stale")]
    StaleFinalizedCursor,
    /// The finalized task or its canonical report is internally inconsistent.
    #[error("native repair task is invalid")]
    InvalidFinalizedTask,
    /// No local provider identity is configured.
    #[error("native repair local provider binding is unavailable")]
    ProviderBindingUnavailable,
    /// The finalized task belongs to another provider.
    #[error("native repair task provider does not match local storage")]
    ProviderBindingMismatch,
    /// The finalized task already has an immutable terminal result.
    #[error("native repair task is already terminal")]
    AlreadyTerminal,
    /// The finalized task has no active worker lease.
    #[error("native repair task has no finalized lease")]
    LeaseMissing,
    /// The finalized lease belongs to another authority.
    #[error("native repair lease owner does not match the execution authority")]
    LeaseOwnerMismatch,
    /// The finalized lease is malformed or expired.
    #[error("native repair lease is invalid or expired")]
    LeaseInvalid,
    /// The local storage scheduler refused bounded execution.
    #[error("native repair storage scheduler is saturated")]
    SchedulerSaturated,
    /// A fixed execution resource ceiling was exceeded.
    #[error("native repair execution resource limit is exceeded")]
    ResourceLimitExceeded,
    /// The native execution lock is poisoned.
    #[error("native repair execution lock is poisoned")]
    RuntimePoisoned,
    /// Durable native transaction forwarding failed.
    #[error("native repair terminal transaction forwarding failed")]
    Forwarder(#[from] RepairTransactionForwarderError),
    /// A lifecycle-leased storage read or replacement failed.
    #[error("native repair storage operation failed")]
    Storage(#[from] crate::store::StorageError),
    /// The configured rehydration orchestrator could not fetch remote chunks.
    #[error("native repair orchestrator rehydration failed")]
    Orchestrator(#[from] RepairOrchestratorError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeRepairFailureCodeV1 {
    ManifestMissing = 1,
    InvalidChunksRemain = 2,
}
#[derive(Debug)]
struct NativeRepairStorageOutcomeV1 {
    failure: Option<NativeRepairFailureCodeV1>,
    evidence_digest: [u8; 32],
    invalid_before: usize,
    rehydrated: usize,
    invalid_after: usize,
}
impl NodeHandle {
    /// Execute storage repair only under one exact finalized native lease.
    ///
    /// The caller must read `finalized_task` from the same immutable finalized view identified by
    /// `transaction_context.finalized_cursor`. Any cursor, provider, canonical-report,
    /// terminal-state, lease-owner, generation, or expiry mismatch is rejected before storage I/O.
    /// `check_authority` must re-read current finalized authority and current UTC from the owning
    /// ledger before every replacement and quarantine publication; it must not trust this snapshot.
    /// Success and bounded failure both enqueue one deterministic native terminal action through
    /// the durable transaction forwarder.
    pub fn execute_finalized_native_repair(
        &self,
        finalized_task: &RepairFinalizedTaskV1,
        authority: &AccountId,
        transaction_context: &RepairTransactionContextV1,
        now_unix_ms: u64,
        check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
    ) -> Result<NativeRepairExecutionOutcomeV1, NativeRepairExecutionErrorV1> {
        if !self.repair_config.enabled() {
            return Err(NativeRepairExecutionErrorV1::Disabled);
        }
        transaction_context
            .validate()
            .map_err(|_| NativeRepairExecutionErrorV1::InvalidNetworkContext)?;
        if finalized_task.finalized_cursor != transaction_context.finalized_cursor {
            return Err(NativeRepairExecutionErrorV1::StaleFinalizedCursor);
        }
        let task = &finalized_task.task;
        validate_task(task).map_err(|_| NativeRepairExecutionErrorV1::InvalidFinalizedTask)?;
        let local_provider = self
            .capacity_usage()
            .provider_id
            .ok_or(NativeRepairExecutionErrorV1::ProviderBindingUnavailable)?;
        if local_provider != task.provider_id {
            return Err(NativeRepairExecutionErrorV1::ProviderBindingMismatch);
        }
        if task.terminal_outcome.is_some() {
            return Err(NativeRepairExecutionErrorV1::AlreadyTerminal);
        }
        let lease = task
            .lease
            .as_ref()
            .ok_or(NativeRepairExecutionErrorV1::LeaseMissing)?;
        if &lease.owner != authority {
            return Err(NativeRepairExecutionErrorV1::LeaseOwnerMismatch);
        }
        if now_unix_ms == 0
            || lease.generation == 0
            || lease.acquired_at_unix_ms == 0
            || lease.renewed_at_unix_ms < lease.acquired_at_unix_ms
            || lease.expires_at_unix_ms <= lease.renewed_at_unix_ms
            || now_unix_ms >= lease.expires_at_unix_ms
        {
            return Err(NativeRepairExecutionErrorV1::LeaseInvalid);
        }
        let storage = self
            .storage
            .as_ref()
            .ok_or(NativeRepairExecutionErrorV1::StorageUnavailable)?;
        storage
            .ensure_durability_healthy()
            .map_err(|_| NativeRepairExecutionErrorV1::StorageDurabilityUnavailable)?;
        let execution_context = NativeRepairExecutionContextV1 {
            network_id: transaction_context.network_id,
            finalized_cursor: transaction_context.finalized_cursor,
            task_id: task.task_id,
            ticket_id: task.ticket_id.clone(),
            manifest_digest: task.manifest_digest,
            provider_id: task.provider_id,
            lease_owner_account: authority.to_string(),
            task_revision: task.revision,
            lease_generation: lease.generation,
        };
        let _execution_guard = self
            .native_repair_singleflight
            .try_enter(task.task_id)
            .map_err(|error| match error {
                NativeRepairSingleflightErrorV1::AlreadyInFlight
                | NativeRepairSingleflightErrorV1::AtCapacity => {
                    NativeRepairExecutionErrorV1::SchedulerSaturated
                }
                NativeRepairSingleflightErrorV1::InvalidTaskId => {
                    NativeRepairExecutionErrorV1::InvalidFinalizedTask
                }
                NativeRepairSingleflightErrorV1::Poisoned => {
                    NativeRepairExecutionErrorV1::RuntimePoisoned
                }
            })?;
        check_authority()?;
        let orchestrator = self.repair_orchestrator();
        let storage_outcome = self
            .schedulers
            .try_with_pin(|| {
                execute_storage_repair(
                    storage,
                    orchestrator.as_deref(),
                    &execution_context,
                    check_authority,
                )
            })
            .map_err(|_| NativeRepairExecutionErrorV1::SchedulerSaturated)??;
        let idempotency_key = format!(
            "{TERMINAL_IDEMPOTENCY_PREFIX_V1}:{}:{}:{}",
            hex::encode(task.task_id),
            task.revision,
            lease.generation
        );
        let (action, terminal_kind) = match storage_outcome.failure {
            None => {
                let evidence_digest = storage_outcome.evidence_digest;
                (
                    SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                        lease_generation: lease.generation,
                        evidence_digest,
                        idempotency_key,
                    }),
                    NativeRepairTerminalKindV1::Complete { evidence_digest },
                )
            }
            Some(_) => {
                let failure_digest = storage_outcome.evidence_digest;
                (
                    SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
                        lease_generation: lease.generation,
                        failure_digest,
                        idempotency_key,
                    }),
                    NativeRepairTerminalKindV1::Fail { failure_digest },
                )
            }
        };
        check_authority()?;
        let enqueue_result = self.enqueue_repair_transaction(
            authority.clone(),
            RepairOperationV1::Action(ApplySorafsRepairTaskAction::new(
                task.ticket_id.clone(),
                task.revision,
                action,
            )),
            transaction_context,
        )?;
        Ok(NativeRepairExecutionOutcomeV1 {
            operation_id: enqueue_result.operation_id(),
            enqueue_result,
            terminal_kind,
            invalid_chunks_before: storage_outcome.invalid_before,
            rehydrated_chunks: storage_outcome.rehydrated,
            invalid_chunks_after: storage_outcome.invalid_after,
        })
    }
}
fn execute_storage_repair(
    storage: &StorageBackend,
    orchestrator: Option<&dyn RepairOrchestrator>,
    context: &NativeRepairExecutionContextV1,
    check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
) -> Result<NativeRepairStorageOutcomeV1, NativeRepairExecutionErrorV1> {
    let Some(outcome) =
        storage.with_manifest_io_by_digest(&context.manifest_digest, |manifest| {
            execute_storage_repair_for_manifest(
                storage,
                orchestrator,
                context,
                manifest,
                check_authority,
            )
        })?
    else {
        return Ok(NativeRepairStorageOutcomeV1 {
            failure: Some(NativeRepairFailureCodeV1::ManifestMissing),
            evidence_digest: missing_manifest_failure_digest(context)?,
            invalid_before: 0,
            rehydrated: 0,
            invalid_after: 0,
        });
    };
    outcome
}
fn execute_storage_repair_for_manifest(
    storage: &StorageBackend,
    orchestrator: Option<&dyn RepairOrchestrator>,
    context: &NativeRepairExecutionContextV1,
    manifest: &StoredManifest,
    check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
) -> Result<NativeRepairStorageOutcomeV1, NativeRepairExecutionErrorV1> {
    check_authority()?;
    manifest_chunks_bounded(manifest)?;
    let mut invalid = invalid_chunks(manifest, check_authority)?;
    let invalid_before = invalid.len();
    if invalid.is_empty() {
        check_authority()?;
        storage.publish_verified_repair(manifest, check_authority)?;
        return Ok(NativeRepairStorageOutcomeV1 {
            failure: None,
            evidence_digest: terminal_evidence_digest(context, None, manifest, &[])?,
            invalid_before,
            rehydrated: 0,
            invalid_after: 0,
        });
    }
    let mut rehydrated = restore_from_local_replicas(storage, manifest, &invalid, check_authority)?;
    invalid = invalid_chunks(manifest, check_authority)?;
    if !invalid.is_empty() {
        rehydrated = rehydrated.saturating_add(restore_from_orchestrator(
            storage,
            orchestrator,
            context,
            manifest,
            &invalid,
            check_authority,
        )?);
    }
    let invalid_after = invalid_chunks(manifest, check_authority)?;
    let failure =
        (!invalid_after.is_empty()).then_some(NativeRepairFailureCodeV1::InvalidChunksRemain);
    if failure.is_none() {
        check_authority()?;
        storage.publish_verified_repair(manifest, check_authority)?;
    }
    Ok(NativeRepairStorageOutcomeV1 {
        failure,
        evidence_digest: terminal_evidence_digest(context, failure, manifest, &invalid_after)?,
        invalid_before,
        rehydrated,
        invalid_after: invalid_after.len(),
    })
}
fn manifest_chunks_bounded(manifest: &StoredManifest) -> Result<(), NativeRepairExecutionErrorV1> {
    validate_repair_inventory(
        manifest.chunk_count(),
        (0..manifest.chunk_count()).map(|index| manifest.chunk(index).map(|chunk| chunk.length)),
    )
}
fn validate_repair_inventory(
    count: usize,
    lengths: impl IntoIterator<Item = Option<u32>>,
) -> Result<(), NativeRepairExecutionErrorV1> {
    if count > NATIVE_REPAIR_MAX_CHUNKS_V1 {
        return Err(NativeRepairExecutionErrorV1::ResourceLimitExceeded);
    }
    let mut observed = 0usize;
    for length in lengths {
        let length = length.ok_or(NativeRepairExecutionErrorV1::InvalidFinalizedTask)?;
        observed += 1;
        if observed > count || length == 0 || length > sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES {
            return Err(NativeRepairExecutionErrorV1::ResourceLimitExceeded);
        }
    }
    if observed != count {
        return Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask);
    }
    Ok(())
}
fn checked_chunk(
    manifest: &StoredManifest,
    index: usize,
) -> Result<&ChunkFileRecord, NativeRepairExecutionErrorV1> {
    manifest
        .chunk(index)
        .ok_or(NativeRepairExecutionErrorV1::InvalidFinalizedTask)
}
fn invalid_chunks(
    manifest: &StoredManifest,
    check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
) -> Result<Vec<usize>, NativeRepairExecutionErrorV1> {
    let mut invalid = Vec::new();
    for index in 0..manifest.chunk_count() {
        check_authority()?;
        if read_valid_chunk(checked_chunk(manifest, index)?).is_none() {
            invalid
                .try_reserve(1)
                .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?;
            invalid.push(index);
        }
    }
    Ok(invalid)
}
fn read_valid_chunk(chunk: &ChunkFileRecord) -> Option<Vec<u8>> {
    crate::store::read_verified_chunk_file(chunk).ok()
}
fn restore_from_local_replicas(
    storage: &StorageBackend,
    target_manifest: &StoredManifest,
    invalid: &[usize],
    check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
) -> Result<usize, NativeRepairExecutionErrorV1> {
    let required = invalid
        .iter()
        .map(|index| checked_chunk(target_manifest, *index).map(|chunk| chunk.digest))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut inspected = 0_usize;
    let mut sources = BTreeMap::<[u8; 32], (String, usize)>::new();
    collect_local_repair_sources(target_manifest, &required, &mut inspected, &mut sources)?;
    let mut after = None;
    let mut manifests_inspected = 0usize;
    while sources.len() < required.len()
        && inspected < NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1
        && manifests_inspected < NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1
    {
        check_authority()?;
        let Some(manifest_id) = storage.next_repair_manifest_id(after.as_deref())? else {
            break;
        };
        after = Some(manifest_id.clone());
        manifests_inspected += 1;
        if manifest_id == target_manifest.manifest_id() {
            continue;
        }
        let result = storage.with_manifest_io(&manifest_id, |manifest| {
            collect_local_repair_sources(manifest, &required, &mut inspected, &mut sources)
        });
        match result {
            Ok(result) => result?,
            Err(
                crate::store::StorageError::ManifestNotFound { .. }
                | crate::store::StorageError::ManifestRetirementInProgress { .. },
            ) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let mut restored = 0_usize;
    for index in invalid {
        check_authority()?;
        let target = checked_chunk(target_manifest, *index)?;
        if read_valid_chunk(target).is_some() {
            continue;
        }
        let Some((source_manifest_id, source_index)) = sources.get(&target.digest) else {
            continue;
        };
        // Retain only locators during discovery. Re-read one authenticated chunk while its owning
        // manifest is leased; GC may remove the source only after the bytes have been copied.
        let bytes = if source_manifest_id == target_manifest.manifest_id() {
            read_valid_chunk(checked_chunk(target_manifest, *source_index)?)
        } else {
            match storage.with_manifest_io(source_manifest_id, |source| {
                source.chunk(*source_index).and_then(read_valid_chunk)
            }) {
                Ok(bytes) => bytes,
                Err(
                    crate::store::StorageError::ManifestNotFound { .. }
                    | crate::store::StorageError::ManifestRetirementInProgress { .. },
                ) => None,
                Err(error) => return Err(error.into()),
            }
        };
        let Some(bytes) = bytes else { continue };
        if bytes.len() != target.length as usize
            || blake3::hash(&bytes).as_bytes() != &target.digest
        {
            continue;
        }
        check_authority()?;
        storage.replace_chunk_for_repair(target_manifest, target, &bytes)?;
        if read_valid_chunk(target).is_some() {
            restored = restored.saturating_add(1);
        }
    }
    Ok(restored)
}
fn collect_local_repair_sources(
    manifest: &StoredManifest,
    required: &BTreeSet<[u8; 32]>,
    inspected: &mut usize,
    sources: &mut BTreeMap<[u8; 32], (String, usize)>,
) -> Result<(), NativeRepairExecutionErrorV1> {
    for index in 0..manifest.chunk_count() {
        if sources.len() == required.len() {
            break;
        }
        *inspected = (*inspected)
            .checked_add(1)
            .ok_or(NativeRepairExecutionErrorV1::ResourceLimitExceeded)?;
        if *inspected > NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1 {
            // Reaching the local discovery budget leaves remote retrieval available.
            return Ok(());
        }
        let Some(candidate) = manifest.chunk(index) else {
            return Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask);
        };
        if !required.contains(&candidate.digest) || sources.contains_key(&candidate.digest) {
            continue;
        }
        if read_valid_chunk(candidate).is_none() {
            continue;
        }
        sources.insert(candidate.digest, (manifest.manifest_id().to_owned(), index));
    }
    Ok(())
}
fn restore_from_orchestrator(
    storage: &StorageBackend,
    orchestrator: Option<&dyn RepairOrchestrator>,
    context: &NativeRepairExecutionContextV1,
    manifest: &StoredManifest,
    invalid: &[usize],
    check_authority: &dyn Fn() -> Result<(), NativeRepairExecutionErrorV1>,
) -> Result<usize, NativeRepairExecutionErrorV1> {
    let Some(orchestrator) = orchestrator else {
        return Ok(0);
    };
    let requested = invalid
        .iter()
        .map(|index| checked_chunk(manifest, *index))
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected = BTreeMap::<[u8; 32], Vec<&ChunkFileRecord>>::new();
    for target in &requested {
        expected.entry(target.digest).or_default().push(*target);
    }
    let mut seen = BTreeSet::new();
    let mut restored = 0_usize;
    orchestrator.rehydrate_missing_chunks(context, manifest, &requested, &mut |payload| {
        let targets = expected.get(&payload.digest).ok_or_else(|| {
            RepairOrchestratorError::other("repair returned an unrequested chunk")
        })?;
        if !seen.insert(payload.digest) {
            return Err(RepairOrchestratorError::other(
                "repair returned a duplicate chunk",
            ));
        }
        validate_repair_chunk_payload(&payload, targets)?;
        for target in targets {
            if read_valid_chunk(target).is_some() || payload.bytes.len() != target.length as usize {
                continue;
            }
            check_authority()
                .map_err(|_| RepairOrchestratorError::other("native repair authority changed"))?;
            storage.replace_chunk_for_repair(manifest, target, &payload.bytes)?;
            if read_valid_chunk(target).is_some() {
                restored = restored.saturating_add(1);
            }
        }
        Ok(())
    })?;
    Ok(restored)
}
fn validate_repair_chunk_payload(
    payload: &RepairChunkPayload,
    requested: &[&ChunkFileRecord],
) -> Result<(), RepairOrchestratorError> {
    if payload.bytes.len() > sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES as usize
        || !requested.iter().any(|target| {
            target.digest == payload.digest && target.length as usize == payload.bytes.len()
        })
        || blake3::hash(&payload.bytes).as_bytes() != &payload.digest
    {
        return Err(RepairOrchestratorError::other(
            "repair returned invalid chunk bytes",
        ));
    }
    Ok(())
}
fn terminal_evidence_digest(
    context: &NativeRepairExecutionContextV1,
    failure: Option<NativeRepairFailureCodeV1>,
    manifest: &StoredManifest,
    invalid_after: &[usize],
) -> Result<[u8; 32], NativeRepairExecutionErrorV1> {
    let mut hasher = terminal_evidence_hasher(context)?;
    match failure {
        None => hasher.update(&[0]),
        Some(code) => hasher.update(&[1, code as u8]),
    };
    manifest_chunks_bounded(manifest)?;
    hash_u64(
        &mut hasher,
        u64::try_from(manifest.chunk_count())
            .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?,
    );
    for index in 0..manifest.chunk_count() {
        let chunk = checked_chunk(manifest, index)?;
        hash_u64(&mut hasher, chunk.offset);
        hash_u64(&mut hasher, u64::from(chunk.length));
        hasher.update(&chunk.digest);
    }
    hash_u64(
        &mut hasher,
        u64::try_from(invalid_after.len())
            .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?,
    );
    for index in invalid_after {
        let chunk = checked_chunk(manifest, *index)?;
        hash_u64(&mut hasher, chunk.offset);
        hash_u64(&mut hasher, u64::from(chunk.length));
        hasher.update(&chunk.digest);
    }
    Ok(*hasher.finalize().as_bytes())
}
fn missing_manifest_failure_digest(
    context: &NativeRepairExecutionContextV1,
) -> Result<[u8; 32], NativeRepairExecutionErrorV1> {
    let mut hasher = terminal_evidence_hasher(context)?;
    hasher.update(&[1, NativeRepairFailureCodeV1::ManifestMissing as u8]);
    Ok(*hasher.finalize().as_bytes())
}
fn terminal_evidence_hasher(
    context: &NativeRepairExecutionContextV1,
) -> Result<blake3::Hasher, NativeRepairExecutionErrorV1> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TERMINAL_EVIDENCE_DIGEST_DOMAIN_V1);
    hasher.update(context.network_id.as_bytes());
    hash_u64(&mut hasher, context.finalized_cursor.height);
    hasher.update(&context.finalized_cursor.block_hash);
    hasher.update(&context.task_id);
    hash_bytes(&mut hasher, context.ticket_id.as_bytes())?;
    hasher.update(&context.manifest_digest);
    hasher.update(&context.provider_id);
    hash_bytes(&mut hasher, context.lease_owner_account.as_bytes())?;
    hash_u64(&mut hasher, context.task_revision);
    hash_u64(&mut hasher, context.lease_generation);
    Ok(hasher)
}
fn hash_bytes(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
) -> Result<(), NativeRepairExecutionErrorV1> {
    hash_u64(
        hasher,
        u64::try_from(bytes.len())
            .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?,
    );
    hasher.update(bytes);
    Ok(())
}
fn hash_u64(hasher: &mut blake3::Hasher, value: u64) {
    hasher.update(&value.to_le_bytes());
}
#[cfg(test)]
mod tests {
    include!("native_repair_worker/streaming_tests.rs");
    use super::*;
    #[cfg(unix)]
    use std::{fs, os::unix::fs::symlink, path::Path};
    fn evidence_context(network_seed: u8) -> NativeRepairExecutionContextV1 {
        let genesis_hash =
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([network_seed; iroha_crypto::Hash::LENGTH]),
            );
        NativeRepairExecutionContextV1 {
            network_id: NetworkId::from_genesis_hash(genesis_hash),
            finalized_cursor: RepairFinalizedCursorV1 {
                height: 17,
                block_hash: [0x44; 32],
            },
            task_id: [0x11; 32],
            ticket_id: "REP-VECTOR-1".to_owned(),
            manifest_digest: [0x22; 32],
            provider_id: [0x33; 32],
            lease_owner_account: "authority-vector".to_owned(),
            task_revision: 7,
            lease_generation: 9,
        }
    }
    #[test]
    fn terminal_evidence_domain_binding_matches_fixed_vector() {
        let context = evidence_context(0xA1);
        let digest = terminal_evidence_hasher(&context)
            .expect("hash fixed terminal evidence context")
            .finalize();
        assert_eq!(
            hex::encode(digest.as_bytes()),
            "9868c77c8c987798fa4b20a7783687cf8838a1a4403c3f3da4c246de4dc36750"
        );
        let foreign_network = terminal_evidence_hasher(&evidence_context(0xA2))
            .expect("hash foreign network context")
            .finalize();
        assert_ne!(foreign_network, digest);
        let mut different_task = context.clone();
        different_task.task_id[0] ^= 1;
        assert_ne!(
            terminal_evidence_hasher(&different_task)
                .expect("hash different task context")
                .finalize(),
            digest
        );
        let mut different_block_hash = context.clone();
        different_block_hash.finalized_cursor.block_hash[0] ^= 1;
        assert_ne!(
            terminal_evidence_hasher(&different_block_hash)
                .expect("hash different finalized block context")
                .finalize(),
            digest
        );
        let mut different_height = context;
        different_height.finalized_cursor.height += 1;
        assert_ne!(
            terminal_evidence_hasher(&different_height)
                .expect("hash different finalized height context")
                .finalize(),
            digest
        );
    }
    #[test]
    fn repair_payload_validation_binds_exact_requested_digest_length_and_bytes() {
        let bytes = b"verified remote chunk".to_vec();
        let record = ChunkFileRecord {
            path: "chunk.bin".into(),
            offset: 0,
            length: bytes.len() as u32,
            digest: *blake3::hash(&bytes).as_bytes(),
            role: None,
            group_id: None,
        };
        let mut payload = RepairChunkPayload {
            digest: record.digest,
            bytes,
            source: None,
        };
        validate_repair_chunk_payload(&payload, &[&record]).unwrap();
        assert!(validate_repair_chunk_payload(&payload, &[]).is_err());
        payload.bytes[0] ^= 1;
        assert!(validate_repair_chunk_payload(&payload, &[&record]).is_err());
        payload.bytes[0] ^= 1;
        payload.bytes.push(0);
        assert!(validate_repair_chunk_payload(&payload, &[&record]).is_err());
        payload.bytes.pop();
        payload.digest[0] ^= 1;
        assert!(validate_repair_chunk_payload(&payload, &[&record]).is_err());
    }
    #[cfg(unix)]
    fn chunk_record(path: &Path, bytes: &[u8]) -> ChunkFileRecord {
        ChunkFileRecord {
            path: path.to_path_buf(),
            offset: 0,
            length: u32::try_from(bytes.len()).expect("test chunk length fits u32"),
            digest: *blake3::hash(bytes).as_bytes(),
            role: None,
            group_id: None,
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_repair_chunk_validation_rejects_symlink() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bytes = b"native repair symlink rejection";
        let target = temp.path().join("target.chunk");
        let linked = temp.path().join("linked.chunk");
        fs::write(&target, bytes).expect("write target");
        symlink(&target, &linked).expect("create symlink");
        assert!(read_valid_chunk(&chunk_record(&linked, bytes)).is_none());
    }
    #[cfg(unix)]
    #[test]
    fn native_repair_chunk_validation_rejects_hardlink() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bytes = b"native repair hardlink rejection";
        let chunk = temp.path().join("chunk.bin");
        let alias = temp.path().join("alias.bin");
        fs::write(&chunk, bytes).expect("write chunk");
        fs::hard_link(&chunk, &alias).expect("create hard link");
        assert!(read_valid_chunk(&chunk_record(&chunk, bytes)).is_none());
    }
}
