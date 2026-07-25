//! Finalized-ledger-driven SoraFS storage repair execution.
//!
//! This module is the only production storage-repair execution boundary. It
//! accepts an exact finalized native task, verifies the current native lease
//! before any storage I/O, and durably enqueues one deterministic native
//! terminal action. It never mutates the process-local repair scheduler.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::sorafs::{
        ApplySorafsRepairTaskAction, SorafsRepairCompleteV1, SorafsRepairFailV1,
        SorafsRepairTaskActionV1,
    },
    sorafs::moderation_ledger::{
        REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedCursorV1, RepairFinalizedTaskV1,
        sorafs_repair_task_id_v1,
    },
};
use thiserror::Error;

use crate::{
    NodeHandle, RepairChunkPayload, RepairOrchestrator,
    native_repair_singleflight::NativeRepairSingleflightErrorV1,
    repair_transaction_forwarder::{
        RepairOperationV1, RepairTransactionContextV1, RepairTransactionEnqueueResultV1,
        RepairTransactionForwarderError,
    },
    store::{ChunkFileRecord, StorageBackend, StoredManifest},
};

/// Maximum chunks inspected for one native repair execution.
pub const NATIVE_REPAIR_MAX_CHUNKS_V1: usize = 65_536;
/// Maximum source chunk records inspected for one native repair execution.
pub const NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1: usize = 1_000_000;
/// Maximum aggregate target bytes admitted for one native repair execution.
pub const NATIVE_REPAIR_MAX_TARGET_BYTES_V1: u64 = 1_073_741_824;

const TERMINAL_IDEMPOTENCY_PREFIX_V1: &str = "native-repair-terminal-v1";
const TERMINAL_EVIDENCE_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.native-repair.terminal-evidence.v1\0";

/// Exact immutable context exposed to a runtime repair orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRepairExecutionContextV1 {
    /// Exact active chain identity.
    pub chain_id: ChainId,
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
    /// The finalized chain context is invalid.
    #[error("native repair chain context is invalid")]
    InvalidChainContext,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeRepairFailureCodeV1 {
    ManifestMissing = 1,
    InvalidChunksRemain = 2,
}

#[derive(Debug)]
struct NativeRepairStorageOutcomeV1 {
    failure: Option<NativeRepairFailureCodeV1>,
    manifest: Option<StoredManifest>,
    invalid_before: usize,
    rehydrated: usize,
    invalid_after: Vec<ChunkFileRecord>,
}

impl NodeHandle {
    /// Execute storage repair only under one exact finalized native lease.
    ///
    /// The caller must read `finalized_task` from the same immutable finalized
    /// view identified by `transaction_context.finalized_cursor`. Any cursor,
    /// provider, canonical-report, terminal-state, lease-owner, generation, or
    /// expiry mismatch is rejected before storage I/O. Success and bounded
    /// failure both enqueue one deterministic native terminal action through
    /// the durable transaction forwarder.
    pub fn execute_finalized_native_repair(
        &self,
        finalized_task: &RepairFinalizedTaskV1,
        authority: &AccountId,
        transaction_context: &RepairTransactionContextV1,
        now_unix_ms: u64,
    ) -> Result<NativeRepairExecutionOutcomeV1, NativeRepairExecutionErrorV1> {
        if !self.repair_config.enabled() {
            return Err(NativeRepairExecutionErrorV1::Disabled);
        }
        transaction_context
            .validate()
            .map_err(|_| NativeRepairExecutionErrorV1::InvalidChainContext)?;
        if finalized_task.finalized_cursor != transaction_context.finalized_cursor {
            return Err(NativeRepairExecutionErrorV1::StaleFinalizedCursor);
        }
        let task = &finalized_task.task;
        if task.version != REPAIR_LEDGER_TASK_VERSION_V1
            || task.task_id == [0; 32]
            || task.task_id != sorafs_repair_task_id_v1(task.source_identity)
            || task.revision == 0
            || task.ticket_id.is_empty()
        {
            return Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask);
        }
        let report =
            crate::repair_transaction_forwarder::decode_repair_report(&task.canonical_report)
                .map_err(|_| NativeRepairExecutionErrorV1::InvalidFinalizedTask)?;
        if report.ticket_id.0 != task.ticket_id
            || report.evidence.manifest_digest != task.manifest_digest
            || report.evidence.provider_id != task.provider_id
            || report.auditor_account != task.submitted_by.to_string()
        {
            return Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask);
        }
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
            chain_id: transaction_context.chain_id.clone(),
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
        let orchestrator = self.repair_orchestrator();
        let storage_outcome = self
            .schedulers
            .try_with_pin(|| {
                execute_storage_repair(storage, orchestrator.as_deref(), &execution_context)
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
                let manifest = storage_outcome
                    .manifest
                    .as_ref()
                    .ok_or(NativeRepairExecutionErrorV1::InvalidFinalizedTask)?;
                let evidence_digest =
                    terminal_evidence_digest(&execution_context, None, manifest, &[])?;
                (
                    SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                        lease_generation: lease.generation,
                        evidence_digest,
                        idempotency_key,
                    }),
                    NativeRepairTerminalKindV1::Complete { evidence_digest },
                )
            }
            Some(failure) => {
                let failure_digest = if let Some(manifest) = storage_outcome.manifest.as_ref() {
                    terminal_evidence_digest(
                        &execution_context,
                        Some(failure),
                        manifest,
                        &storage_outcome.invalid_after,
                    )?
                } else {
                    missing_manifest_failure_digest(&execution_context)?
                };
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
            invalid_chunks_after: storage_outcome.invalid_after.len(),
        })
    }
}

fn execute_storage_repair(
    storage: &StorageBackend,
    orchestrator: Option<&dyn RepairOrchestrator>,
    context: &NativeRepairExecutionContextV1,
) -> Result<NativeRepairStorageOutcomeV1, NativeRepairExecutionErrorV1> {
    let Some(manifest) = storage.manifest_by_digest(&context.manifest_digest) else {
        return Ok(NativeRepairStorageOutcomeV1 {
            failure: Some(NativeRepairFailureCodeV1::ManifestMissing),
            manifest: None,
            invalid_before: 0,
            rehydrated: 0,
            invalid_after: Vec::new(),
        });
    };
    let chunks = manifest_chunks_bounded(&manifest)?;
    let mut invalid = invalid_chunks(&chunks);
    let invalid_before = invalid.len();
    if invalid.is_empty() {
        return Ok(NativeRepairStorageOutcomeV1 {
            failure: None,
            manifest: Some(manifest),
            invalid_before,
            rehydrated: 0,
            invalid_after: Vec::new(),
        });
    }

    let mut rehydrated = restore_from_local_replicas(storage, &invalid)?;
    invalid = invalid_chunks(&chunks);
    if !invalid.is_empty() {
        rehydrated = rehydrated.saturating_add(restore_from_orchestrator(
            orchestrator,
            context,
            &manifest,
            &invalid,
        ));
    }
    let invalid_after = invalid_chunks(&chunks);
    let failure =
        (!invalid_after.is_empty()).then_some(NativeRepairFailureCodeV1::InvalidChunksRemain);
    Ok(NativeRepairStorageOutcomeV1 {
        failure,
        manifest: Some(manifest),
        invalid_before,
        rehydrated,
        invalid_after,
    })
}

fn manifest_chunks_bounded(
    manifest: &StoredManifest,
) -> Result<Vec<ChunkFileRecord>, NativeRepairExecutionErrorV1> {
    if manifest.chunk_count() > NATIVE_REPAIR_MAX_CHUNKS_V1 {
        return Err(NativeRepairExecutionErrorV1::ResourceLimitExceeded);
    }
    let mut total_bytes = 0_u64;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(manifest.chunk_count())
        .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?;
    for index in 0..manifest.chunk_count() {
        let chunk = manifest
            .chunk(index)
            .ok_or(NativeRepairExecutionErrorV1::InvalidFinalizedTask)?;
        total_bytes = total_bytes
            .checked_add(u64::from(chunk.length))
            .ok_or(NativeRepairExecutionErrorV1::ResourceLimitExceeded)?;
        if total_bytes > NATIVE_REPAIR_MAX_TARGET_BYTES_V1 {
            return Err(NativeRepairExecutionErrorV1::ResourceLimitExceeded);
        }
        chunks.push(chunk.clone());
    }
    Ok(chunks)
}

fn invalid_chunks(chunks: &[ChunkFileRecord]) -> Vec<ChunkFileRecord> {
    chunks
        .iter()
        .filter(|chunk| read_valid_chunk(chunk).is_none())
        .cloned()
        .collect()
}

fn read_valid_chunk(chunk: &ChunkFileRecord) -> Option<Vec<u8>> {
    let metadata = fs::symlink_metadata(&chunk.path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() != u64::from(chunk.length) {
        return None;
    }
    let bytes = fs::read(&chunk.path).ok()?;
    if bytes.len() != chunk.length as usize || blake3::hash(&bytes).as_bytes() != &chunk.digest {
        return None;
    }
    Some(bytes)
}

fn restore_from_local_replicas(
    storage: &StorageBackend,
    invalid: &[ChunkFileRecord],
) -> Result<usize, NativeRepairExecutionErrorV1> {
    let required = invalid
        .iter()
        .map(|chunk| chunk.digest)
        .collect::<BTreeSet<_>>();
    let mut manifests = storage.manifests();
    manifests.sort_by(|left, right| left.manifest_id().cmp(right.manifest_id()));
    let mut inspected = 0_usize;
    let mut sources = BTreeMap::<[u8; 32], (String, Vec<u8>)>::new();
    for manifest in manifests {
        for index in 0..manifest.chunk_count() {
            inspected = inspected
                .checked_add(1)
                .ok_or(NativeRepairExecutionErrorV1::ResourceLimitExceeded)?;
            if inspected > NATIVE_REPAIR_MAX_SOURCE_CHUNKS_V1 {
                return Err(NativeRepairExecutionErrorV1::ResourceLimitExceeded);
            }
            let Some(candidate) = manifest.chunk(index) else {
                return Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask);
            };
            if !required.contains(&candidate.digest) || sources.contains_key(&candidate.digest) {
                continue;
            }
            let Some(bytes) = read_valid_chunk(candidate) else {
                continue;
            };
            let key = stable_local_path_key(storage.root_dir(), &candidate.path);
            sources.insert(candidate.digest, (key, bytes));
        }
    }

    let mut restored = 0_usize;
    for target in invalid {
        if read_valid_chunk(target).is_some() {
            continue;
        }
        let Some((_, bytes)) = sources.get(&target.digest) else {
            continue;
        };
        if bytes.len() != target.length as usize {
            continue;
        }
        if crate::store::write_atomic(&target.path, bytes).is_ok()
            && read_valid_chunk(target).is_some()
        {
            restored = restored.saturating_add(1);
        }
    }
    Ok(restored)
}

fn restore_from_orchestrator(
    orchestrator: Option<&dyn RepairOrchestrator>,
    context: &NativeRepairExecutionContextV1,
    manifest: &StoredManifest,
    invalid: &[ChunkFileRecord],
) -> usize {
    let Some(orchestrator) = orchestrator else {
        return 0;
    };
    let Ok(payloads) = orchestrator.rehydrate_missing_chunks(context, manifest, invalid) else {
        return 0;
    };
    if payloads.len() > invalid.len() {
        return 0;
    }
    let mut expected = BTreeMap::<[u8; 32], Vec<&ChunkFileRecord>>::new();
    for target in invalid {
        expected.entry(target.digest).or_default().push(target);
    }
    let mut seen = BTreeSet::new();
    let mut restored = 0_usize;
    for RepairChunkPayload { digest, bytes, .. } in payloads {
        if !seen.insert(digest)
            || blake3::hash(&bytes).as_bytes() != &digest
            || !expected.contains_key(&digest)
        {
            continue;
        }
        for target in &expected[&digest] {
            if read_valid_chunk(target).is_some() || bytes.len() != target.length as usize {
                continue;
            }
            if crate::store::write_atomic(&target.path, &bytes).is_ok()
                && read_valid_chunk(target).is_some()
            {
                restored = restored.saturating_add(1);
            }
        }
    }
    restored
}

fn stable_local_path_key(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn terminal_evidence_digest(
    context: &NativeRepairExecutionContextV1,
    failure: Option<NativeRepairFailureCodeV1>,
    manifest: &StoredManifest,
    invalid_after: &[ChunkFileRecord],
) -> Result<[u8; 32], NativeRepairExecutionErrorV1> {
    let mut hasher = terminal_evidence_hasher(context)?;
    match failure {
        None => hasher.update(&[0]),
        Some(code) => hasher.update(&[1, code as u8]),
    };
    let chunks = manifest_chunks_bounded(manifest)?;
    hash_u64(
        &mut hasher,
        u64::try_from(chunks.len())
            .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?,
    );
    for chunk in &chunks {
        hash_u64(&mut hasher, chunk.offset);
        hash_u64(&mut hasher, u64::from(chunk.length));
        hasher.update(&chunk.digest);
    }
    hash_u64(
        &mut hasher,
        u64::try_from(invalid_after.len())
            .map_err(|_| NativeRepairExecutionErrorV1::ResourceLimitExceeded)?,
    );
    for chunk in invalid_after {
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
    hash_bytes(&mut hasher, context.chain_id.as_str().as_bytes())?;
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
