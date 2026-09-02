//! PoR challenge/proof tracking for the embedded storage node.
use crate::store::StoredManifest;
use ed25519_dalek::{Signature, VerifyingKey};
use iroha_data_model::{
    metadata::Metadata,
    sorafs::{
        moderation_ledger::sorafs_repair_task_id_v1,
        reputation::{PorTerminalFailureKindV1, PorTerminalOutcomeV1, PorTerminalStatusV1},
    },
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use norito::json::Value as JsonValue;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sorafs_car::PorMerkleTree;
use sorafs_manifest::{
    por::{
        AuditOutcomeV1, AuditVerdictV1, POR_CHALLENGE_STATUS_VERSION_V1, PorChallengeOutcome,
        PorChallengeStatusV1, PorChallengeV1, PorChallengeValidationError, PorProofV1,
        PorProofValidationError, derive_challenge_id, derive_challenge_seed,
    },
    repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairPorFailureCauseV1, RepairReportV1, RepairTicketId,
    },
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use thiserror::Error;
const SMALL_LEAF_MAX_LEN: u32 = 4 * 1024;
const GIB: u64 = 1_073_741_824;
const SAMPLE_TIER_EDGE: u16 = 1;
const SAMPLE_TIER_STANDARD: u16 = 2;
const SAMPLE_TIER_ARCHIVAL: u16 = 3;
const DUPLICATE_RETRY_LIMIT: usize = 8;
const SAMPLE_MULTIPLIER_METADATA_KEY: &str = "profile.sample_multiplier";
const SAMPLE_MULTIPLIER_DEFAULT_KEY: &str = "default";
const DEFAULT_SAMPLE_MULTIPLIER: u16 = 1;
const MAX_SAMPLE_MULTIPLIER: u16 = 4;
const DEFAULT_TRACKER_ENTRY_LIMIT: usize =
    iroha_config::parameters::defaults::sorafs::storage::RUNTIME_STATE_ENTRY_LIMIT_MAX;
/// Domain separator for a failed PoR challenge's exactly-once repair source.
pub const POR_REPAIR_SOURCE_ID_DOMAIN_V1: &[u8] = b"sorafs.por.repair-source.v1";
/// Domain separator for one retained PoR-to-reputation delivery binding.
pub const POR_REPUTATION_TERMINAL_WORK_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.por.reputation-terminal-work.v1";
/// Domain separator for canonical finalized-PoR archive records.
pub const POR_FINALIZED_REPLAY_ARCHIVE_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.por.finalized-replay-archive.record.v1";
/// Domain separator for signed finalized-PoR archive heads.
pub const POR_FINALIZED_REPLAY_ARCHIVE_HEAD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.por.finalized-replay-archive.head.v1";
/// Domain separator for a signed absence result at one exact archive head.
pub const POR_FINALIZED_REPLAY_ARCHIVE_ABSENCE_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.por.finalized-replay-archive.absence.v1";
/// Derive the cross-peer repair source identity for one PoR challenge.
#[must_use]
pub fn por_repair_source_identity_v1(challenge_id: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POR_REPAIR_SOURCE_ID_DOMAIN_V1);
    hasher.update(&challenge_id);
    *hasher.finalize().as_bytes()
}
/// Payload-free canonical material needed to enqueue a failed PoR repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorFailedRepairIntentV1 {
    /// Manifest affected by the failed proof.
    pub manifest_digest: [u8; 32],
    /// Provider responsible for the failed proof.
    pub provider_id: [u8; 32],
    /// Challenge whose terminal failed verdict originated the repair.
    pub challenge_id: [u8; 32],
    /// Number of samples that failed verification.
    pub failed_samples: u16,
    /// Canonical proof digest, when the provider submitted a proof.
    pub proof_digest: Option<[u8; 32]>,
    /// Final auditor decision timestamp.
    pub decided_at_unix: u64,
}
impl PorFailedRepairIntentV1 {
    fn validate(self) -> Result<(), PorRepairHandoffError> {
        if self.manifest_digest == [0; 32]
            || self.provider_id == [0; 32]
            || self.challenge_id == [0; 32]
            || self.failed_samples == 0
            || self.decided_at_unix == 0
        {
            return Err(PorRepairHandoffError(
                "failed PoR repair intent contains a zero-valued required field".to_owned(),
            ));
        }
        Ok(())
    }
    /// Return the deterministic exactly-once source identity.
    #[must_use]
    pub fn source_identity(self) -> [u8; 32] {
        por_repair_source_identity_v1(self.challenge_id)
    }
    /// Return the chain-authoritative repair task identity.
    #[must_use]
    pub fn repair_task_id(self) -> [u8; 32] {
        sorafs_repair_task_id_v1(self.source_identity())
    }
}
/// Build the canonical payload-free repair report for a failed PoR verdict.
///
/// The caller supplies the runtime transaction authority; process-local history identifiers,
/// verdict reasons, signatures, and metadata are never copied into the chain payload.
pub fn canonical_por_failure_repair_report_v1(
    intent: PorFailedRepairIntentV1,
    runtime_authority: &str,
) -> Result<RepairReportV1, PorRepairHandoffError> {
    intent.validate()?;
    let report = RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId(format!("POR-{}", hex::encode_upper(intent.challenge_id))),
        auditor_account: runtime_authority.to_owned(),
        submitted_at_unix: intent.decided_at_unix,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: intent.manifest_digest,
            provider_id: intent.provider_id,
            por_history_id: None,
            cause: RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                challenge_id: intent.challenge_id,
                failed_samples: intent.failed_samples,
                proof_digest: intent.proof_digest,
            }),
            evidence_json: None,
            notes: None,
        },
        notes: None,
    };
    report
        .validate()
        .map_err(|error| PorRepairHandoffError(error.to_string()))?;
    Ok(report)
}
/// Payload-free failure returned by the native PoR repair handoff.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("PoR repair handoff failed: {0}")]
pub struct PorRepairHandoffError(pub String);
/// Idempotent native repair handoff used to drain the durable failed-verdict outbox.
pub trait PorRepairHandoff: Send + Sync + std::fmt::Debug {
    /// Enqueue the canonical failed-PoR report exactly once and return its
    /// chain-authoritative task identity.
    fn enqueue_failed_por_repair(
        &self,
        intent: &PorFailedRepairIntentV1,
    ) -> Result<[u8; 32], PorRepairHandoffError>;
}
/// Randomness bundle sourced for a PoR epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorRandomness {
    /// Epoch identifier (`floor(unix_time / 3600)`).
    pub epoch_id: u64,
    /// Unix timestamp when the challenge is issued.
    pub issued_at_unix: u64,
    /// Response window (seconds) before the challenge is considered expired.
    pub response_window_secs: u64,
    /// drand round number.
    pub drand_round: u64,
    /// drand randomness payload (32 bytes).
    pub drand_randomness: [u8; 32],
    /// drand BLS signature covering the randomness.
    pub drand_signature: [u8; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
}
/// Provider VRF output/proof for a manifest/epoch pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestVrfBundle {
    /// Governance-controlled provider identifier bound into the proof.
    pub provider_id: [u8; 32],
    /// Manifest digest bound into the proof.
    pub manifest_digest: [u8; 32],
    /// PoR epoch identifier bound into the proof.
    pub epoch_id: u64,
    /// Drand round bound into the proof.
    pub drand_round: u64,
    /// VRF output bytes.
    pub output: [u8; 32],
    /// Variant-tagged, fixed-size proof attesting to the VRF output.
    pub proof: iroha_crypto::vrf::VrfProof,
}
/// Lookup key for a provider/manifest VRF submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManifestVrfKey {
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Manifest digest.
    pub manifest_digest: [u8; 32],
}
/// Planned PoR challenge alongside sampling metadata.
#[derive(Debug, Clone)]
pub struct PlannedChallenge {
    /// Canonical PoR challenge payload.
    pub challenge: PorChallengeV1,
    /// Number of duplicate sample indices emitted because unique leaves were exhausted.
    pub duplicate_samples: usize,
}
/// Errors returned when deriving PoR challenges.
#[derive(Debug, Error)]
pub enum PorChallengePlannerError {
    /// Storage backend unavailable when planning challenges.
    #[error("storage backend unavailable")]
    StorageDisabled,
    /// Provider identifier not registered for the current storage handle.
    #[error("storage provider unavailable")]
    ProviderUnavailable,
    /// Storage backend has no PoR leaves for the manifest.
    #[error("manifest does not expose any PoR leaves")]
    EmptyMerkleTree,
    /// drand signature is an inert placeholder.
    #[error("drand signature must not be all zero")]
    InvalidDrandSignature,
    /// A VRF is required before the configured forced-challenge deadline.
    #[error("provider VRF is not yet available for manifest {manifest_hex}")]
    MissingVrfBeforeDeadline {
        /// Manifest digest rendered as canonical lowercase hex.
        manifest_hex: String,
    },
    /// A verified VRF bundle was not bound to the planned challenge inputs.
    #[error("provider VRF bundle binding does not match the planned challenge")]
    VrfBindingMismatch,
    /// Sample count exceeded the supported `u16` range.
    #[error("sample count {0} exceeds u16::MAX")]
    SampleCountOverflow(usize),
    /// Manifest chunk profile is not recognised.
    #[error("manifest chunk profile handle is empty")]
    EmptyChunkProfile,
    /// Provider metadata declared an invalid PoR sample multiplier.
    #[error(
        "capacity metadata `profile.sample_multiplier` invalid for provider {provider_hex}: {reason}"
    )]
    InvalidSampleMultiplier {
        /// Provider identifier rendered as a hexadecimal string.
        provider_hex: String,
        /// Human-readable reason describing the configuration error.
        reason: String,
    },
    /// Challenge validation failed for the generated payload.
    #[error("challenge validation failed: {0}")]
    ChallengeInvalid(#[from] sorafs_manifest::por::PorChallengeValidationError),
}
#[derive(Debug, Clone, Copy)]
struct SamplePlan {
    tier: u16,
    small_target: usize,
    large_target: usize,
}
#[derive(Debug)]
struct SampleSelection {
    indices: Vec<u64>,
    duplicate_count: usize,
}
/// Snapshot describing the backlog for a manifest/provider pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PorBacklogEntry {
    /// Manifest digest referenced by the outstanding challenges.
    pub manifest_digest: [u8; 32],
    /// Provider identifier challenged for the manifest.
    pub provider_id: [u8; 32],
    /// Number of outstanding PoR challenges for the pair.
    pub pending_challenges: u64,
    /// Oldest epoch identifier tracked in the backlog.
    pub oldest_epoch_id: Option<u64>,
    /// Earliest response deadline recorded across pending challenges.
    pub oldest_response_deadline_unix: Option<u64>,
}
/// Sampling multiplier policy derived from governance metadata.
#[derive(Debug, Clone)]
pub struct PorSamplePolicy {
    default_multiplier: u16,
    overrides: HashMap<String, u16>,
}
impl Default for PorSamplePolicy {
    fn default() -> Self {
        Self {
            default_multiplier: DEFAULT_SAMPLE_MULTIPLIER,
            overrides: HashMap::new(),
        }
    }
}
impl PorSamplePolicy {
    /// Construct a sampling policy for `provider_id` using the supplied metadata.
    pub fn from_metadata(
        provider_id: [u8; 32],
        metadata: &Metadata,
    ) -> Result<Self, PorChallengePlannerError> {
        let Some(raw_value) = metadata.get(SAMPLE_MULTIPLIER_METADATA_KEY) else {
            return Ok(Self::default());
        };
        let provider_hex = hex::encode(provider_id);
        let json_value = raw_value.try_into_any::<JsonValue>().map_err(|err| {
            PorChallengePlannerError::InvalidSampleMultiplier {
                provider_hex: provider_hex.clone(),
                reason: format!("invalid JSON payload: {err}"),
            }
        })?;
        parse_sample_policy(&json_value).map_err(|err| {
            PorChallengePlannerError::InvalidSampleMultiplier {
                provider_hex,
                reason: err.to_string(),
            }
        })
    }
    /// Return the multiplier associated with `profile_handle`, defaulting to the global value.
    #[must_use]
    pub fn multiplier_for(&self, profile_handle: &str) -> u16 {
        self.overrides
            .get(profile_handle)
            .copied()
            .unwrap_or(self.default_multiplier)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct SampleMultiplierError(String);
fn parse_sample_policy(value: &JsonValue) -> Result<PorSamplePolicy, SampleMultiplierError> {
    match value {
        JsonValue::Number(_) | JsonValue::String(_) => {
            let multiplier = parse_multiplier_value(value, SAMPLE_MULTIPLIER_METADATA_KEY)?;
            Ok(PorSamplePolicy {
                default_multiplier: multiplier,
                overrides: HashMap::new(),
            })
        }
        JsonValue::Object(map) => {
            let mut policy = PorSamplePolicy::default();
            for (key, entry) in map {
                let context = format!("{SAMPLE_MULTIPLIER_METADATA_KEY}.{key}");
                let multiplier = parse_multiplier_value(entry, &context)?;
                if key.eq_ignore_ascii_case(SAMPLE_MULTIPLIER_DEFAULT_KEY) {
                    policy.default_multiplier = multiplier;
                } else {
                    policy.overrides.insert(key.clone(), multiplier);
                }
            }
            Ok(policy)
        }
        JsonValue::Null => Err(SampleMultiplierError(format!(
            "`{SAMPLE_MULTIPLIER_METADATA_KEY}` must not be null"
        ))),
        _ => Err(SampleMultiplierError(format!(
            "`{SAMPLE_MULTIPLIER_METADATA_KEY}` must be a number, string, or object"
        ))),
    }
}
fn parse_multiplier_value(value: &JsonValue, context: &str) -> Result<u16, SampleMultiplierError> {
    if let Some(raw) = value.as_u64() {
        return ensure_multiplier_range(raw, context);
    }
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(SampleMultiplierError(format!(
                "`{context}` must not be an empty string"
            )));
        }
        let parsed = trimmed.parse::<u16>().map_err(|_| {
            SampleMultiplierError(format!(
                "`{context}` must be an integer between 1 and {MAX_SAMPLE_MULTIPLIER}, found `{trimmed}`"
            ))
        })?;
        return ensure_multiplier_range(u64::from(parsed), context);
    }
    Err(SampleMultiplierError(format!(
        "`{context}` must be an integer or string, found {}",
        describe_json_type(value)
    )))
}
fn ensure_multiplier_range(value: u64, context: &str) -> Result<u16, SampleMultiplierError> {
    if value == 0 || value > u64::from(MAX_SAMPLE_MULTIPLIER) {
        return Err(SampleMultiplierError(format!(
            "`{context}` must be between 1 and {MAX_SAMPLE_MULTIPLIER}, found {value}"
        )));
    }
    Ok(value as u16)
}
fn describe_json_type(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}
fn determine_sample_plan(content_len: u64, multiplier: u16) -> SamplePlan {
    let (tier, base_small, base_large) = if content_len < 10 * GIB {
        (SAMPLE_TIER_EDGE, 64usize, 0usize)
    } else if content_len < 100 * GIB {
        (SAMPLE_TIER_STANDARD, 96usize, 32usize)
    } else {
        (SAMPLE_TIER_ARCHIVAL, 0usize, 256usize)
    };
    let factor = usize::from(multiplier);
    SamplePlan {
        tier,
        small_target: base_small.saturating_mul(factor),
        large_target: base_large.saturating_mul(factor),
    }
}
fn draw_samples(
    rng: &mut ChaCha20Rng,
    target: usize,
    specific_pool: &[u64],
    all_pool: &[u64],
    seen: &mut HashSet<u64>,
    selected: &mut Vec<u64>,
    duplicate_count: &mut usize,
) {
    if target == 0 {
        return;
    }
    let effective_pool = if specific_pool.is_empty() {
        all_pool
    } else {
        specific_pool
    };
    if effective_pool.is_empty() {
        return;
    }
    let start_len = selected.len();
    let mut attempts = 0usize;
    while selected.len() - start_len < target {
        let idx = (rng.next_u64() as usize) % effective_pool.len();
        let leaf = effective_pool[idx];
        if seen.insert(leaf) {
            selected.push(leaf);
            attempts = 0;
        } else {
            attempts += 1;
            if attempts >= DUPLICATE_RETRY_LIMIT || seen.len() == all_pool.len() {
                selected.push(leaf);
                *duplicate_count = duplicate_count.saturating_add(1);
                attempts = 0;
            }
        }
    }
}
fn sample_leaf_indices(
    tree: &PorMerkleTree,
    seed: [u8; 32],
    plan: SamplePlan,
) -> Result<SampleSelection, PorChallengePlannerError> {
    let total = tree.leaf_count();
    if total == 0 {
        return Err(PorChallengePlannerError::EmptyMerkleTree);
    }
    let mut all_indices = Vec::with_capacity(total);
    let mut small_indices = Vec::new();
    let mut large_indices = Vec::new();
    let mut flat_index = 0u64;
    for chunk in tree.chunks() {
        for segment in &chunk.segments {
            for leaf in &segment.leaves {
                if leaf.length <= SMALL_LEAF_MAX_LEN {
                    small_indices.push(flat_index);
                } else {
                    large_indices.push(flat_index);
                }
                all_indices.push(flat_index);
                flat_index = flat_index.saturating_add(1);
            }
        }
    }
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    let mut duplicate_count = 0usize;
    let total_target = plan.small_target + plan.large_target;
    if total_target == 0 {
        return Ok(SampleSelection {
            indices: Vec::new(),
            duplicate_count: 0,
        });
    }
    draw_samples(
        &mut rng,
        plan.small_target,
        &small_indices,
        &all_indices,
        &mut seen,
        &mut selected,
        &mut duplicate_count,
    );
    draw_samples(
        &mut rng,
        plan.large_target,
        &large_indices,
        &all_indices,
        &mut seen,
        &mut selected,
        &mut duplicate_count,
    );
    if selected.is_empty() {
        return Err(PorChallengePlannerError::EmptyMerkleTree);
    }
    selected.sort_unstable();
    Ok(SampleSelection {
        indices: selected,
        duplicate_count,
    })
}
/// Construct a PoR challenge for the supplied manifest.
pub fn build_por_challenge_for_manifest(
    manifest: &StoredManifest,
    provider_id: [u8; 32],
    randomness: &PorRandomness,
    vrf: Option<&ManifestVrfBundle>,
    policy: &PorSamplePolicy,
    allow_forced: bool,
) -> Result<PlannedChallenge, PorChallengePlannerError> {
    if randomness.drand_signature.iter().all(|byte| *byte == 0) {
        return Err(PorChallengePlannerError::InvalidDrandSignature);
    }
    let chunk_profile = manifest.chunk_profile_handle();
    if chunk_profile.is_empty() {
        return Err(PorChallengePlannerError::EmptyChunkProfile);
    }
    let multiplier = policy.multiplier_for(chunk_profile);
    let plan = determine_sample_plan(manifest.content_length(), multiplier);
    let manifest_digest = *manifest.manifest_digest();
    let (vrf_output, vrf_proof, forced) = match vrf {
        Some(bundle)
            if bundle.provider_id == provider_id
                && bundle.manifest_digest == manifest_digest
                && bundle.epoch_id == randomness.epoch_id
                && bundle.drand_round == randomness.drand_round =>
        {
            (Some(bundle.output), Some(bundle.proof), false)
        }
        Some(_) => return Err(PorChallengePlannerError::VrfBindingMismatch),
        None if allow_forced => (None, None, true),
        None => {
            return Err(PorChallengePlannerError::MissingVrfBeforeDeadline {
                manifest_hex: hex::encode(manifest_digest),
            });
        }
    };
    let seed = derive_challenge_seed(
        &randomness.drand_randomness,
        vrf_output.as_ref(),
        &manifest_digest,
        randomness.epoch_id,
    );
    let selection = sample_leaf_indices(manifest.por_tree_ref(), seed, plan)?;
    let sample_count_usize = selection.indices.len();
    let sample_count = u16::try_from(sample_count_usize)
        .map_err(|_| PorChallengePlannerError::SampleCountOverflow(sample_count_usize))?;
    let challenge_id = derive_challenge_id(
        &seed,
        &manifest_digest,
        &provider_id,
        randomness.epoch_id,
        randomness.drand_round,
    );
    let deadline_at = randomness
        .issued_at_unix
        .saturating_add(randomness.response_window_secs);
    let challenge = PorChallengeV1 {
        version: sorafs_manifest::por::POR_CHALLENGE_VERSION_V1,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id: randomness.epoch_id,
        drand_round: randomness.drand_round,
        drand_randomness: randomness.drand_randomness,
        drand_signature: randomness.drand_signature,
        vrf_output,
        vrf_proof,
        forced,
        chunking_profile: chunk_profile.to_owned(),
        seed,
        sample_tier: plan.tier,
        sample_count,
        sample_indices: selection.indices.clone(),
        issued_at: randomness.issued_at_unix,
        deadline_at,
    };
    challenge.validate()?;
    Ok(PlannedChallenge {
        challenge,
        duplicate_samples: selection.duplicate_count,
    })
}
/// Statistics extracted from an audit verdict.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct PorVerdictStats {
    /// Number of successful samples recorded by the verdict.
    pub success_samples: u64,
    /// Number of failed samples recorded by the verdict.
    pub failed_samples: u64,
}
/// Runtime counters for challenge randomness and accepted-proof latency.
///
/// The counters are process-local telemetry and deliberately do not participate
/// in replay-protection checkpoints. Durable challenge/proof state remains the
/// source of truth after restart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PorProtocolMetricsSnapshot {
    /// Distinct valid challenges admitted by this process.
    pub challenges_total: u64,
    /// Distinct challenges carrying a provider VRF output and proof.
    pub vrf_challenges: u64,
    /// Distinct forced challenges admitted without a provider VRF.
    pub forced_challenges: u64,
    /// Challenge seed and challenge-id bindings successfully validated.
    pub seed_bindings_validated: u64,
    /// Challenge submissions rejected for an invalid seed or challenge-id binding.
    pub seed_binding_failures: u64,
    /// Distinct provider proofs accepted by this process.
    pub proofs_accepted: u64,
    /// Number of accepted proof-latency observations.
    pub proof_latency_samples: u64,
    /// Sum of accepted proof latency in milliseconds.
    pub proof_latency_total_ms: u64,
    /// Maximum accepted proof latency in milliseconds.
    pub proof_latency_max_ms: u64,
}
#[derive(Debug, Default)]
struct PorProtocolMetrics {
    challenges_total: AtomicU64,
    vrf_challenges: AtomicU64,
    forced_challenges: AtomicU64,
    seed_bindings_validated: AtomicU64,
    seed_binding_failures: AtomicU64,
    proofs_accepted: AtomicU64,
    proof_latency_samples: AtomicU64,
    proof_latency_total_ms: AtomicU64,
    proof_latency_max_ms: AtomicU64,
}
impl PorProtocolMetrics {
    fn snapshot(&self) -> PorProtocolMetricsSnapshot {
        PorProtocolMetricsSnapshot {
            challenges_total: self.challenges_total.load(Ordering::Relaxed),
            vrf_challenges: self.vrf_challenges.load(Ordering::Relaxed),
            forced_challenges: self.forced_challenges.load(Ordering::Relaxed),
            seed_bindings_validated: self.seed_bindings_validated.load(Ordering::Relaxed),
            seed_binding_failures: self.seed_binding_failures.load(Ordering::Relaxed),
            proofs_accepted: self.proofs_accepted.load(Ordering::Relaxed),
            proof_latency_samples: self.proof_latency_samples.load(Ordering::Relaxed),
            proof_latency_total_ms: self.proof_latency_total_ms.load(Ordering::Relaxed),
            proof_latency_max_ms: self.proof_latency_max_ms.load(Ordering::Relaxed),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ChallengeState {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
    proof_submitted_at: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct FinalizedChallengeStateV1 {
    state: ChallengeState,
    verdict: AuditVerdictV1,
    stats: PorVerdictStats,
    repair_task_id: Option<[u8; 32]>,
    repair_handoff_acknowledged: bool,
    reputation_sequence: u64,
    reputation_terminal: PorTerminalOutcomeV1,
}
impl ChallengeState {
    fn to_status(&self) -> PorChallengeStatusV1 {
        PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: self.challenge.challenge_id,
            manifest_digest: self.challenge.manifest_digest,
            provider_id: self.challenge.provider_id,
            epoch_id: self.challenge.epoch_id,
            drand_round: self.challenge.drand_round,
            status: if self.proof_digest.is_some() {
                PorChallengeOutcome::ProofSubmitted
            } else {
                PorChallengeOutcome::AwaitingProof
            },
            sample_count: self.challenge.sample_count,
            forced: self.challenge.forced,
            issued_at: self.challenge.issued_at,
            responded_at: self.proof_submitted_at,
            proof_digest: self.proof_digest,
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: None,
        }
    }
}
impl FinalizedChallengeStateV1 {
    fn to_status(&self) -> PorChallengeStatusV1 {
        let mut status = self.state.to_status();
        status.status = match self.verdict.outcome {
            AuditOutcomeV1::Success => PorChallengeOutcome::Verified,
            AuditOutcomeV1::Failed => PorChallengeOutcome::Failed,
            AuditOutcomeV1::Repaired => PorChallengeOutcome::Repaired,
        };
        status.repair_task_id = self.repair_task_id;
        if self.verdict.outcome != AuditOutcomeV1::Success {
            status
                .failure_reason
                .clone_from(&self.verdict.failure_reason);
        }
        status
    }
    fn pending_repair_work(&self) -> Result<Option<PorPendingRepairWorkV1>, PorTrackerError> {
        let Some(repair_task_id) = self.repair_task_id else {
            return Ok(None);
        };
        if self.repair_handoff_acknowledged {
            return Ok(None);
        }
        let failed_samples = u16::try_from(self.stats.failed_samples)
            .map_err(|_| PorTrackerError::InvalidFailedSampleCount)?;
        let intent = PorFailedRepairIntentV1 {
            manifest_digest: self.verdict.manifest_digest,
            provider_id: self.verdict.provider_id,
            challenge_id: self.verdict.challenge_id,
            failed_samples,
            proof_digest: self.verdict.proof_digest,
            decided_at_unix: self.verdict.decided_at,
        };
        intent
            .validate()
            .map_err(PorTrackerError::RepairIntentInvalid)?;
        if intent.repair_task_id() != repair_task_id {
            return Err(PorTrackerError::RepairTaskIdMismatch);
        }
        Ok(Some(PorPendingRepairWorkV1 {
            intent,
            repair_task_id,
        }))
    }
}
/// Pinned identity and verification policy for a finalized-PoR replay archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PorFinalizedReplayArchiveBindingV1 {
    /// Stable deployment-owned archive identity.
    pub archive_id: [u8; 32],
    /// Non-zero adapter/policy revision.
    pub revision: u64,
    /// Exact public archive policy digest.
    pub policy_digest: [u8; 32],
    /// Ed25519 public key authenticating append receipts and readbacks.
    pub signing_public_key: [u8; 32],
}
impl PorFinalizedReplayArchiveBindingV1 {
    /// Construct and validate an exact deployment-owned archive binding.
    ///
    /// # Errors
    ///
    /// Rejects zero identity, revision, or policy material and non-canonical
    /// or weak Ed25519 verification keys.
    pub fn try_new(
        archive_id: [u8; 32],
        revision: u64,
        policy_digest: [u8; 32],
        signing_public_key: [u8; 32],
    ) -> Result<Self, PorTrackerError> {
        let binding = Self {
            archive_id,
            revision,
            policy_digest,
            signing_public_key,
        };
        binding.verifying_key()?;
        Ok(binding)
    }
    fn verifying_key(self) -> Result<VerifyingKey, PorTrackerError> {
        if self.archive_id == [0; 32] || self.revision == 0 || self.policy_digest == [0; 32] {
            return Err(PorTrackerError::InvalidReplayArchiveBinding);
        }
        let key = VerifyingKey::from_bytes(&self.signing_public_key)
            .map_err(|_| PorTrackerError::InvalidReplayArchiveBinding)?;
        if key.to_bytes() != self.signing_public_key || key.is_weak() {
            return Err(PorTrackerError::InvalidReplayArchiveBinding);
        }
        Ok(key)
    }
}
/// Canonical source record persisted by the authenticated replay archive.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PorFinalizedReplayArchiveRecordV1 {
    finalized: FinalizedChallengeStateV1,
}
impl PorFinalizedReplayArchiveRecordV1 {
    fn from_finalized(finalized: FinalizedChallengeStateV1) -> Self {
        Self { finalized }
    }
    /// Finalization-order sequence bound by this record.
    #[must_use]
    pub const fn reputation_sequence(&self) -> u64 {
        self.finalized.reputation_sequence
    }
    /// Challenge identity bound by this record.
    #[must_use]
    pub const fn challenge_id(&self) -> [u8; 32] {
        self.finalized.state.challenge.challenge_id
    }
    /// Return the exact retained PoR-to-reputation work.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical terminal encoding fails.
    pub fn reputation_work(&self) -> Result<PorReputationTerminalWorkV1, PorTrackerError> {
        retained_reputation_work(&self.finalized)
    }
    /// Verify all retained challenge, verdict, repair, terminal, and digest invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when any retained source field is malformed,
    /// unauthenticated, or inconsistent with its canonical projection.
    pub fn validate(&self) -> Result<(), PorTrackerError> {
        validate_replay_archive_record(self)
    }
    /// Canonical domain-separated record digest.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding cannot be produced.
    pub fn record_digest(&self) -> Result<[u8; 32], PorTrackerError> {
        let bytes =
            norito::to_bytes(self).map_err(|_| PorTrackerError::ReplayArchiveCanonicalEncoding)?;
        let len = u64::try_from(bytes.len())
            .map_err(|_| PorTrackerError::ReplayArchiveCanonicalEncoding)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_FINALIZED_REPLAY_ARCHIVE_RECORD_DIGEST_DOMAIN_V1);
        hasher.update(&len.to_le_bytes());
        hasher.update(&bytes);
        Ok(*hasher.finalize().as_bytes())
    }
}
/// Provider-authenticated receipt for one replay-archive append.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PorFinalizedReplayArchiveReceiptV1 {
    binding: PorFinalizedReplayArchiveBindingV1,
    reputation_sequence: u64,
    challenge_id: [u8; 32],
    record_digest: [u8; 32],
    reputation_work_digest: [u8; 32],
    previous_head_digest: Option<[u8; 32]>,
    head_digest: [u8; 32],
    signature: [u8; 64],
}
impl PorFinalizedReplayArchiveReceiptV1 {
    /// Derive the exact digest an external archive signer must authenticate.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid binding or canonical record.
    pub fn signing_digest(
        binding: PorFinalizedReplayArchiveBindingV1,
        record: &PorFinalizedReplayArchiveRecordV1,
        previous_head_digest: Option<[u8; 32]>,
    ) -> Result<[u8; 32], PorTrackerError> {
        binding.verifying_key()?;
        record.validate()?;
        if previous_head_digest == Some([0; 32]) {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        let record_digest = record.record_digest()?;
        let reputation_work_digest = record.reputation_work()?.work_digest;
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_FINALIZED_REPLAY_ARCHIVE_HEAD_DIGEST_DOMAIN_V1);
        hasher.update(&binding.archive_id);
        hasher.update(&binding.revision.to_le_bytes());
        hasher.update(&binding.policy_digest);
        hasher.update(&binding.signing_public_key);
        hasher.update(&record.reputation_sequence().to_le_bytes());
        hasher.update(&record.challenge_id());
        hasher.update(&record_digest);
        hasher.update(&reputation_work_digest);
        match previous_head_digest {
            Some(previous) => {
                hasher.update(&[1]);
                hasher.update(&previous);
            }
            None => {
                hasher.update(&[0]);
            }
        }
        Ok(*hasher.finalize().as_bytes())
    }
    /// Construct and verify an archive receipt from an external signature.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid binding, record, chain link, or signature.
    pub fn try_new(
        binding: PorFinalizedReplayArchiveBindingV1,
        record: &PorFinalizedReplayArchiveRecordV1,
        previous_head_digest: Option<[u8; 32]>,
        signature: [u8; 64],
    ) -> Result<Self, PorTrackerError> {
        let head_digest = Self::signing_digest(binding, record, previous_head_digest)?;
        binding
            .verifying_key()?
            .verify_strict(&head_digest, &Signature::from_bytes(&signature))
            .map_err(|_| PorTrackerError::InvalidReplayArchiveReceipt)?;
        Ok(Self {
            binding,
            reputation_sequence: record.reputation_sequence(),
            challenge_id: record.challenge_id(),
            record_digest: record.record_digest()?,
            reputation_work_digest: record.reputation_work()?.work_digest,
            previous_head_digest,
            head_digest,
            signature,
        })
    }
    /// Verify the canonical fields and provider signature carried by this receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the binding, chain fields, digest, or signature is
    /// malformed or unauthenticated.
    pub fn validate(self) -> Result<(), PorTrackerError> {
        let key = self.binding.verifying_key()?;
        if self.reputation_sequence == 0
            || self.challenge_id == [0; 32]
            || self.record_digest == [0; 32]
            || self.reputation_work_digest == [0; 32]
            || self.previous_head_digest == Some([0; 32])
            || self.head_digest == [0; 32]
        {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_FINALIZED_REPLAY_ARCHIVE_HEAD_DIGEST_DOMAIN_V1);
        hasher.update(&self.binding.archive_id);
        hasher.update(&self.binding.revision.to_le_bytes());
        hasher.update(&self.binding.policy_digest);
        hasher.update(&self.binding.signing_public_key);
        hasher.update(&self.reputation_sequence.to_le_bytes());
        hasher.update(&self.challenge_id);
        hasher.update(&self.record_digest);
        hasher.update(&self.reputation_work_digest);
        match self.previous_head_digest {
            Some(previous) => {
                hasher.update(&[1]);
                hasher.update(&previous);
            }
            None => {
                hasher.update(&[0]);
            }
        }
        if self.head_digest != *hasher.finalize().as_bytes() {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        key.verify_strict(&self.head_digest, &Signature::from_bytes(&self.signature))
            .map_err(|_| PorTrackerError::InvalidReplayArchiveReceipt)
    }
    /// Verify that this receipt authenticates one exact canonical record.
    ///
    /// `expected_previous_head` is an outer option so callers can either enforce an exact
    /// predecessor (`Some`) or validate the receipt without constraining its predecessor (`None`).
    ///
    /// # Errors
    ///
    /// Returns an error for a substituted binding, record, predecessor, digest, or signature.
    pub fn validate_record(
        self,
        expected_binding: PorFinalizedReplayArchiveBindingV1,
        record: &PorFinalizedReplayArchiveRecordV1,
        expected_previous_head: Option<Option<[u8; 32]>>,
    ) -> Result<(), PorTrackerError> {
        if self.binding != expected_binding
            || self.reputation_sequence != record.reputation_sequence()
            || self.challenge_id != record.challenge_id()
            || self.record_digest != record.record_digest()?
            || self.reputation_work_digest != record.reputation_work()?.work_digest
            || expected_previous_head.is_some_and(|expected| expected != self.previous_head_digest)
            || self.head_digest
                != Self::signing_digest(self.binding, record, self.previous_head_digest)?
        {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        self.validate()
    }
    /// Exact archive binding authenticated by this receipt.
    #[must_use]
    pub const fn binding(self) -> PorFinalizedReplayArchiveBindingV1 {
        self.binding
    }
    /// Finalization-order sequence authenticated by this receipt.
    #[must_use]
    pub const fn reputation_sequence(self) -> u64 {
        self.reputation_sequence
    }
    /// Challenge identity authenticated by this receipt.
    #[must_use]
    pub const fn challenge_id(self) -> [u8; 32] {
        self.challenge_id
    }
    /// Canonical record digest authenticated by this receipt.
    #[must_use]
    pub const fn record_digest(self) -> [u8; 32] {
        self.record_digest
    }
    /// Canonical retained-work digest authenticated by this receipt.
    #[must_use]
    pub const fn reputation_work_digest(self) -> [u8; 32] {
        self.reputation_work_digest
    }
    /// Exact predecessor head authenticated by this receipt.
    #[must_use]
    pub const fn previous_head_digest(self) -> Option<[u8; 32]> {
        self.previous_head_digest
    }
    /// Signed archive head digest.
    #[must_use]
    pub const fn head_digest(self) -> [u8; 32] {
        self.head_digest
    }
    /// Ed25519 signature authenticating the receipt head digest.
    #[must_use]
    pub const fn signature(self) -> [u8; 64] {
        self.signature
    }
}
/// Exact resource bounds for one authenticated replay-archive lookup proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorFinalizedReplayArchiveProofBoundsV1 {
    max_successor_receipts: usize,
    max_successor_proof_bytes: u64,
}
impl PorFinalizedReplayArchiveProofBoundsV1 {
    /// Construct non-zero count and canonical-byte bounds.
    ///
    /// # Errors
    ///
    /// Returns an error when either bound is zero or the count cannot be
    /// represented on this platform.
    pub fn try_new(
        max_successor_receipts: u32,
        max_successor_proof_bytes: u64,
    ) -> Result<Self, PorTrackerError> {
        if max_successor_receipts == 0 || max_successor_proof_bytes == 0 {
            return Err(PorTrackerError::ReplayArchiveProofLimitExceeded);
        }
        Ok(Self {
            max_successor_receipts: usize::try_from(max_successor_receipts)
                .map_err(|_| PorTrackerError::ReplayArchiveProofLimitExceeded)?,
            max_successor_proof_bytes,
        })
    }
    /// Maximum signed successor receipts accepted in one proof.
    #[must_use]
    pub const fn max_successor_receipts(self) -> usize {
        self.max_successor_receipts
    }
    /// Maximum canonical successor-proof bytes accepted in one proof.
    #[must_use]
    pub const fn max_successor_proof_bytes(self) -> u64 {
        self.max_successor_proof_bytes
    }
    /// Qualify an outer transport frame before decoding its successor receipts.
    ///
    /// Production adapters should call this with the authenticated or length-prefixed receipt count
    /// and frame length before reserving or decoding the successor collection. The typed lookup
    /// boundary still revalidates the canonical decoded representation.
    ///
    /// # Errors
    ///
    /// Returns an error when the declared count is not representable, a non-empty collection
    /// declares an empty frame, or either configured resource ceiling is exceeded.
    pub fn validate_framed_successor_shape(
        self,
        declared_successor_receipts: u64,
        framed_successor_bytes: u64,
    ) -> Result<(), PorTrackerError> {
        let declared_successor_receipts = usize::try_from(declared_successor_receipts)
            .map_err(|_| PorTrackerError::ReplayArchiveProofLimitExceeded)?;
        if declared_successor_receipts > self.max_successor_receipts
            || framed_successor_bytes > self.max_successor_proof_bytes
            || (declared_successor_receipts != 0 && framed_successor_bytes == 0)
        {
            return Err(PorTrackerError::ReplayArchiveProofLimitExceeded);
        }
        Ok(())
    }
    #[cfg(test)]
    fn production_default() -> Self {
        Self::try_new(
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::MAX_SUCCESSOR_RECEIPTS,
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::MAX_SUCCESSOR_PROOF_BYTES,
        )
        .expect("non-zero finalized PoR replay-archive proof defaults")
    }
}
/// Authenticated archive lookup result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorFinalizedReplayArchiveReadbackV1 {
    /// Exact canonical finalized record.
    pub record: PorFinalizedReplayArchiveRecordV1,
    /// Provider-authenticated receipt binding that record.
    pub receipt: PorFinalizedReplayArchiveReceiptV1,
    /// Signed contiguous successors proving inclusion at the pinned head.
    pub successor_receipts: Vec<PorFinalizedReplayArchiveReceiptV1>,
}
impl PorFinalizedReplayArchiveReadbackV1 {
    /// Verify this record and its bounded contiguous inclusion path at one exact head.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical proof bounds are exceeded or any record,
    /// receipt, chain link, binding, or final head is invalid.
    pub fn validate_at_checkpoint(
        &self,
        binding: PorFinalizedReplayArchiveBindingV1,
        checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<(), PorTrackerError> {
        if self.successor_receipts.len() > proof_bounds.max_successor_receipts {
            return Err(PorTrackerError::ReplayArchiveProofLimitExceeded);
        }
        let canonical_successors = norito::to_bytes(&self.successor_receipts)
            .map_err(|_| PorTrackerError::ReplayArchiveCanonicalEncoding)?;
        if u64::try_from(canonical_successors.len()).unwrap_or(u64::MAX)
            > proof_bounds.max_successor_proof_bytes
        {
            return Err(PorTrackerError::ReplayArchiveProofLimitExceeded);
        }
        checkpoint_head.validate()?;
        if checkpoint_head.binding != binding {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        self.receipt.validate_record(binding, &self.record, None)?;
        let mut previous = self.receipt;
        for successor in &self.successor_receipts {
            successor.validate()?;
            if successor.binding != binding
                || successor.previous_head_digest != Some(previous.head_digest)
                || successor.reputation_sequence
                    != previous
                        .reputation_sequence
                        .checked_add(1)
                        .ok_or(PorTrackerError::ReputationSequenceOverflow)?
            {
                return Err(PorTrackerError::InvalidReplayArchiveReceipt);
            }
            previous = *successor;
        }
        if previous != checkpoint_head {
            return Err(PorTrackerError::InvalidReplayArchiveReceipt);
        }
        Ok(())
    }
}
/// Provider-authenticated proof that one challenge is absent at an exact signed head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PorFinalizedReplayArchiveAbsenceProofV1 {
    binding: PorFinalizedReplayArchiveBindingV1,
    challenge_id: [u8; 32],
    checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
    signature: [u8; 64],
}
impl PorFinalizedReplayArchiveAbsenceProofV1 {
    /// Derive the exact digest an external archive signer must authenticate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, binding, or checkpoint head.
    pub fn signing_digest(
        binding: PorFinalizedReplayArchiveBindingV1,
        challenge_id: [u8; 32],
        checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
    ) -> Result<[u8; 32], PorTrackerError> {
        if challenge_id == [0; 32] || checkpoint_head.binding != binding {
            return Err(PorTrackerError::InvalidReplayArchiveAbsenceProof);
        }
        binding.verifying_key()?;
        checkpoint_head.validate()?;
        let head_bytes = norito::to_bytes(&checkpoint_head)
            .map_err(|_| PorTrackerError::ReplayArchiveCanonicalEncoding)?;
        let head_len = u64::try_from(head_bytes.len())
            .map_err(|_| PorTrackerError::ReplayArchiveCanonicalEncoding)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_FINALIZED_REPLAY_ARCHIVE_ABSENCE_DIGEST_DOMAIN_V1);
        hasher.update(&binding.archive_id);
        hasher.update(&binding.revision.to_le_bytes());
        hasher.update(&binding.policy_digest);
        hasher.update(&binding.signing_public_key);
        hasher.update(&challenge_id);
        hasher.update(&head_len.to_le_bytes());
        hasher.update(&head_bytes);
        Ok(*hasher.finalize().as_bytes())
    }
    /// Construct and verify a head-bound absence proof.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed or unauthenticated proof.
    pub fn try_new(
        binding: PorFinalizedReplayArchiveBindingV1,
        challenge_id: [u8; 32],
        checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
        signature: [u8; 64],
    ) -> Result<Self, PorTrackerError> {
        let digest = Self::signing_digest(binding, challenge_id, checkpoint_head)?;
        binding
            .verifying_key()?
            .verify_strict(&digest, &Signature::from_bytes(&signature))
            .map_err(|_| PorTrackerError::InvalidReplayArchiveAbsenceProof)?;
        Ok(Self {
            binding,
            challenge_id,
            checkpoint_head,
            signature,
        })
    }
    /// Exact archive binding authenticated by this proof.
    #[must_use]
    pub const fn binding(self) -> PorFinalizedReplayArchiveBindingV1 {
        self.binding
    }
    /// Challenge identity whose absence is authenticated.
    #[must_use]
    pub const fn challenge_id(self) -> [u8; 32] {
        self.challenge_id
    }
    /// Exact signed checkpoint head against which absence was proven.
    #[must_use]
    pub const fn checkpoint_head(self) -> PorFinalizedReplayArchiveReceiptV1 {
        self.checkpoint_head
    }
    /// Ed25519 signature authenticating this exact absence statement.
    #[must_use]
    pub const fn signature(self) -> [u8; 64] {
        self.signature
    }
    /// Verify this signed absence proof against one exact challenge and head.
    ///
    /// # Errors
    ///
    /// Returns an error when the proof is substituted, stale, malformed, or unauthenticated.
    pub fn validate_at_checkpoint(
        self,
        binding: PorFinalizedReplayArchiveBindingV1,
        challenge_id: [u8; 32],
        checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
    ) -> Result<(), PorTrackerError> {
        if self.binding != binding
            || self.challenge_id != challenge_id
            || self.checkpoint_head != checkpoint_head
        {
            return Err(PorTrackerError::InvalidReplayArchiveAbsenceProof);
        }
        let digest = Self::signing_digest(binding, challenge_id, checkpoint_head)?;
        binding
            .verifying_key()?
            .verify_strict(&digest, &Signature::from_bytes(&self.signature))
            .map_err(|_| PorTrackerError::InvalidReplayArchiveAbsenceProof)
    }
}
/// Authenticated result of an exact checkpoint-bound archive lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PorFinalizedReplayArchiveLookupV1 {
    /// The exact record and contiguous inclusion path were returned.
    Found(Box<PorFinalizedReplayArchiveReadbackV1>),
    /// Absence was signed against the exact requested checkpoint head.
    Absent(Box<PorFinalizedReplayArchiveAbsenceProofV1>),
}
/// Payload-free external replay-archive failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PorFinalizedReplayArchiveExternalErrorV1 {
    /// The archive could not provide authenticated service.
    #[error("finalized PoR replay archive is unavailable")]
    Unavailable,
    /// The archive rejected the exact request.
    #[error("finalized PoR replay archive rejected the request")]
    Rejected,
}
/// Deployment-injected authenticated finalized-PoR replay archive.
///
/// Append must be durable before success. Repeating the same record and expected predecessor must
/// return the exact same signed receipt even after later successors exist; it must never move the
/// current head backwards. Substituted material must fail. Lookups must authenticate the returned
/// record before it crosses this boundary.
pub trait PorFinalizedReplayArchiveV1: Send + Sync + std::fmt::Debug {
    /// Return the stable credential-free production adapter handle.
    fn runtime_handle(&self) -> &str;
    /// Return the exact live adapter identity and verification binding.
    fn binding(
        &self,
    ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1>;
    /// Prove the archive can authenticate its current monotonic head.
    ///
    /// This call must not mutate archive state. Adapters that cannot establish
    /// fresh authenticated read/write readiness must fail closed.
    fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1>;
    /// Return the current provider-authenticated monotonic head.
    fn current_head(
        &self,
    ) -> Result<Option<PorFinalizedReplayArchiveReceiptV1>, PorFinalizedReplayArchiveExternalErrorV1>;
    /// Durably append one exact record after the supplied signed head.
    fn append(
        &self,
        record: &PorFinalizedReplayArchiveRecordV1,
        expected_previous_head: Option<[u8; 32]>,
    ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1>;
    /// Return authenticated presence or absence for `challenge_id` at the
    /// caller's exact checkpoint head.
    ///
    /// A found record must include a signed contiguous successor chain ending at
    /// `expected_checkpoint_head`. Absence must carry an independent signature over that exact
    /// head. A transport-backed adapter must apply
    /// [`PorFinalizedReplayArchiveProofBoundsV1::validate_framed_successor_shape`] to its outer
    /// length/count envelope before allocating or decoding an untrusted successor collection. The
    /// typed result is validated again by the caller. An adapter unable to prove either property
    /// must return an external error.
    fn lookup(
        &self,
        challenge_id: [u8; 32],
        expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1>;
}
#[derive(Debug)]
struct PorTrackerState {
    pending: HashMap<[u8; 32], ChallengeState>,
    finalized: HashMap<[u8; 32], FinalizedChallengeStateV1>,
    compacted_statuses: HashMap<[u8; 32], PorChallengeStatusV1>,
    status_generation: u64,
    last_reputation_sequence: u64,
    acknowledged_reputation_terminal: Option<PorReputationTerminalAckV1>,
    replay_archive_receipt: Option<PorFinalizedReplayArchiveReceiptV1>,
    latest_status_removals: Vec<[u8; 32]>,
    entry_limit: usize,
}
impl Default for PorTrackerState {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            finalized: HashMap::new(),
            compacted_statuses: HashMap::new(),
            status_generation: 1,
            last_reputation_sequence: 0,
            acknowledged_reputation_terminal: None,
            replay_archive_receipt: None,
            latest_status_removals: Vec::new(),
            entry_limit: DEFAULT_TRACKER_ENTRY_LIMIT,
        }
    }
}
/// Canonical durable snapshot of PoR challenge replay-protection state.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct PorTrackerCheckpointV1 {
    pending: Vec<ChallengeState>,
    finalized: Vec<FinalizedChallengeStateV1>,
    compacted_statuses: Vec<PorChallengeStatusV1>,
    status_generation: u64,
    last_reputation_sequence: u64,
    acknowledged_reputation_terminal: Option<PorReputationTerminalAckV1>,
    replay_archive_receipt: Option<PorFinalizedReplayArchiveReceiptV1>,
}
/// Bounded authoritative PoR status snapshot exported by the node checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorStatusAuthoritySnapshotV1 {
    /// Non-zero generation advanced by every committed lifecycle mutation.
    pub generation: u64,
    /// Exact status history in strictly increasing challenge-id order.
    pub statuses: Vec<PorChallengeStatusV1>,
}
/// One node-authoritative PoR status record after a durable lifecycle operation.
///
/// The update is emitted while the node's auxiliary-checkpoint transaction is
/// still serialized. Torii can therefore advance its rebuildable indexes by one
/// exact generation without cloning or sorting the complete retained history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorStatusAuthorityUpdateV1 {
    /// Non-zero generation visible in the durable node checkpoint.
    pub generation: u64,
    /// Exact retained status affected by the lifecycle operation.
    pub status: PorChallengeStatusV1,
    /// Terminal statuses retired from the bounded local projection by the same durable generation.
    pub removed_challenge_ids: Vec<[u8; 32]>,
}
/// Durable effect of a failed PoR lifecycle mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PorMutationDispositionV1 {
    /// Validation or admission failed before authoritative state changed.
    NoMutation,
    /// An in-memory mutation was restored to its exact pre-call checkpoint.
    RolledBack,
    /// The durable checkpoint may contain the mutation, so reconciliation is required.
    CommitUncertain,
    /// Restoring the exact pre-call checkpoint failed.
    RollbackFailed,
}
impl PorMutationDispositionV1 {
    /// Whether a rebuildable Torii projection must be invalidated.
    #[must_use]
    pub const fn invalidates_projection(self) -> bool {
        matches!(self, Self::CommitUncertain | Self::RollbackFailed)
    }
}
/// Typed PoR mutation failure carrying its exact durable-state disposition.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct PorMutationFailureV1 {
    error: PorTrackerError,
    disposition: PorMutationDispositionV1,
}
impl PorMutationFailureV1 {
    pub(crate) const fn new(error: PorTrackerError, disposition: PorMutationDispositionV1) -> Self {
        Self { error, disposition }
    }
    /// Construct a failure that did not mutate authoritative state.
    #[must_use]
    pub const fn no_mutation(error: PorTrackerError) -> Self {
        Self::new(error, PorMutationDispositionV1::NoMutation)
    }
    /// Return the durable effect of the failed call.
    #[must_use]
    pub const fn disposition(&self) -> PorMutationDispositionV1 {
        self.disposition
    }
    /// Recover the original tracker error for compatibility-only callers.
    #[must_use]
    pub fn into_tracker_error(self) -> PorTrackerError {
        self.error
    }
}
/// One failed-verdict repair intent retained until its durable handoff is acknowledged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorPendingRepairWorkV1 {
    /// Canonical repair intent derived from the retained verdict.
    pub intent: PorFailedRepairIntentV1,
    /// Deterministic chain-authoritative repair task identity.
    pub repair_task_id: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(clippy::large_enum_variant, reason = "by-value replay contract")]
pub(crate) enum PorChallengeRecordOutcomeV1 {
    Inserted,
    ExactReplay(PorChallengeStatusV1),
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(clippy::large_enum_variant, reason = "by-value replay contract")]
pub(crate) enum PorProofRecordOutcomeV1 {
    Inserted,
    ExactReplay(PorChallengeStatusV1),
}
/// Outcome of durably acknowledging one failed-verdict repair handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PorRepairHandoffAckOutcomeV1 {
    /// Pending work was acknowledged by this call.
    Advanced,
    /// The exact acknowledgement was already retained.
    ExactReplay,
}
/// Result of reconciling one durable failed-verdict repair outbox entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PorRepairReconcileOutcomeV1 {
    /// No repair handoff remains pending.
    Idle,
    /// One exact repair handoff was admitted and durably acknowledged.
    Reconciled {
        /// Exact work retained by the authoritative PoR checkpoint.
        work: PorPendingRepairWorkV1,
        /// Durable acknowledgement result.
        acknowledgement: PorRepairHandoffAckOutcomeV1,
    },
}
/// Failure while reconciling the durable failed-verdict repair outbox.
#[derive(Debug, Error)]
pub enum PorRepairReconcileErrorV1 {
    /// Reading or acknowledging the node checkpoint failed.
    #[error("PoR repair checkpoint failure: {0}")]
    Tracker(#[from] PorTrackerError),
    /// The durable repair admission boundary rejected the exact work.
    #[error(transparent)]
    Handoff(#[from] PorRepairHandoffError),
    /// The repair boundary returned an identity other than the deterministic one.
    #[error("PoR repair handoff returned task {actual:?}; expected {expected:?}")]
    TaskIdMismatch {
        /// Deterministic identity retained in the node checkpoint.
        expected: [u8; 32],
        /// Identity returned by the repair admission boundary.
        actual: [u8; 32],
    },
}
#[cfg(test)]
impl PorTrackerCheckpointV1 {
    pub(crate) fn has_no_finalized_challenges(&self) -> bool {
        self.finalized.is_empty()
    }
    pub(crate) const fn replay_archive_receipt(
        &self,
    ) -> Option<PorFinalizedReplayArchiveReceiptV1> {
        self.replay_archive_receipt
    }
}
/// One canonical, replay-stable PoR terminal awaiting reputation admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorReputationTerminalWorkV1 {
    /// Strictly monotonic node-local delivery sequence.
    pub sequence: u64,
    /// Provider attributed by the retained authenticated challenge.
    pub provider_id: [u8; 32],
    /// Canonical terminal projection retained with finalized PoR state.
    pub terminal: PorTerminalOutcomeV1,
    /// Domain-separated digest binding the sequence, provider, and terminal.
    pub work_digest: [u8; 32],
}
impl PorReputationTerminalWorkV1 {
    fn try_new(
        sequence: u64,
        provider_id: [u8; 32],
        terminal: PorTerminalOutcomeV1,
    ) -> Result<Self, PorTrackerError> {
        if sequence == 0 || provider_id == [0; 32] {
            return Err(PorTrackerError::InvalidReputationTerminalWork);
        }
        let terminal_bytes = norito::to_bytes(&terminal)
            .map_err(|_| PorTrackerError::ReputationTerminalCanonicalEncoding)?;
        let terminal_len = u64::try_from(terminal_bytes.len())
            .map_err(|_| PorTrackerError::ReputationTerminalCanonicalEncoding)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_REPUTATION_TERMINAL_WORK_DIGEST_DOMAIN_V1);
        hasher.update(&sequence.to_le_bytes());
        hasher.update(&provider_id);
        hasher.update(&terminal_len.to_le_bytes());
        hasher.update(&terminal_bytes);
        Ok(Self {
            sequence,
            provider_id,
            terminal,
            work_digest: *hasher.finalize().as_bytes(),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PorReputationTerminalAckV1 {
    sequence: u64,
    work_digest: [u8; 32],
}
/// Outcome of advancing the durable PoR-to-reputation delivery cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PorReputationTerminalAckOutcomeV1 {
    /// The exact next terminal advanced the tracker acknowledgement cursor.
    Advanced,
    /// The exact terminal acknowledgement was already retained.
    ExactReplay,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PorVerdictTransitionV1 {
    pub(crate) stats: PorVerdictStats,
    pub(crate) repair_task_id: Option<[u8; 32]>,
    pub(crate) reputation_work: PorReputationTerminalWorkV1,
    pub(crate) authority_status: PorChallengeStatusV1,
    pub(crate) newly_finalized: bool,
}
/// Tracks the lifecycle of PoR challenges, proofs, and verdicts.
#[derive(Debug, Clone)]
pub struct PorTracker {
    inner: Arc<RwLock<PorTrackerState>>,
    metrics: Arc<PorProtocolMetrics>,
}
impl Default for PorTracker {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PorTrackerState::default())),
            metrics: Arc::new(PorProtocolMetrics::default()),
        }
    }
}
impl PorTracker {
    /// Construct a tracker with a hard ceiling for pending and finalized entries.
    #[must_use]
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PorTrackerState {
                entry_limit: entry_limit.clamp(1, DEFAULT_TRACKER_ENTRY_LIMIT),
                ..PorTrackerState::default()
            })),
            metrics: Arc::new(PorProtocolMetrics::default()),
        }
    }
    fn next_status_generation(state: &PorTrackerState) -> Result<u64, PorTrackerError> {
        state
            .status_generation
            .checked_add(1)
            .ok_or(PorTrackerError::StatusGenerationExhausted)
    }
    fn validate_authority_status(status: &PorChallengeStatusV1) -> Result<(), PorTrackerError> {
        status
            .validate()
            .map_err(|error| PorTrackerError::InvalidAuthorityStatus(error.to_string()))
    }
    fn oldest_compacted_status_id(state: &PorTrackerState) -> Option<[u8; 32]> {
        state
            .compacted_statuses
            .values()
            .min_by_key(|status| (status.issued_at, status.challenge_id))
            .map(|status| status.challenge_id)
    }
    /// Register a new PoR challenge.
    pub(crate) fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<PorChallengeRecordOutcomeV1, PorTrackerError> {
        self.record_challenge_with_archive_option(challenge, None, None)
    }
    /// Register or replay a challenge with exact configured proof bounds.
    pub(crate) fn record_challenge_with_archive_and_bounds(
        &self,
        challenge: &PorChallengeV1,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorChallengeRecordOutcomeV1, PorTrackerError> {
        self.record_challenge_with_archive_option(
            challenge,
            Some(replay_archive),
            Some(proof_bounds),
        )
    }
    fn record_challenge_with_archive_option(
        &self,
        challenge: &PorChallengeV1,
        replay_archive: Option<&dyn PorFinalizedReplayArchiveV1>,
        proof_bounds: Option<PorFinalizedReplayArchiveProofBoundsV1>,
    ) -> Result<PorChallengeRecordOutcomeV1, PorTrackerError> {
        if let Err(error) = challenge.validate() {
            if matches!(
                error,
                PorChallengeValidationError::SeedMismatch
                    | PorChallengeValidationError::ChallengeIdMismatch
            ) {
                self.metrics
                    .seed_binding_failures
                    .fetch_add(1, Ordering::Relaxed);
            }
            return Err(PorTrackerError::ChallengeInvalid(error));
        }
        let mut state = self.inner.write().expect("por tracker poisoned");
        state.latest_status_removals.clear();
        if let Some(finalized) = state.finalized.get(&challenge.challenge_id) {
            return if finalized.state.challenge == *challenge {
                Ok(PorChallengeRecordOutcomeV1::ExactReplay(
                    finalized.to_status(),
                ))
            } else {
                Err(PorTrackerError::ChallengeConflict)
            };
        }
        if !state.pending.contains_key(&challenge.challenge_id)
            && let Some(latest_archive_receipt) = state.replay_archive_receipt
        {
            let replay_archive = replay_archive.ok_or(PorTrackerError::ReplayArchiveRequired)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            let binding = replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if binding != latest_archive_receipt.binding {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            let proof_bounds =
                proof_bounds.ok_or(PorTrackerError::ReplayArchiveProofLimitExceeded)?;
            let lookup = replay_archive
                .lookup(challenge.challenge_id, latest_archive_receipt, proof_bounds)
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?
                != binding
            {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            match lookup {
                PorFinalizedReplayArchiveLookupV1::Found(readback) => {
                    readback.validate_at_checkpoint(
                        binding,
                        latest_archive_receipt,
                        proof_bounds,
                    )?;
                    validate_replay_archive_record(&readback.record)?;
                    return if readback.record.finalized.state.challenge == *challenge {
                        Ok(PorChallengeRecordOutcomeV1::ExactReplay(
                            readback.record.finalized.to_status(),
                        ))
                    } else {
                        Err(PorTrackerError::ChallengeConflict)
                    };
                }
                PorFinalizedReplayArchiveLookupV1::Absent(absence) => {
                    absence.validate_at_checkpoint(
                        binding,
                        challenge.challenge_id,
                        latest_archive_receipt,
                    )?;
                }
            }
        }
        let inserted_state = ChallengeState {
            challenge: challenge.clone(),
            proof_digest: None,
            proof_submitted_at: None,
        };
        Self::validate_authority_status(&inserted_state.to_status())?;
        let retained_count = state
            .pending
            .len()
            .checked_add(state.finalized.len())
            .and_then(|count| count.checked_add(state.compacted_statuses.len()))
            .ok_or_else(|| {
                PorTrackerError::InvalidCheckpoint(
                    "PoR retained status count overflowed".to_owned(),
                )
            })?;
        let retired_status = (!state.pending.contains_key(&challenge.challenge_id)
            && retained_count >= state.entry_limit)
            .then(|| Self::oldest_compacted_status_id(&state))
            .flatten();
        if !state.pending.contains_key(&challenge.challenge_id)
            && retained_count >= state.entry_limit
            && retired_status.is_none()
        {
            return Err(PorTrackerError::PendingRetentionExhausted {
                limit: state.entry_limit,
            });
        }
        let next_generation = if state.pending.contains_key(&challenge.challenge_id) {
            None
        } else {
            Some(Self::next_status_generation(&state)?)
        };
        let replay_status = match state.pending.entry(challenge.challenge_id) {
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(inserted_state);
                None
            }
            std::collections::hash_map::Entry::Occupied(occupied) => {
                // Allow idempotent replays of the same challenge but reject mismatched payloads.
                if occupied.get().challenge == *challenge {
                    Some(occupied.get().to_status())
                } else {
                    return Err(PorTrackerError::ChallengeConflict);
                }
            }
        };
        if let Some(status) = replay_status {
            state.latest_status_removals.clear();
            return Ok(PorChallengeRecordOutcomeV1::ExactReplay(status));
        }
        state.latest_status_removals.clear();
        if let Some(retired_status) = retired_status {
            let removed = state.compacted_statuses.remove(&retired_status);
            debug_assert!(removed.is_some());
            state.latest_status_removals.push(retired_status);
        }
        state.status_generation =
            next_generation.expect("vacant PoR challenge advanced its generation");
        self.metrics
            .challenges_total
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .seed_bindings_validated
            .fetch_add(1, Ordering::Relaxed);
        if challenge.forced {
            self.metrics
                .forced_challenges
                .fetch_add(1, Ordering::Relaxed);
        } else {
            debug_assert!(
                challenge.vrf_output.is_some() && challenge.vrf_proof.is_some(),
                "validated non-forced PoR challenge must carry a VRF bundle"
            );
            self.metrics.vrf_challenges.fetch_add(1, Ordering::Relaxed);
        }
        Ok(PorChallengeRecordOutcomeV1::Inserted)
    }
    /// Register a PoR proof response authenticated by provider admission.
    pub(crate) fn record_proof(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<PorProofRecordOutcomeV1, PorTrackerError> {
        self.record_proof_with_archive_option(proof, admitted_provider_key, None, None)
    }
    /// Register or exactly replay a PoR proof with authenticated archive lookup.
    pub(crate) fn record_proof_with_archive_and_bounds(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorProofRecordOutcomeV1, PorTrackerError> {
        self.record_proof_with_archive_option(
            proof,
            admitted_provider_key,
            Some(replay_archive),
            Some(proof_bounds),
        )
    }
    fn record_proof_with_archive_option(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
        replay_archive: Option<&dyn PorFinalizedReplayArchiveV1>,
        proof_bounds: Option<PorFinalizedReplayArchiveProofBoundsV1>,
    ) -> Result<PorProofRecordOutcomeV1, PorTrackerError> {
        proof.validate().map_err(PorTrackerError::ProofInvalid)?;
        proof
            .verify_signature_for_provider(admitted_provider_key)
            .map_err(PorTrackerError::ProofSignatureInvalid)?;
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        tracker.latest_status_removals.clear();
        let proof_digest = proof.proof_digest();
        if let Some(state) = tracker.pending.get(&proof.challenge_id) {
            validate_proof_against_challenge(proof, state)?;
            if let Some(retained_digest) = state.proof_digest {
                if retained_digest == proof_digest
                    && state.proof_submitted_at == Some(proof.submitted_at)
                {
                    return Ok(PorProofRecordOutcomeV1::ExactReplay(state.to_status()));
                }
                return Err(PorTrackerError::DuplicateProof);
            }
            let mut prospective = state.clone();
            prospective.proof_digest = Some(proof_digest);
            prospective.proof_submitted_at = Some(proof.submitted_at);
            Self::validate_authority_status(&prospective.to_status())?;
            let next_generation = Self::next_status_generation(&tracker)?;
            let latency_ms = proof
                .submitted_at
                .saturating_sub(state.challenge.issued_at)
                .saturating_mul(1_000);
            tracker.pending.insert(proof.challenge_id, prospective);
            self.metrics.proofs_accepted.fetch_add(1, Ordering::Relaxed);
            self.metrics
                .proof_latency_samples
                .fetch_add(1, Ordering::Relaxed);
            self.metrics
                .proof_latency_total_ms
                .fetch_add(latency_ms, Ordering::Relaxed);
            self.metrics
                .proof_latency_max_ms
                .fetch_max(latency_ms, Ordering::Relaxed);
            tracker.status_generation = next_generation;
            return Ok(PorProofRecordOutcomeV1::Inserted);
        }
        if let Some(finalized) = tracker.finalized.get(&proof.challenge_id) {
            validate_proof_against_challenge(proof, &finalized.state)?;
            return if finalized.state.proof_digest == Some(proof_digest)
                && finalized.state.proof_submitted_at == Some(proof.submitted_at)
            {
                Ok(PorProofRecordOutcomeV1::ExactReplay(finalized.to_status()))
            } else {
                Err(PorTrackerError::DuplicateProof)
            };
        }
        if let Some(status) = tracker.compacted_statuses.get(&proof.challenge_id) {
            ensure_match(
                proof.manifest_digest,
                status.manifest_digest,
                PorTrackerError::MismatchManifest,
            )?;
            ensure_match(
                proof.provider_id,
                status.provider_id,
                PorTrackerError::MismatchProvider,
            )?;
            return if status.proof_digest == Some(proof_digest)
                && status.responded_at == Some(proof.submitted_at)
            {
                Ok(PorProofRecordOutcomeV1::ExactReplay(status.clone()))
            } else {
                Err(PorTrackerError::DuplicateProof)
            };
        }
        if let Some(latest_archive_receipt) = tracker.replay_archive_receipt {
            let replay_archive = replay_archive.ok_or(PorTrackerError::ReplayArchiveRequired)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            let binding = replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if binding != latest_archive_receipt.binding {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            let proof_bounds =
                proof_bounds.ok_or(PorTrackerError::ReplayArchiveProofLimitExceeded)?;
            let lookup = replay_archive
                .lookup(proof.challenge_id, latest_archive_receipt, proof_bounds)
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?
                != binding
            {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            match lookup {
                PorFinalizedReplayArchiveLookupV1::Found(readback) => {
                    readback.validate_at_checkpoint(
                        binding,
                        latest_archive_receipt,
                        proof_bounds,
                    )?;
                    validate_replay_archive_record(&readback.record)?;
                    let retained = &readback.record.finalized.state;
                    validate_proof_against_challenge(proof, retained)?;
                    return if retained.proof_digest == Some(proof_digest)
                        && retained.proof_submitted_at == Some(proof.submitted_at)
                    {
                        Ok(PorProofRecordOutcomeV1::ExactReplay(
                            readback.record.finalized.to_status(),
                        ))
                    } else {
                        Err(PorTrackerError::DuplicateProof)
                    };
                }
                PorFinalizedReplayArchiveLookupV1::Absent(absence) => {
                    absence.validate_at_checkpoint(
                        binding,
                        proof.challenge_id,
                        latest_archive_receipt,
                    )?;
                }
            }
        }
        Err(PorTrackerError::UnknownChallenge)
    }
    /// Return the current process-local PoR protocol telemetry snapshot.
    #[must_use]
    pub fn protocol_metrics(&self) -> PorProtocolMetricsSnapshot {
        self.metrics.snapshot()
    }
    /// Finalise a challenge using an audit verdict.
    #[cfg(test)]
    pub(crate) fn record_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<PorVerdictStats, PorTrackerError> {
        self.record_verdict_durable(verdict, trusted_auditor_keys, auditor_threshold)
            .map(|transition| transition.stats)
    }
    /// Finalise a challenge and retain deterministic repair work in the
    /// authoritative checkpoint for post-commit reconciliation.
    pub(crate) fn record_verdict_durable(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<PorVerdictTransitionV1, PorTrackerError> {
        self.record_verdict_with_archive_option(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            None,
            None,
        )
    }
    #[cfg(test)]
    fn record_verdict_with(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        _retired_precommit_handoff: impl FnOnce(
            &PorFailedRepairIntentV1,
        ) -> Result<[u8; 32], PorRepairHandoffError>,
    ) -> Result<PorVerdictTransitionV1, PorTrackerError> {
        self.record_verdict_durable(verdict, trusted_auditor_keys, auditor_threshold)
    }
    /// Finalise or replay a verdict against an authenticated compacted archive
    /// with exact configured proof bounds.
    ///
    /// The archive is consulted only when the challenge no longer exists in
    /// local pending/finalized state. Its live binding and signed readback must
    /// match the checkpoint-pinned archive before an exact replay is returned.
    pub(crate) fn record_verdict_durable_with_archive_and_bounds(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorVerdictTransitionV1, PorTrackerError> {
        self.record_verdict_with_archive_option(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            Some(replay_archive),
            Some(proof_bounds),
        )
    }
    #[cfg(test)]
    fn record_verdict_with_archive_and_bounds(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
        _retired_precommit_handoff: impl FnOnce(
            &PorFailedRepairIntentV1,
        ) -> Result<[u8; 32], PorRepairHandoffError>,
    ) -> Result<PorVerdictTransitionV1, PorTrackerError> {
        self.record_verdict_durable_with_archive_and_bounds(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            replay_archive,
            proof_bounds,
        )
    }
    fn record_verdict_with_archive_option(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        replay_archive: Option<&dyn PorFinalizedReplayArchiveV1>,
        proof_bounds: Option<PorFinalizedReplayArchiveProofBoundsV1>,
    ) -> Result<PorVerdictTransitionV1, PorTrackerError> {
        verdict
            .validate()
            .map_err(PorTrackerError::VerdictInvalid)?;
        verdict
            .verify_signatures_with_policy(trusted_auditor_keys, auditor_threshold)
            .map_err(PorTrackerError::VerdictSignatureInvalid)?;
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        tracker.latest_status_removals.clear();
        if let Some(finalized) = tracker.finalized.get(&verdict.challenge_id) {
            if finalized.verdict != *verdict {
                return Err(PorTrackerError::VerdictConflict);
            }
            return Ok(PorVerdictTransitionV1 {
                stats: finalized.stats,
                repair_task_id: finalized.repair_task_id,
                reputation_work: retained_reputation_work(finalized)?,
                authority_status: finalized.to_status(),
                newly_finalized: false,
            });
        }
        let next_generation = tracker
            .pending
            .contains_key(&verdict.challenge_id)
            .then(|| Self::next_status_generation(&tracker))
            .transpose()?;
        let Some(state) = tracker.pending.get(&verdict.challenge_id) else {
            let Some(latest_archive_receipt) = tracker.replay_archive_receipt else {
                return Err(PorTrackerError::UnknownChallenge);
            };
            let replay_archive = replay_archive.ok_or(PorTrackerError::ReplayArchiveRequired)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            let binding = replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if binding != latest_archive_receipt.binding {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            let proof_bounds =
                proof_bounds.ok_or(PorTrackerError::ReplayArchiveProofLimitExceeded)?;
            let lookup = replay_archive
                .lookup(verdict.challenge_id, latest_archive_receipt, proof_bounds)
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?
                != binding
            {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            let readback = match lookup {
                PorFinalizedReplayArchiveLookupV1::Found(readback) => readback,
                PorFinalizedReplayArchiveLookupV1::Absent(absence) => {
                    absence.validate_at_checkpoint(
                        binding,
                        verdict.challenge_id,
                        latest_archive_receipt,
                    )?;
                    return Err(PorTrackerError::UnknownChallenge);
                }
            };
            readback.validate_at_checkpoint(binding, latest_archive_receipt, proof_bounds)?;
            validate_replay_archive_record(&readback.record)?;
            if readback.record.finalized.verdict != *verdict {
                return Err(PorTrackerError::VerdictConflict);
            }
            return Ok(PorVerdictTransitionV1 {
                stats: readback.record.finalized.stats,
                repair_task_id: readback.record.finalized.repair_task_id,
                reputation_work: readback.record.reputation_work()?,
                authority_status: readback.record.finalized.to_status(),
                newly_finalized: false,
            });
        };
        let stats = validate_verdict_transition(state, verdict)?;
        if tracker.finalized.len() >= tracker.entry_limit {
            return Err(PorTrackerError::FinalizedRetentionExhausted {
                limit: tracker.entry_limit,
            });
        }
        let reputation_sequence = tracker
            .last_reputation_sequence
            .checked_add(1)
            .ok_or(PorTrackerError::ReputationSequenceOverflow)?;
        let failed_repair_intent = if verdict.outcome == AuditOutcomeV1::Failed {
            let failed_samples = u16::try_from(stats.failed_samples)
                .map_err(|_| PorTrackerError::InvalidFailedSampleCount)?;
            let intent = PorFailedRepairIntentV1 {
                manifest_digest: verdict.manifest_digest,
                provider_id: verdict.provider_id,
                challenge_id: verdict.challenge_id,
                failed_samples,
                proof_digest: verdict.proof_digest,
                decided_at_unix: verdict.decided_at,
            };
            intent
                .validate()
                .map_err(PorTrackerError::RepairIntentInvalid)?;
            Some(intent)
        } else {
            None
        };
        let expected_repair_task_id =
            failed_repair_intent.map(PorFailedRepairIntentV1::repair_task_id);
        // Projection and deterministic repair work are validated before the
        // authoritative checkpoint transition. External handoff occurs only
        // after this state is durable and is acknowledged by a later checkpoint.
        let reputation_terminal = por_reputation_terminal_from_retained_v1(
            state,
            verdict,
            stats,
            expected_repair_task_id,
        )?;
        let reputation_work = PorReputationTerminalWorkV1::try_new(
            reputation_sequence,
            state.challenge.provider_id,
            reputation_terminal,
        )?;
        let repair_task_id = expected_repair_task_id;
        let finalized = FinalizedChallengeStateV1 {
            state: state.clone(),
            verdict: verdict.clone(),
            stats,
            repair_task_id,
            repair_handoff_acknowledged: repair_task_id.is_none(),
            reputation_sequence,
            reputation_terminal,
        };
        let authority_status = finalized.to_status();
        Self::validate_authority_status(&authority_status)?;
        tracker
            .pending
            .remove(&verdict.challenge_id)
            .expect("validated PoR challenge must remain while write lock is held");
        tracker.finalized.insert(verdict.challenge_id, finalized);
        tracker.last_reputation_sequence = reputation_sequence;
        tracker.status_generation =
            next_generation.expect("new PoR verdict advanced its status generation");
        Ok(PorVerdictTransitionV1 {
            stats,
            repair_task_id,
            reputation_work,
            authority_status,
            newly_finalized: true,
        })
    }
    /// Export pending and finalized challenge state in deterministic order.
    pub(crate) fn checkpoint(&self) -> PorTrackerCheckpointV1 {
        let tracker = self.inner.read().expect("por tracker poisoned");
        let mut pending = tracker.pending.values().cloned().collect::<Vec<_>>();
        pending.sort_by_key(|state| state.challenge.challenge_id);
        let mut finalized = tracker.finalized.values().cloned().collect::<Vec<_>>();
        finalized.sort_by_key(|state| state.state.challenge.challenge_id);
        let mut compacted_statuses = tracker
            .compacted_statuses
            .values()
            .cloned()
            .collect::<Vec<_>>();
        compacted_statuses.sort_by_key(|status| status.challenge_id);
        PorTrackerCheckpointV1 {
            pending,
            finalized,
            compacted_statuses,
            status_generation: tracker.status_generation,
            last_reputation_sequence: tracker.last_reputation_sequence,
            acknowledged_reputation_terminal: tracker.acknowledged_reputation_terminal,
            replay_archive_receipt: tracker.replay_archive_receipt,
        }
    }
    /// Export the complete bounded status history owned by this tracker.
    pub(crate) fn status_authority_snapshot(
        &self,
    ) -> Result<PorStatusAuthoritySnapshotV1, PorTrackerError> {
        let tracker = self.inner.read().expect("por tracker poisoned");
        if tracker.status_generation == 0 {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status generation is zero".to_owned(),
            ));
        }
        let total = tracker
            .pending
            .len()
            .checked_add(tracker.finalized.len())
            .and_then(|value| value.checked_add(tracker.compacted_statuses.len()))
            .ok_or_else(|| {
                PorTrackerError::InvalidCheckpoint("PoR status history length overflow".to_owned())
            })?;
        if total > tracker.entry_limit {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status history exceeds its retention bound".to_owned(),
            ));
        }
        let mut statuses = Vec::with_capacity(total);
        statuses.extend(tracker.pending.values().map(ChallengeState::to_status));
        statuses.extend(
            tracker
                .finalized
                .values()
                .map(FinalizedChallengeStateV1::to_status),
        );
        statuses.extend(tracker.compacted_statuses.values().cloned());
        statuses.sort_by_key(|status| status.challenge_id);
        if statuses
            .windows(2)
            .any(|pair| pair[0].challenge_id >= pair[1].challenge_id)
        {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status history contains duplicate or unordered challenge ids".to_owned(),
            ));
        }
        for status in &statuses {
            status.validate().map_err(|error| {
                PorTrackerError::InvalidCheckpoint(format!(
                    "invalid authoritative PoR status projection: {error}"
                ))
            })?;
        }
        Ok(PorStatusAuthoritySnapshotV1 {
            generation: tracker.status_generation,
            statuses,
        })
    }
    /// Return one exact status and generation without materializing history.
    pub(crate) fn status_authority_update(
        &self,
        challenge_id: [u8; 32],
    ) -> Result<PorStatusAuthorityUpdateV1, PorTrackerError> {
        let tracker = self.inner.read().expect("por tracker poisoned");
        if tracker.status_generation == 0 {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status generation is zero".to_owned(),
            ));
        }
        let pending = tracker
            .pending
            .get(&challenge_id)
            .map(ChallengeState::to_status);
        let finalized = tracker
            .finalized
            .get(&challenge_id)
            .map(FinalizedChallengeStateV1::to_status);
        let compacted = tracker.compacted_statuses.get(&challenge_id).cloned();
        let retained_copies = usize::from(pending.is_some())
            .checked_add(usize::from(finalized.is_some()))
            .and_then(|count| count.checked_add(usize::from(compacted.is_some())))
            .ok_or_else(|| {
                PorTrackerError::InvalidCheckpoint(
                    "PoR status ownership count overflowed".to_owned(),
                )
            })?;
        if retained_copies != 1 {
            return if retained_copies == 0 {
                Err(PorTrackerError::UnknownChallenge)
            } else {
                Err(PorTrackerError::InvalidCheckpoint(
                    "PoR status exists in multiple lifecycle stores".to_owned(),
                ))
            };
        }
        let status = pending
            .or(finalized)
            .or(compacted)
            .expect("exactly one retained PoR status was counted");
        status.validate().map_err(|error| {
            PorTrackerError::InvalidCheckpoint(format!(
                "invalid authoritative PoR status update: {error}"
            ))
        })?;
        Ok(PorStatusAuthorityUpdateV1 {
            generation: tracker.status_generation,
            status,
            removed_challenge_ids: tracker.latest_status_removals.clone(),
        })
    }
    /// Build a same-generation no-op projection update for an exact replay.
    ///
    /// A replay may name a status still retained locally or a terminal whose
    /// full source record is authenticated by the checkpoint-pinned archive
    /// after rolling projection retention retired it.
    pub(crate) fn status_authority_replay_update(
        &self,
        status: PorChallengeStatusV1,
    ) -> Result<PorStatusAuthorityUpdateV1, PorTrackerError> {
        Self::validate_authority_status(&status)?;
        let tracker = self.inner.read().expect("por tracker poisoned");
        if tracker.status_generation == 0 {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status generation is zero".to_owned(),
            ));
        }
        let retained = tracker
            .pending
            .get(&status.challenge_id)
            .map(ChallengeState::to_status)
            .or_else(|| {
                tracker
                    .finalized
                    .get(&status.challenge_id)
                    .map(FinalizedChallengeStateV1::to_status)
            })
            .or_else(|| {
                tracker
                    .compacted_statuses
                    .get(&status.challenge_id)
                    .cloned()
            });
        match retained {
            Some(retained) if retained != status => {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "exact PoR replay status differs from retained authority".to_owned(),
                ));
            }
            Some(_) => {}
            None => {
                if tracker.replay_archive_receipt.is_none()
                    || matches!(
                        status.status,
                        PorChallengeOutcome::AwaitingProof | PorChallengeOutcome::ProofSubmitted
                    )
                {
                    return Err(PorTrackerError::InvalidCheckpoint(
                        "exact PoR replay is absent from retained or archived terminal authority"
                            .to_owned(),
                    ));
                }
            }
        }
        Ok(PorStatusAuthorityUpdateV1 {
            generation: tracker.status_generation,
            status,
            removed_challenge_ids: Vec::new(),
        })
    }
    /// Return the oldest failed-verdict repair intent not yet acknowledged.
    pub(crate) fn next_pending_repair_work(
        &self,
    ) -> Result<Option<PorPendingRepairWorkV1>, PorTrackerError> {
        let tracker = self.inner.read().expect("por tracker poisoned");
        let mut pending = tracker
            .finalized
            .values()
            .filter(|state| state.repair_task_id.is_some() && !state.repair_handoff_acknowledged)
            .collect::<Vec<_>>();
        pending.sort_by_key(|state| state.reputation_sequence);
        pending
            .first()
            .map_or(Ok(None), |state| state.pending_repair_work())
    }
    /// Acknowledge the exact retained failed-verdict repair handoff.
    pub(crate) fn acknowledge_repair_handoff(
        &self,
        challenge_id: [u8; 32],
        repair_task_id: [u8; 32],
    ) -> Result<PorRepairHandoffAckOutcomeV1, PorTrackerError> {
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        let finalized = tracker
            .finalized
            .get_mut(&challenge_id)
            .ok_or(PorTrackerError::UnknownChallenge)?;
        if finalized.repair_task_id != Some(repair_task_id) {
            return Err(PorTrackerError::RepairTaskIdMismatch);
        }
        if finalized.repair_handoff_acknowledged {
            return Ok(PorRepairHandoffAckOutcomeV1::ExactReplay);
        }
        finalized.repair_handoff_acknowledged = true;
        Ok(PorRepairHandoffAckOutcomeV1::Advanced)
    }
    /// Return the exact next retained PoR terminal awaiting reputation admission.
    ///
    /// Work is exposed strictly in finalization order. The same item is
    /// returned until its sequence and binding digest are acknowledged.
    pub fn next_reputation_terminal_work(
        &self,
    ) -> Result<Option<PorReputationTerminalWorkV1>, PorTrackerError> {
        let tracker = self.inner.read().expect("por tracker poisoned");
        let next_sequence = match tracker.acknowledged_reputation_terminal {
            Some(acknowledged) if acknowledged.sequence == tracker.last_reputation_sequence => {
                return Ok(None);
            }
            Some(acknowledged) => acknowledged
                .sequence
                .checked_add(1)
                .ok_or(PorTrackerError::ReputationSequenceOverflow)?,
            None => 1,
        };
        let Some(finalized) = tracker
            .finalized
            .values()
            .find(|finalized| finalized.reputation_sequence == next_sequence)
        else {
            return Ok(None);
        };
        retained_reputation_work(finalized).map(Some)
    }
    /// Return the number of retained terminals not yet acknowledged.
    #[must_use]
    pub fn pending_reputation_terminal_count(&self) -> u64 {
        let tracker = self.inner.read().expect("por tracker poisoned");
        let acknowledged = tracker
            .acknowledged_reputation_terminal
            .map_or(0, |ack| ack.sequence);
        tracker
            .last_reputation_sequence
            .saturating_sub(acknowledged)
    }
    /// Return history keys whose live lifecycle or delivery work is not yet archived.
    pub(crate) fn protected_history_keys(&self) -> HashSet<([u8; 32], [u8; 32])> {
        let tracker = self.inner.read().expect("por tracker poisoned");
        tracker
            .pending
            .values()
            .map(|state| (state.challenge.manifest_digest, state.challenge.provider_id))
            .chain(tracker.finalized.values().map(|state| {
                (
                    state.state.challenge.manifest_digest,
                    state.state.challenge.provider_id,
                )
            }))
            .collect()
    }
    /// Advance the delivery cursor for the exact next retained terminal.
    ///
    /// Skipped, foreign, stale, or digest-substituted acknowledgements fail
    /// closed. An exact replay of the latest acknowledgement is idempotent.
    pub(crate) fn acknowledge_reputation_terminal(
        &self,
        sequence: u64,
        work_digest: [u8; 32],
    ) -> Result<PorReputationTerminalAckOutcomeV1, PorTrackerError> {
        if sequence == 0 || work_digest == [0; 32] {
            return Err(PorTrackerError::InvalidReputationTerminalAcknowledgement);
        }
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        if let Some(acknowledged) = tracker.acknowledged_reputation_terminal {
            if sequence == acknowledged.sequence {
                return if work_digest == acknowledged.work_digest {
                    Ok(PorReputationTerminalAckOutcomeV1::ExactReplay)
                } else {
                    Err(PorTrackerError::ReputationAcknowledgementDigestMismatch)
                };
            }
            if sequence < acknowledged.sequence {
                return Err(PorTrackerError::StaleReputationAcknowledgement {
                    acknowledged: acknowledged.sequence,
                    received: sequence,
                });
            }
        }
        let expected = tracker
            .acknowledged_reputation_terminal
            .map_or(Some(1), |ack| ack.sequence.checked_add(1))
            .ok_or(PorTrackerError::ReputationSequenceOverflow)?;
        if sequence != expected {
            return Err(PorTrackerError::SkippedReputationAcknowledgement {
                expected,
                received: sequence,
            });
        }
        let finalized = tracker
            .finalized
            .values()
            .find(|finalized| finalized.reputation_sequence == expected)
            .ok_or(PorTrackerError::UnknownReputationTerminalWork { sequence })?;
        let retained = retained_reputation_work(finalized)?;
        if retained.work_digest != work_digest {
            return Err(PorTrackerError::ReputationAcknowledgementDigestMismatch);
        }
        tracker.acknowledged_reputation_terminal = Some(PorReputationTerminalAckV1 {
            sequence,
            work_digest,
        });
        Ok(PorReputationTerminalAckOutcomeV1::Advanced)
    }
    /// Reconcile a restored local archive head with an authenticated live prefix.
    ///
    /// A live-ahead head is accepted only when every newly archived record is
    /// the exact acknowledged prefix still retained by the local checkpoint.
    /// This closes the crash window where durable external appends precede the
    /// local checkpoint without adding an intent field to the V1 format.
    pub(crate) fn reconcile_restored_replay_archive_head(
        &self,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        expected_binding: PorFinalizedReplayArchiveBindingV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<bool, PorTrackerError> {
        replay_archive
            .check_readiness()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        if replay_archive
            .binding()
            .map_err(PorTrackerError::ReplayArchiveExternal)?
            != expected_binding
        {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        let restored_head = self
            .inner
            .read()
            .expect("por tracker poisoned")
            .replay_archive_receipt;
        if let Some(restored) = restored_head {
            restored.validate()?;
            if restored.binding() != expected_binding {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
        }
        let current_head = replay_archive
            .current_head()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        if let Some(current) = current_head {
            current.validate()?;
            if current.binding() != expected_binding {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
        }
        let reconciliation = match (restored_head, current_head) {
            (Some(_), None) => return Err(PorTrackerError::ReplayArchiveHeadRollback),
            (None, None) => None,
            (Some(restored), Some(current))
                if current.reputation_sequence() < restored.reputation_sequence()
                    || (current.reputation_sequence() == restored.reputation_sequence()
                        && current != restored) =>
            {
                return Err(PorTrackerError::ReplayArchiveHeadRollback);
            }
            (Some(restored), Some(current)) if current == restored => None,
            (restored, Some(current)) => {
                let restored_sequence =
                    restored.map_or(0, PorFinalizedReplayArchiveReceiptV1::reputation_sequence);
                let advance_count = current
                    .reputation_sequence()
                    .checked_sub(restored_sequence)
                    .filter(|count| *count != 0)
                    .ok_or(PorTrackerError::ReplayArchiveHeadRollback)?;
                let successor_count = advance_count
                    .checked_sub(1)
                    .ok_or(PorTrackerError::ReplayArchiveHeadRollback)?;
                if successor_count
                    > u64::try_from(proof_bounds.max_successor_receipts())
                        .map_err(|_| PorTrackerError::ReplayArchiveProofLimitExceeded)?
                {
                    return Err(PorTrackerError::ReplayArchiveProofLimitExceeded);
                }
                let advance_count = usize::try_from(advance_count)
                    .map_err(|_| PorTrackerError::ReplayArchiveProofLimitExceeded)?;
                let tracker = self.inner.read().expect("por tracker poisoned");
                let acknowledged = tracker
                    .acknowledged_reputation_terminal
                    .ok_or(PorTrackerError::ReplayArchiveHeadRollback)?;
                let last_reputation_sequence = tracker.last_reputation_sequence;
                if current.reputation_sequence() > acknowledged.sequence
                    || acknowledged.sequence > last_reputation_sequence
                    || current.reputation_sequence() > last_reputation_sequence
                    || advance_count > tracker.finalized.len()
                {
                    return Err(PorTrackerError::ReplayArchiveHeadRollback);
                }
                let mut by_sequence = BTreeMap::new();
                for (challenge_id, finalized) in &tracker.finalized {
                    if finalized.reputation_sequence > restored_sequence
                        && finalized.reputation_sequence <= current.reputation_sequence()
                        && !finalized.repair_handoff_acknowledged
                    {
                        return Err(PorTrackerError::RepairHandoffPendingCompaction);
                    }
                    if finalized.reputation_sequence > restored_sequence
                        && finalized.reputation_sequence <= current.reputation_sequence()
                        && by_sequence
                            .insert(
                                finalized.reputation_sequence,
                                (
                                    *challenge_id,
                                    PorFinalizedReplayArchiveRecordV1::from_finalized(
                                        finalized.clone(),
                                    ),
                                ),
                            )
                            .is_some()
                    {
                        return Err(PorTrackerError::ReplayArchiveHeadRollback);
                    }
                }
                if by_sequence.len() != advance_count {
                    return Err(PorTrackerError::ReplayArchiveHeadRollback);
                }
                let mut local_prefix = Vec::with_capacity(advance_count);
                for sequence in restored_sequence
                    .checked_add(1)
                    .ok_or(PorTrackerError::ReputationSequenceOverflow)?
                    ..=current.reputation_sequence()
                {
                    let local = by_sequence
                        .remove(&sequence)
                        .ok_or(PorTrackerError::ReplayArchiveHeadRollback)?;
                    validate_replay_archive_record(&local.1)?;
                    local_prefix.push(local);
                }
                drop(tracker);
                let first = local_prefix
                    .first()
                    .ok_or(PorTrackerError::ReplayArchiveHeadRollback)?;
                let lookup = replay_archive
                    .lookup(first.0, current, proof_bounds)
                    .map_err(PorTrackerError::ReplayArchiveExternal)?;
                let PorFinalizedReplayArchiveLookupV1::Found(readback) = lookup else {
                    return Err(PorTrackerError::ReplayArchiveHeadRollback);
                };
                readback.validate_at_checkpoint(expected_binding, current, proof_bounds)?;
                if readback.record != first.1
                    || readback.successor_receipts.len() != local_prefix.len().saturating_sub(1)
                {
                    return Err(PorTrackerError::ReplayArchiveHeadRollback);
                }
                let first_previous = restored.map(PorFinalizedReplayArchiveReceiptV1::head_digest);
                readback.receipt.validate_record(
                    expected_binding,
                    &first.1,
                    Some(first_previous),
                )?;
                let mut previous = readback.receipt;
                for (receipt, (_, record)) in readback
                    .successor_receipts
                    .iter()
                    .copied()
                    .zip(local_prefix.iter().skip(1))
                {
                    receipt.validate_record(
                        expected_binding,
                        record,
                        Some(Some(previous.head_digest())),
                    )?;
                    previous = receipt;
                }
                if previous != current {
                    return Err(PorTrackerError::ReplayArchiveHeadRollback);
                }
                Some((
                    current,
                    local_prefix,
                    acknowledged,
                    last_reputation_sequence,
                ))
            }
        };
        replay_archive
            .check_readiness()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        if replay_archive
            .binding()
            .map_err(PorTrackerError::ReplayArchiveExternal)?
            != expected_binding
        {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        let confirmed_head = replay_archive
            .current_head()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        if confirmed_head != current_head {
            return Err(PorTrackerError::ReplayArchiveHeadRollback);
        }
        if replay_archive
            .binding()
            .map_err(PorTrackerError::ReplayArchiveExternal)?
            != expected_binding
        {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        let Some((current, local_prefix, acknowledged, last_reputation_sequence)) = reconciliation
        else {
            return Ok(false);
        };
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        if tracker.replay_archive_receipt != restored_head
            || tracker.acknowledged_reputation_terminal != Some(acknowledged)
            || tracker.last_reputation_sequence != last_reputation_sequence
            || local_prefix.iter().any(|(challenge_id, record)| {
                tracker.finalized.get(challenge_id) != Some(&record.finalized)
            })
        {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR replay state changed during startup archive reconciliation".to_owned(),
            ));
        }
        if local_prefix
            .iter()
            .any(|(challenge_id, _)| tracker.compacted_statuses.contains_key(challenge_id))
        {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR replay prefix overlaps compacted status history".to_owned(),
            ));
        }
        for (challenge_id, record) in local_prefix {
            let status = record.finalized.to_status();
            if tracker.finalized.remove(&challenge_id).is_none() {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "PoR replay prefix disappeared during startup archive reconciliation"
                        .to_owned(),
                ));
            }
            let replaced = tracker.compacted_statuses.insert(challenge_id, status);
            debug_assert!(replaced.is_none());
        }
        tracker.replay_archive_receipt = Some(current);
        Ok(true)
    }
    /// Archive and compact a bounded acknowledged finalized prefix.
    ///
    /// Every record is durably appended and its provider-authenticated receipt is
    /// verified before local replay state is removed. If any append fails,
    /// in-memory state rolls back to its exact pre-call snapshot; an archive
    /// that committed before the failure must return the same receipt on retry.
    pub(crate) fn compact_acknowledged_with_replay_archive(
        &self,
        replay_archive: &dyn PorFinalizedReplayArchiveV1,
        expected_binding: PorFinalizedReplayArchiveBindingV1,
        maximum_records: u32,
    ) -> Result<u32, PorTrackerError> {
        if maximum_records == 0 {
            return Err(PorTrackerError::InvalidReplayArchiveCompactionLimit);
        }
        replay_archive
            .check_readiness()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        let binding = replay_archive
            .binding()
            .map_err(PorTrackerError::ReplayArchiveExternal)?;
        binding.verifying_key()?;
        if binding != expected_binding {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        if tracker
            .replay_archive_receipt
            .is_some_and(|receipt| receipt.binding != binding)
        {
            return Err(PorTrackerError::ReplayArchiveBindingMismatch);
        }
        let original_finalized = tracker.finalized.clone();
        let original_compacted_statuses = tracker.compacted_statuses.clone();
        let original_receipt = tracker.replay_archive_receipt;
        let result = (|| {
            let acknowledged = tracker
                .acknowledged_reputation_terminal
                .map_or(0, |acknowledged| acknowledged.sequence);
            let mut compacted = 0_u32;
            while compacted < maximum_records {
                let archived_through = tracker
                    .replay_archive_receipt
                    .map_or(0, |receipt| receipt.reputation_sequence);
                let next_sequence = archived_through
                    .checked_add(1)
                    .ok_or(PorTrackerError::ReputationSequenceOverflow)?;
                if next_sequence > acknowledged {
                    break;
                }
                let (challenge_id, finalized) = tracker
                    .finalized
                    .iter()
                    .find(|(_, finalized)| finalized.reputation_sequence == next_sequence)
                    .map(|(challenge_id, finalized)| (*challenge_id, finalized.clone()))
                    .ok_or_else(|| {
                        PorTrackerError::InvalidCheckpoint(
                            "acknowledged PoR archive prefix is not locally contiguous".to_owned(),
                        )
                    })?;
                if !finalized.repair_handoff_acknowledged {
                    return Err(PorTrackerError::RepairHandoffPendingCompaction);
                }
                let status = finalized.to_status();
                let record = PorFinalizedReplayArchiveRecordV1::from_finalized(finalized);
                validate_replay_archive_record(&record)?;
                let previous_head = tracker
                    .replay_archive_receipt
                    .map(|receipt| receipt.head_digest);
                let receipt = replay_archive
                    .append(&record, previous_head)
                    .map_err(PorTrackerError::ReplayArchiveExternal)?;
                receipt.validate_record(binding, &record, Some(previous_head))?;
                tracker.finalized.remove(&challenge_id);
                if tracker
                    .compacted_statuses
                    .insert(challenge_id, status)
                    .is_some()
                {
                    return Err(PorTrackerError::InvalidCheckpoint(
                        "PoR replay compaction duplicated status history".to_owned(),
                    ));
                }
                tracker.replay_archive_receipt = Some(receipt);
                compacted = compacted
                    .checked_add(1)
                    .ok_or(PorTrackerError::ReputationSequenceOverflow)?;
            }
            replay_archive
                .check_readiness()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?
                != binding
            {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            let confirmed_head = replay_archive
                .current_head()
                .map_err(PorTrackerError::ReplayArchiveExternal)?;
            if let Some(confirmed) = confirmed_head {
                confirmed.validate()?;
                if confirmed.binding() != binding {
                    return Err(PorTrackerError::ReplayArchiveBindingMismatch);
                }
            }
            if confirmed_head != tracker.replay_archive_receipt {
                return Err(PorTrackerError::ReplayArchiveHeadRollback);
            }
            if replay_archive
                .binding()
                .map_err(PorTrackerError::ReplayArchiveExternal)?
                != binding
            {
                return Err(PorTrackerError::ReplayArchiveBindingMismatch);
            }
            Ok(compacted)
        })();
        if result.is_err() {
            tracker.finalized = original_finalized;
            tracker.compacted_statuses = original_compacted_statuses;
            tracker.replay_archive_receipt = original_receipt;
        }
        result
    }
    /// Restore a validated deterministic tracker checkpoint.
    pub(crate) fn restore_checkpoint(
        &self,
        checkpoint: PorTrackerCheckpointV1,
    ) -> Result<(), PorTrackerError> {
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        if checkpoint.pending.len() > tracker.entry_limit {
            return Err(PorTrackerError::PendingRetentionExhausted {
                limit: tracker.entry_limit,
            });
        }
        if checkpoint.finalized.len() > tracker.entry_limit {
            return Err(PorTrackerError::FinalizedRetentionExhausted {
                limit: tracker.entry_limit,
            });
        }
        let status_count = checkpoint
            .pending
            .len()
            .checked_add(checkpoint.finalized.len())
            .and_then(|value| value.checked_add(checkpoint.compacted_statuses.len()))
            .ok_or_else(|| {
                PorTrackerError::InvalidCheckpoint("PoR status history length overflow".to_owned())
            })?;
        if status_count > tracker.entry_limit {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status history exceeds its retention bound".to_owned(),
            ));
        }
        let minimum_generation = u64::try_from(status_count)
            .map_err(|_| {
                PorTrackerError::InvalidCheckpoint(
                    "PoR status history length is not representable".to_owned(),
                )
            })?
            .checked_add(1)
            .ok_or(PorTrackerError::StatusGenerationExhausted)?;
        let status_generation = checkpoint.status_generation;
        if status_generation < minimum_generation {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR status generation is below its retained-history floor".to_owned(),
            ));
        }
        let last_reputation_sequence = checkpoint.last_reputation_sequence;
        let acknowledged_reputation_terminal = checkpoint.acknowledged_reputation_terminal;
        let replay_archive_receipt = checkpoint.replay_archive_receipt;
        if let Some(receipt) = replay_archive_receipt {
            receipt.validate()?;
        }
        let archived_through =
            replay_archive_receipt.map_or(0, |receipt| receipt.reputation_sequence);
        let mut pending = HashMap::with_capacity(checkpoint.pending.len());
        let mut previous_pending_id = None;
        for state in checkpoint.pending {
            state
                .challenge
                .validate()
                .map_err(PorTrackerError::ChallengeInvalid)?;
            if state.proof_digest.is_some() != state.proof_submitted_at.is_some() {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "proof digest and submission timestamp must either both be present or both be absent"
                        .to_owned(),
                ));
            }
            if let Some(submitted_at) = state.proof_submitted_at
                && (submitted_at < state.challenge.issued_at
                    || submitted_at > state.challenge.deadline_at)
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "proof submission timestamp falls outside its challenge window".to_owned(),
                ));
            }
            Self::validate_authority_status(&state.to_status()).map_err(|error| {
                PorTrackerError::InvalidCheckpoint(format!(
                    "pending state has no valid authoritative projection: {error}"
                ))
            })?;
            let challenge_id = state.challenge.challenge_id;
            if previous_pending_id.is_some_and(|previous| previous >= challenge_id) {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "pending challenges must be strictly ordered by challenge id".to_owned(),
                ));
            }
            previous_pending_id = Some(challenge_id);
            if pending.insert(challenge_id, state).is_some() {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "duplicate pending challenge id".to_owned(),
                ));
            }
        }
        let mut finalized = HashMap::with_capacity(checkpoint.finalized.len());
        let mut reputation_sequences = Vec::with_capacity(checkpoint.finalized.len());
        let mut previous_finalized_id = None;
        for finalized_state in checkpoint.finalized {
            finalized_state
                .state
                .challenge
                .validate()
                .map_err(PorTrackerError::ChallengeInvalid)?;
            if finalized_state.state.proof_digest.is_some()
                != finalized_state.state.proof_submitted_at.is_some()
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized proof digest and timestamp must both be present or absent"
                        .to_owned(),
                ));
            }
            if let Some(submitted_at) = finalized_state.state.proof_submitted_at
                && (submitted_at < finalized_state.state.challenge.issued_at
                    || submitted_at > finalized_state.state.challenge.deadline_at)
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized proof timestamp falls outside its challenge window".to_owned(),
                ));
            }
            finalized_state
                .verdict
                .validate()
                .map_err(PorTrackerError::VerdictInvalid)?;
            finalized_state
                .verdict
                .verify_signatures()
                .map_err(PorTrackerError::VerdictSignatureInvalid)?;
            let expected_stats =
                validate_verdict_transition(&finalized_state.state, &finalized_state.verdict)?;
            if finalized_state.stats != expected_stats {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized verdict statistics do not match the retained challenge".to_owned(),
                ));
            }
            let expected_task_id = (finalized_state.verdict.outcome == AuditOutcomeV1::Failed)
                .then(|| {
                    sorafs_repair_task_id_v1(por_repair_source_identity_v1(
                        finalized_state.verdict.challenge_id,
                    ))
                });
            if finalized_state.repair_task_id != expected_task_id {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized verdict repair task identity is inconsistent".to_owned(),
                ));
            }
            if expected_task_id.is_none() && !finalized_state.repair_handoff_acknowledged {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "non-failed PoR verdict retained a pending repair handoff".to_owned(),
                ));
            }
            let expected_terminal = por_reputation_terminal_from_retained_v1(
                &finalized_state.state,
                &finalized_state.verdict,
                expected_stats,
                expected_task_id,
            )?;
            if finalized_state.reputation_terminal != expected_terminal {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized PoR reputation terminal differs from retained source state"
                        .to_owned(),
                ));
            }
            retained_reputation_work(&finalized_state)?;
            Self::validate_authority_status(&finalized_state.to_status()).map_err(|error| {
                PorTrackerError::InvalidCheckpoint(format!(
                    "finalized state has no valid authoritative projection: {error}"
                ))
            })?;
            reputation_sequences.push(finalized_state.reputation_sequence);
            let challenge_id = finalized_state.state.challenge.challenge_id;
            if previous_finalized_id.is_some_and(|previous| previous >= challenge_id) {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized challenges must be strictly ordered by challenge id".to_owned(),
                ));
            }
            previous_finalized_id = Some(challenge_id);
            if finalized.insert(challenge_id, finalized_state).is_some() {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "duplicate finalized challenge id".to_owned(),
                ));
            }
        }
        if pending.keys().any(|id| finalized.contains_key(id)) {
            return Err(PorTrackerError::InvalidCheckpoint(
                "challenge id appears in both pending and finalized state".to_owned(),
            ));
        }
        let mut compacted_statuses = HashMap::with_capacity(checkpoint.compacted_statuses.len());
        let mut previous_compacted_id = None;
        for status in checkpoint.compacted_statuses {
            status.validate().map_err(|error| {
                PorTrackerError::InvalidCheckpoint(format!("invalid compacted PoR status: {error}"))
            })?;
            if matches!(
                status.status,
                PorChallengeOutcome::AwaitingProof | PorChallengeOutcome::ProofSubmitted
            ) {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "compacted PoR status is not terminal".to_owned(),
                ));
            }
            if previous_compacted_id.is_some_and(|previous| previous >= status.challenge_id) {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "compacted PoR statuses must be strictly ordered by challenge id".to_owned(),
                ));
            }
            previous_compacted_id = Some(status.challenge_id);
            if pending.contains_key(&status.challenge_id)
                || finalized.contains_key(&status.challenge_id)
                || compacted_statuses
                    .insert(status.challenge_id, status)
                    .is_some()
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "challenge id appears more than once in PoR status history".to_owned(),
                ));
            }
        }
        reputation_sequences.sort_unstable();
        let finalized_count = u64::try_from(reputation_sequences.len()).map_err(|_| {
            PorTrackerError::InvalidCheckpoint(
                "finalized PoR reputation sequence count is not representable".to_owned(),
            )
        })?;
        let expected_retained_count = last_reputation_sequence
            .checked_sub(archived_through)
            .ok_or_else(|| {
                PorTrackerError::InvalidCheckpoint(
                    "PoR replay archive extends beyond finalized sequence state".to_owned(),
                )
            })?;
        if expected_retained_count != finalized_count
            || reputation_sequences
                .iter()
                .copied()
                .zip(
                    archived_through
                        .checked_add(1)
                        .ok_or(PorTrackerError::ReputationSequenceOverflow)?
                        ..=last_reputation_sequence,
                )
                .any(|(actual, expected)| actual != expected)
        {
            return Err(PorTrackerError::InvalidCheckpoint(
                "retained PoR reputation sequences must be unique and contiguous after the authenticated archive prefix".to_owned(),
            ));
        }
        if let Some(acknowledged) = acknowledged_reputation_terminal {
            if acknowledged.sequence == 0
                || acknowledged.sequence > last_reputation_sequence
                || archived_through > acknowledged.sequence
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "PoR reputation acknowledgement sequence is outside retained work".to_owned(),
                ));
            }
            if acknowledged.sequence > archived_through {
                let retained = finalized
                    .values()
                    .find(|state| state.reputation_sequence == acknowledged.sequence)
                    .ok_or_else(|| {
                        PorTrackerError::InvalidCheckpoint(
                            "PoR reputation acknowledgement names missing retained work".to_owned(),
                        )
                    })
                    .and_then(retained_reputation_work)?;
                if retained.work_digest != acknowledged.work_digest {
                    return Err(PorTrackerError::InvalidCheckpoint(
                        "PoR reputation acknowledgement digest differs from retained work"
                            .to_owned(),
                    ));
                }
            } else if replay_archive_receipt
                .is_none_or(|receipt| receipt.reputation_work_digest != acknowledged.work_digest)
            {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "PoR reputation acknowledgement digest differs from authenticated archive work"
                        .to_owned(),
                ));
            }
        } else if archived_through != 0 {
            return Err(PorTrackerError::InvalidCheckpoint(
                "PoR replay archive exists without an acknowledged delivery prefix".to_owned(),
            ));
        }
        tracker.pending = pending;
        tracker.finalized = finalized;
        tracker.compacted_statuses = compacted_statuses;
        tracker.status_generation = status_generation;
        tracker.last_reputation_sequence = last_reputation_sequence;
        tracker.acknowledged_reputation_terminal = acknowledged_reputation_terminal;
        tracker.replay_archive_receipt = replay_archive_receipt;
        tracker.latest_status_removals.clear();
        Ok(())
    }
    /// Return whether a challenge remains pending in the tracker.
    #[cfg(test)]
    fn contains_challenge(&self, challenge_id: &[u8; 32]) -> bool {
        self.inner
            .read()
            .expect("por tracker poisoned")
            .pending
            .contains_key(challenge_id)
    }
    /// Return the proof digest recorded for a pending challenge.
    #[cfg(test)]
    fn proof_digest(&self, challenge_id: &[u8; 32]) -> Option<[u8; 32]> {
        self.inner
            .read()
            .expect("por tracker poisoned")
            .pending
            .get(challenge_id)
            .and_then(|state| state.proof_digest)
    }
    /// Return backlog entries for all manifest/provider pairs tracked by the node.
    #[must_use]
    pub fn backlog_entries(&self) -> Vec<PorBacklogEntry> {
        self.collect_backlog(|_| true)
    }
    /// Return backlog entries for the supplied manifest digest.
    #[must_use]
    pub fn backlog_for_manifest(&self, manifest_digest: &[u8; 32]) -> Vec<PorBacklogEntry> {
        self.collect_backlog(|state| state.challenge.manifest_digest == *manifest_digest)
    }
    fn collect_backlog<F>(&self, predicate: F) -> Vec<PorBacklogEntry>
    where
        F: Fn(&ChallengeState) -> bool,
    {
        use std::collections::hash_map::Entry;
        let tracker = self.inner.read().expect("por tracker poisoned");
        let mut grouped: HashMap<([u8; 32], [u8; 32]), PorBacklogEntry> = HashMap::new();
        for state in tracker.pending.values() {
            if !predicate(state) {
                continue;
            }
            let key = (state.challenge.manifest_digest, state.challenge.provider_id);
            match grouped.entry(key) {
                Entry::Occupied(mut entry) => {
                    let snapshot = entry.get_mut();
                    snapshot.pending_challenges = snapshot.pending_challenges.saturating_add(1);
                    snapshot.oldest_epoch_id = Some(match snapshot.oldest_epoch_id {
                        Some(current) => current.min(state.challenge.epoch_id),
                        None => state.challenge.epoch_id,
                    });
                    snapshot.oldest_response_deadline_unix =
                        Some(match snapshot.oldest_response_deadline_unix {
                            Some(current) => current.min(state.challenge.deadline_at),
                            None => state.challenge.deadline_at,
                        });
                }
                Entry::Vacant(entry) => {
                    entry.insert(PorBacklogEntry {
                        manifest_digest: state.challenge.manifest_digest,
                        provider_id: state.challenge.provider_id,
                        pending_challenges: 1,
                        oldest_epoch_id: Some(state.challenge.epoch_id),
                        oldest_response_deadline_unix: Some(state.challenge.deadline_at),
                    });
                }
            }
        }
        grouped.into_values().collect()
    }
}
fn validate_replay_archive_record(
    record: &PorFinalizedReplayArchiveRecordV1,
) -> Result<(), PorTrackerError> {
    let finalized = &record.finalized;
    finalized
        .state
        .challenge
        .validate()
        .map_err(PorTrackerError::ChallengeInvalid)?;
    if finalized.state.proof_digest.is_some() != finalized.state.proof_submitted_at.is_some() {
        return Err(PorTrackerError::InvalidReplayArchiveRecord);
    }
    if let Some(submitted_at) = finalized.state.proof_submitted_at
        && (submitted_at < finalized.state.challenge.issued_at
            || submitted_at > finalized.state.challenge.deadline_at)
    {
        return Err(PorTrackerError::InvalidReplayArchiveRecord);
    }
    finalized
        .verdict
        .validate()
        .map_err(PorTrackerError::VerdictInvalid)?;
    finalized
        .verdict
        .verify_signatures()
        .map_err(PorTrackerError::VerdictSignatureInvalid)?;
    let expected_stats = validate_verdict_transition(&finalized.state, &finalized.verdict)?;
    let expected_task_id = (finalized.verdict.outcome == AuditOutcomeV1::Failed).then(|| {
        sorafs_repair_task_id_v1(por_repair_source_identity_v1(
            finalized.verdict.challenge_id,
        ))
    });
    let expected_terminal = por_reputation_terminal_from_retained_v1(
        &finalized.state,
        &finalized.verdict,
        expected_stats,
        expected_task_id,
    )?;
    if finalized.reputation_sequence == 0
        || finalized.stats != expected_stats
        || finalized.repair_task_id != expected_task_id
        || finalized.reputation_terminal != expected_terminal
    {
        return Err(PorTrackerError::InvalidReplayArchiveRecord);
    }
    retained_reputation_work(finalized)?;
    finalized
        .to_status()
        .validate()
        .map_err(|_| PorTrackerError::InvalidReplayArchiveRecord)?;
    record.record_digest()?;
    Ok(())
}
fn retained_reputation_work(
    finalized: &FinalizedChallengeStateV1,
) -> Result<PorReputationTerminalWorkV1, PorTrackerError> {
    PorReputationTerminalWorkV1::try_new(
        finalized.reputation_sequence,
        finalized.state.challenge.provider_id,
        finalized.reputation_terminal,
    )
}
fn checked_seconds_to_millis(value: u64, field: &'static str) -> Result<u64, PorTrackerError> {
    value
        .checked_mul(1_000)
        .ok_or(PorTrackerError::ReputationTimestampOverflow { field })
}
fn por_reputation_terminal_from_retained_v1(
    state: &ChallengeState,
    verdict: &AuditVerdictV1,
    stats: PorVerdictStats,
    repair_task_id: Option<[u8; 32]>,
) -> Result<PorTerminalOutcomeV1, PorTrackerError> {
    let failed_samples = u16::try_from(stats.failed_samples)
        .map_err(|_| PorTrackerError::InvalidFailedSampleCount)?;
    let status = match verdict.outcome {
        AuditOutcomeV1::Success => {
            if repair_task_id.is_some() {
                return Err(PorTrackerError::InvalidReputationTerminalWork);
            }
            PorTerminalStatusV1::Verified
        }
        AuditOutcomeV1::Repaired => {
            if repair_task_id.is_some() {
                return Err(PorTrackerError::InvalidReputationTerminalWork);
            }
            PorTerminalStatusV1::Repaired
        }
        AuditOutcomeV1::Failed if state.proof_digest.is_some() => {
            if repair_task_id.is_none() {
                return Err(PorTrackerError::InvalidReputationTerminalWork);
            }
            // The tracker rejects proofs submitted after the challenge
            // deadline, so `SubmissionLate` cannot be selected here.
            PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::InvalidProof)
        }
        AuditOutcomeV1::Failed if verdict.decided_at >= state.challenge.deadline_at => {
            if repair_task_id.is_none() {
                return Err(PorTrackerError::InvalidReputationTerminalWork);
            }
            PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::DeadlineExpired)
        }
        AuditOutcomeV1::Failed => {
            // No retained typed fact states that storage was unavailable.
            // Free-text `failure_reason` is deliberately non-authoritative,
            // so it cannot select `StorageUnavailable`.
            return Err(PorTrackerError::UnprojectablePreDeadlineFailure);
        }
    };
    let responded_at_unix_ms = state
        .proof_submitted_at
        .map(|timestamp| checked_seconds_to_millis(timestamp, "responded_at_unix_ms"))
        .transpose()?;
    let verifier_latency_ms = state
        .proof_submitted_at
        .map(|submitted_at| {
            let latency_seconds = verdict.decided_at.checked_sub(submitted_at).ok_or(
                PorTrackerError::VerdictBeforeProof {
                    decided_at: verdict.decided_at,
                    submitted_at,
                },
            )?;
            let latency_ms = checked_seconds_to_millis(latency_seconds, "verifier_latency_ms")?;
            u32::try_from(latency_ms).map_err(|_| PorTrackerError::ReputationLatencyOverflow)
        })
        .transpose()?;
    Ok(PorTerminalOutcomeV1 {
        challenge_id: state.challenge.challenge_id,
        manifest_digest: state.challenge.manifest_digest,
        epoch_id: state.challenge.epoch_id,
        drand_round: state.challenge.drand_round,
        forced: state.challenge.forced,
        sample_count: state.challenge.sample_count,
        failed_samples,
        issued_at_unix_ms: checked_seconds_to_millis(
            state.challenge.issued_at,
            "issued_at_unix_ms",
        )?,
        deadline_at_unix_ms: checked_seconds_to_millis(
            state.challenge.deadline_at,
            "deadline_at_unix_ms",
        )?,
        responded_at_unix_ms,
        decided_at_unix_ms: checked_seconds_to_millis(verdict.decided_at, "decided_at_unix_ms")?,
        proof_digest: state.proof_digest,
        repair_task_id,
        verifier_latency_ms,
        status,
    })
}
fn validate_verdict_transition(
    state: &ChallengeState,
    verdict: &AuditVerdictV1,
) -> Result<PorVerdictStats, PorTrackerError> {
    ensure_match(
        verdict.manifest_digest,
        state.challenge.manifest_digest,
        PorTrackerError::MismatchManifest,
    )?;
    ensure_match(
        verdict.provider_id,
        state.challenge.provider_id,
        PorTrackerError::MismatchProvider,
    )?;
    ensure_match(
        verdict.challenge_id,
        state.challenge.challenge_id,
        PorTrackerError::MismatchChallenge,
    )?;
    if verdict.decided_at < state.challenge.issued_at {
        return Err(PorTrackerError::VerdictBeforeChallenge {
            decided_at: verdict.decided_at,
            issued_at: state.challenge.issued_at,
        });
    }
    match (state.proof_digest, verdict.proof_digest) {
        (Some(expected), Some(actual)) if expected != actual => {
            return Err(PorTrackerError::ProofDigestMismatch);
        }
        (Some(_), None) => return Err(PorTrackerError::MissingVerdictProofDigest),
        (None, Some(_)) => return Err(PorTrackerError::UnexpectedVerdictProofDigest),
        (None, None)
            if matches!(
                verdict.outcome,
                AuditOutcomeV1::Success | AuditOutcomeV1::Repaired
            ) =>
        {
            return Err(PorTrackerError::MissingProofForSuccessfulVerdict);
        }
        _ => {}
    }
    if let Some(submitted_at) = state.proof_submitted_at
        && verdict.decided_at < submitted_at
    {
        return Err(PorTrackerError::VerdictBeforeProof {
            decided_at: verdict.decided_at,
            submitted_at,
        });
    }
    let samples = u64::from(state.challenge.sample_count);
    Ok(match verdict.outcome {
        AuditOutcomeV1::Success | AuditOutcomeV1::Repaired => PorVerdictStats {
            success_samples: samples,
            failed_samples: 0,
        },
        AuditOutcomeV1::Failed => PorVerdictStats {
            success_samples: 0,
            failed_samples: samples,
        },
    })
}
fn ensure_match<T: Eq>(left: T, right: T, err: PorTrackerError) -> Result<(), PorTrackerError> {
    if left == right { Ok(()) } else { Err(err) }
}
fn validate_proof_against_challenge(
    proof: &PorProofV1,
    state: &ChallengeState,
) -> Result<(), PorTrackerError> {
    ensure_match(
        proof.manifest_digest,
        state.challenge.manifest_digest,
        PorTrackerError::MismatchManifest,
    )?;
    ensure_match(
        proof.provider_id,
        state.challenge.provider_id,
        PorTrackerError::MismatchProvider,
    )?;
    if proof.samples.len() != usize::from(state.challenge.sample_count) {
        return Err(PorTrackerError::SampleCountMismatch {
            expected: state.challenge.sample_count,
            actual: u16::try_from(proof.samples.len()).unwrap_or(u16::MAX),
        });
    }
    if !proof
        .samples
        .iter()
        .map(|sample| sample.sample_index)
        .eq(state.challenge.sample_indices.iter().copied())
    {
        return Err(PorTrackerError::SampleIndicesMismatch);
    }
    if proof.submitted_at < state.challenge.issued_at
        || proof.submitted_at > state.challenge.deadline_at
    {
        return Err(PorTrackerError::ProofOutsideChallengeWindow {
            submitted_at: proof.submitted_at,
            issued_at: state.challenge.issued_at,
            deadline_at: state.challenge.deadline_at,
        });
    }
    Ok(())
}
/// Errors returned by [`PorTracker`].
#[derive(Debug, Error)]
pub enum PorTrackerError {
    /// Challenge payload failed structural validation.
    #[error("challenge invalid: {0}")]
    ChallengeInvalid(#[source] sorafs_manifest::por::PorChallengeValidationError),
    /// Proof payload failed structural validation.
    #[error("proof invalid: {0}")]
    ProofInvalid(#[from] PorProofValidationError),
    /// Proof signature is invalid or is not bound to the admitted provider.
    #[error("invalid or unauthorised proof signature: {0}")]
    ProofSignatureInvalid(#[source] sorafs_manifest::por::PorSignatureVerificationError),
    /// Audit verdict failed validation.
    #[error("verdict invalid: {0}")]
    VerdictInvalid(#[source] sorafs_manifest::por::AuditVerdictValidationError),
    /// Verdict signatures do not satisfy the trusted-auditor policy.
    #[error("invalid or unauthorised verdict signatures: {0}")]
    VerdictSignatureInvalid(#[source] sorafs_manifest::por::PorSignatureVerificationError),
    /// Challenge already recorded with differing payload.
    #[error("challenge with identical id already exists")]
    ChallengeConflict,
    /// Pending challenge retention reached its configured hard ceiling.
    #[error("pending PoR challenge retention exhausted (limit {limit})")]
    PendingRetentionExhausted {
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Finalized challenge replay retention reached its configured hard ceiling.
    ///
    /// Acknowledged records can be compacted only through the authenticated replay-archive seam.
    /// Nodes without that deployment adapter fail closed at this ceiling.
    #[error("finalized PoR challenge retention exhausted (limit {limit})")]
    FinalizedRetentionExhausted {
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Every bounded ingestion-history entry still owns unarchived lifecycle work.
    #[error("PoR ingestion history retention exhausted by live work (limit {limit})")]
    HistoryRetentionExhausted {
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Monotonic status generation cannot advance without wrapping.
    #[error("PoR status generation exhausted")]
    StatusGenerationExhausted,
    /// Durable tracker checkpoint is malformed or internally inconsistent.
    #[error("invalid PoR tracker checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// A lifecycle mutation could not produce a valid authoritative status.
    #[error("invalid derived PoR authority status: {0}")]
    InvalidAuthorityStatus(String),
    /// Durable auxiliary runtime checkpoint could not be committed.
    #[error("PoR runtime checkpoint failed: {0}")]
    RuntimeCheckpoint(String),
    /// Exact provider-bond penalty arithmetic could not be represented.
    #[error("PoR penalty arithmetic failed: {0}")]
    PenaltyArithmetic(String),
    /// Challenge id is unknown to the tracker.
    #[error("unknown challenge id")]
    UnknownChallenge,
    /// A terminal verdict replay differs from the verdict already retained.
    #[error("challenge was already finalized by a different verdict")]
    VerdictConflict,
    /// Proof references a different manifest digest.
    #[error("proof manifest digest does not match recorded challenge")]
    MismatchManifest,
    /// Proof references a different provider id.
    #[error("proof provider id does not match recorded challenge")]
    MismatchProvider,
    /// Verdict references a different challenge id.
    #[error("verdict challenge id does not match recorded challenge")]
    MismatchChallenge,
    /// Proof sample count differs from the challenge.
    #[error("proof sample count mismatch (expected {expected}, actual {actual})")]
    SampleCountMismatch {
        /// Expected sample count recorded alongside the challenge.
        expected: u16,
        /// Actual sample count present in the proof payload.
        actual: u16,
    },
    /// Proof sample indices do not exactly cover the challenged indices.
    #[error("proof sample indices do not match the recorded challenge")]
    SampleIndicesMismatch,
    /// Provider timestamp falls outside the challenge response window.
    #[error(
        "proof submitted_at {submitted_at} is outside challenge window {issued_at}..={deadline_at}"
    )]
    ProofOutsideChallengeWindow {
        /// Provider-supplied proof timestamp.
        submitted_at: u64,
        /// Challenge issue timestamp.
        issued_at: u64,
        /// Inclusive challenge deadline.
        deadline_at: u64,
    },
    /// Proof already recorded for the challenge.
    #[error("proof already recorded for this challenge")]
    DuplicateProof,
    /// Verdict proof digest does not match the previously recorded proof.
    #[error("proof digest reported by verdict does not match recorded proof")]
    ProofDigestMismatch,
    /// A proof exists, so the verdict must bind its digest.
    #[error("verdict must include the recorded proof digest")]
    MissingVerdictProofDigest,
    /// Verdict claims a proof digest when no proof was recorded.
    #[error("verdict includes a proof digest but no proof was recorded")]
    UnexpectedVerdictProofDigest,
    /// Successful or repaired verdicts cannot be issued without a proof.
    #[error("successful or repaired verdict requires a recorded proof")]
    MissingProofForSuccessfulVerdict,
    /// Verdict predates the challenge.
    #[error("verdict decided_at {decided_at} predates challenge issued_at {issued_at}")]
    VerdictBeforeChallenge {
        /// Verdict decision timestamp.
        decided_at: u64,
        /// Challenge issue timestamp.
        issued_at: u64,
    },
    /// Verdict predates the proof it adjudicates.
    #[error("verdict decided_at {decided_at} predates proof submitted_at {submitted_at}")]
    VerdictBeforeProof {
        /// Verdict decision timestamp.
        decided_at: u64,
        /// Proof submission timestamp.
        submitted_at: u64,
    },
    /// Failed-sample count could not be represented by the canonical repair schema.
    #[error("failed PoR sample count exceeds the canonical repair schema")]
    InvalidFailedSampleCount,
    /// Retained failed-verdict fields cannot form a canonical repair intent.
    #[error("invalid retained PoR repair intent: {0}")]
    RepairIntentInvalid(#[source] PorRepairHandoffError),
    /// The handoff returned an identifier other than the deterministic native task id.
    #[error("PoR repair handoff returned a mismatched native task id")]
    RepairTaskIdMismatch,
    /// A finalized record cannot be compacted before its repair outbox is acknowledged.
    #[error("PoR repair handoff must be acknowledged before replay compaction")]
    RepairHandoffPendingCompaction,
    /// Whole-second source time cannot be represented canonically in milliseconds.
    #[error("PoR reputation timestamp `{field}` overflows milliseconds")]
    ReputationTimestampOverflow {
        /// Canonical projection field whose conversion overflowed.
        field: &'static str,
    },
    /// Proof-to-decision latency cannot be represented by the V1 `u32` field.
    #[error("PoR reputation verifier latency exceeds the V1 millisecond range")]
    ReputationLatencyOverflow,
    /// Monotonic PoR-to-reputation work sequence cannot advance.
    #[error("PoR reputation terminal sequence overflow")]
    ReputationSequenceOverflow,
    /// Canonical terminal bytes could not be encoded for their delivery binding.
    #[error("failed to canonically encode PoR reputation terminal work")]
    ReputationTerminalCanonicalEncoding,
    /// Retained source material cannot form a canonical reputation terminal.
    #[error("retained PoR state cannot form a canonical reputation terminal")]
    InvalidReputationTerminalWork,
    /// A failed verdict arrived before its deadline without proof or a typed
    /// storage-unavailability fact.
    #[error(
        "failed PoR verdict before deadline has no typed fact selecting a reputation failure kind"
    )]
    UnprojectablePreDeadlineFailure,
    /// Acknowledgement contains a zero sequence or inert digest.
    #[error("invalid PoR reputation terminal acknowledgement")]
    InvalidReputationTerminalAcknowledgement,
    /// Acknowledgement digest differs from the exact retained work.
    #[error("PoR reputation terminal acknowledgement digest mismatch")]
    ReputationAcknowledgementDigestMismatch,
    /// Acknowledgement attempts to skip the exact next retained work.
    #[error("PoR reputation terminal acknowledgement skipped sequence {expected} with {received}")]
    SkippedReputationAcknowledgement {
        /// Exact next sequence.
        expected: u64,
        /// Sequence supplied by the caller.
        received: u64,
    },
    /// Acknowledgement predates the latest retained acknowledgement.
    #[error("stale PoR reputation terminal acknowledgement {received}; latest is {acknowledged}")]
    StaleReputationAcknowledgement {
        /// Latest acknowledged sequence.
        acknowledged: u64,
        /// Older sequence supplied by the caller.
        received: u64,
    },
    /// Acknowledgement names no retained terminal work.
    #[error("unknown PoR reputation terminal work sequence {sequence}")]
    UnknownReputationTerminalWork {
        /// Unrecognized sequence.
        sequence: u64,
    },
    /// Replay-archive identity, revision, policy, or public key is invalid.
    #[error("invalid finalized PoR replay-archive binding")]
    InvalidReplayArchiveBinding,
    /// Signed replay-archive receipt is malformed, substituted, or unauthenticated.
    #[error("invalid finalized PoR replay-archive receipt")]
    InvalidReplayArchiveReceipt,
    /// Signed replay-archive absence result is malformed, stale, or unauthenticated.
    #[error("invalid finalized PoR replay-archive absence proof")]
    InvalidReplayArchiveAbsenceProof,
    /// Canonical replay-archive record is internally inconsistent.
    #[error("invalid finalized PoR replay-archive record")]
    InvalidReplayArchiveRecord,
    /// Canonical replay-archive bytes could not be encoded.
    #[error("failed to canonically encode finalized PoR replay-archive material")]
    ReplayArchiveCanonicalEncoding,
    /// A compacted replay requires the deployment-owned archive adapter.
    #[error("finalized PoR replay requires the checkpoint-pinned archive adapter")]
    ReplayArchiveRequired,
    /// Live archive identity differs from the checkpoint-pinned binding.
    #[error("finalized PoR replay-archive binding changed")]
    ReplayArchiveBindingMismatch,
    /// Live archive head is missing, stale, forked, or cannot prove ancestry.
    #[error("finalized PoR replay-archive head rolled back or forked")]
    ReplayArchiveHeadRollback,
    /// An archive inclusion proof exceeds the exact configured resource bounds.
    #[error("finalized PoR replay-archive proof exceeds configured bounds")]
    ReplayArchiveProofLimitExceeded,
    /// External archive append/readback failed.
    #[error(transparent)]
    ReplayArchiveExternal(#[from] PorFinalizedReplayArchiveExternalErrorV1),
    /// Archive compaction requested a zero work bound.
    #[error("finalized PoR replay-archive compaction limit must be non-zero")]
    InvalidReplayArchiveCompactionLimit,
}
#[cfg(test)]
/// Utilities used only in tests to build attested POR inputs.
pub mod test_support {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use sorafs_manifest::{
        por::{AUDIT_VERDICT_VERSION_V1, POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1},
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };
    fn signing_key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic Ed25519 test key")
    }
    fn sign_payload(signature: &mut AdvertSignature, key_pair: &KeyPair, payload: &[u8]) {
        let (algorithm, public_key) = key_pair.public_key().to_bytes();
        assert_eq!(algorithm, Algorithm::Ed25519);
        signature.public_key = public_key.to_vec();
        signature.signature = IrohaSignature::try_new(key_pair.private_key(), payload)
            .expect("sign deterministic PoR test payload")
            .payload()
            .to_vec();
    }
    /// Deterministic PoR challenge used across unit tests.
    pub fn sample_challenge() -> PorChallengeV1 {
        let manifest_digest = [2; 32];
        let provider_id = [3; 32];
        let epoch_id = 123;
        let drand_round = 456;
        let drand_randomness = [0x41; 32];
        let vrf_output = [0x51; 32];
        let seed = derive_challenge_seed(
            &drand_randomness,
            Some(&vrf_output),
            &manifest_digest,
            epoch_id,
        );
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
        PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0x61; 48],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(iroha_crypto::vrf::VrfProof::SigInG1([0x71; 48])),
            forced: false,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed,
            sample_tier: 1,
            sample_count: 2,
            sample_indices: vec![0, 64],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_600,
        }
    }
    /// Deterministic PoR proof matching [`sample_challenge`].
    pub fn sample_proof(challenge: &PorChallengeV1) -> PorProofV1 {
        let mut proof = PorProofV1 {
            version: POR_PROOF_VERSION_V1,
            challenge_id: challenge.challenge_id,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            samples: vec![
                sorafs_manifest::por::PorProofSampleV1 {
                    sample_index: 0,
                    chunk_offset: 0,
                    chunk_size: 65_536,
                    chunk_digest: [5; 32],
                    leaf_digest: [6; 32],
                },
                sorafs_manifest::por::PorProofSampleV1 {
                    sample_index: 64,
                    chunk_offset: 4_194_304,
                    chunk_size: 65_536,
                    chunk_digest: [7; 32],
                    leaf_digest: [8; 32],
                },
            ],
            auth_path: vec![[9; 32], [10; 32]],
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: Vec::new(),
                signature: Vec::new(),
            },
            submitted_at: 1_700_000_100,
        };
        resign_sample_proof(&mut proof);
        proof
    }
    /// Re-sign a mutated proof with the deterministic admitted provider key.
    pub fn resign_sample_proof(proof: &mut PorProofV1) {
        let key_pair = signing_key(0x11);
        let payload = proof
            .signature_payload_bytes()
            .expect("encode proof signature payload");
        sign_payload(&mut proof.signature, &key_pair, &payload);
    }
    /// Deterministic verdict helper stitched to [`sample_challenge`].
    pub fn sample_verdict(challenge: &PorChallengeV1, digest: [u8; 32]) -> AuditVerdictV1 {
        let mut verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            challenge_id: challenge.challenge_id,
            proof_digest: Some(digest),
            outcome: AuditOutcomeV1::Success,
            failure_reason: None,
            decided_at: 1_700_000_300,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: Vec::new(),
                signature: Vec::new(),
            }],
            metadata: Vec::new(),
        };
        resign_sample_verdict(&mut verdict);
        verdict
    }
    /// Re-sign a mutated verdict with the deterministic trusted auditor key.
    pub fn resign_sample_verdict(verdict: &mut AuditVerdictV1) {
        let key_pair = signing_key(0x13);
        let payload = verdict
            .signature_payload_bytes()
            .expect("encode verdict signature payload");
        sign_payload(&mut verdict.auditor_signatures[0], &key_pair, &payload);
    }
    /// Admitted provider key for [`sample_proof`].
    pub fn sample_provider_key() -> Vec<u8> {
        signing_key(0x11).public_key().to_bytes().1.to_vec()
    }
    /// Trusted auditor set for [`sample_verdict`].
    pub fn sample_auditor_keys() -> Vec<Vec<u8>> {
        vec![signing_key(0x13).public_key().to_bytes().1.to_vec()]
    }
    /// Build one canonical record and authenticated non-empty head for startup tests.
    pub fn sample_replay_archive_record_and_head(
        seed: u8,
    ) -> (
        PorFinalizedReplayArchiveBindingV1,
        PorFinalizedReplayArchiveRecordV1,
        PorFinalizedReplayArchiveReceiptV1,
    ) {
        let archive_signing_key = SigningKey::from_bytes(&[seed; 32]);
        let binding = PorFinalizedReplayArchiveBindingV1::try_new(
            [seed.wrapping_add(1); 32],
            7,
            [seed.wrapping_add(2); 32],
            archive_signing_key.verifying_key().to_bytes(),
        )
        .expect("valid replay-archive test binding");
        let challenge = sample_challenge();
        let proof = sample_proof(&challenge);
        let tracker = PorTracker::default();
        tracker
            .record_challenge(&challenge)
            .expect("record replay-archive test challenge");
        tracker
            .record_proof(&proof, &sample_provider_key())
            .expect("record replay-archive test proof");
        tracker
            .record_verdict(
                &sample_verdict(&challenge, proof.proof_digest()),
                &sample_auditor_keys(),
                1,
            )
            .expect("record replay-archive test verdict");
        let finalized = tracker
            .checkpoint()
            .finalized
            .into_iter()
            .next()
            .expect("one finalized replay-archive test record");
        let record = PorFinalizedReplayArchiveRecordV1::from_finalized(finalized);
        let digest = PorFinalizedReplayArchiveReceiptV1::signing_digest(binding, &record, None)
            .expect("derive replay-archive test head digest");
        let receipt = PorFinalizedReplayArchiveReceiptV1::try_new(
            binding,
            &record,
            None,
            archive_signing_key.sign(&digest).to_bytes(),
        )
        .expect("authenticate replay-archive test head");
        (binding, record, receipt)
    }
    /// Build one authenticated non-empty replay-archive head for startup tests.
    pub fn sample_replay_archive_head(
        seed: u8,
    ) -> (
        PorFinalizedReplayArchiveBindingV1,
        PorFinalizedReplayArchiveReceiptV1,
    ) {
        let (binding, _, receipt) = sample_replay_archive_record_and_head(seed);
        (binding, receipt)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::por::test_support::{
        resign_sample_proof, resign_sample_verdict, sample_auditor_keys, sample_challenge,
        sample_proof, sample_provider_key, sample_verdict,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_data_model::{metadata::Metadata, name::Name};
    use sorafs_car::{POR_LEAF_SIZE, PorMerkleTree, StoredChunk};
    use std::{
        collections::BTreeMap,
        convert::TryFrom,
        str::FromStr,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
    #[derive(Debug)]
    struct MemoryReplayArchive {
        runtime_handle: String,
        binding: PorFinalizedReplayArchiveBindingV1,
        signing_key: SigningKey,
        state: Mutex<MemoryReplayArchiveState>,
    }
    #[derive(Debug, Default)]
    struct MemoryReplayArchiveState {
        records: BTreeMap<[u8; 32], PorFinalizedReplayArchiveReadbackV1>,
        latest_head: Option<[u8; 32]>,
        append_calls: u32,
    }
    impl MemoryReplayArchive {
        fn new(seed: u8) -> Self {
            let signing_key = SigningKey::from_bytes(&[seed; 32]);
            let binding = PorFinalizedReplayArchiveBindingV1::try_new(
                [seed.wrapping_add(1); 32],
                1,
                [seed.wrapping_add(2); 32],
                signing_key.verifying_key().to_bytes(),
            )
            .expect("valid archive binding");
            Self {
                runtime_handle: format!("provider://sorafs/por-replay-archive/{seed:02x}"),
                binding,
                signing_key,
                state: Mutex::new(MemoryReplayArchiveState::default()),
            }
        }
        fn append_calls(&self) -> u32 {
            self.state.lock().expect("archive state").append_calls
        }
        fn retained_records(&self) -> usize {
            self.state.lock().expect("archive state").records.len()
        }
    }
    impl PorFinalizedReplayArchiveV1 for MemoryReplayArchive {
        fn runtime_handle(&self) -> &str {
            &self.runtime_handle
        }
        fn binding(
            &self,
        ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            Ok(self.binding)
        }
        fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1> {
            self.binding
                .verifying_key()
                .map(|_| ())
                .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected)
        }
        fn current_head(
            &self,
        ) -> Result<
            Option<PorFinalizedReplayArchiveReceiptV1>,
            PorFinalizedReplayArchiveExternalErrorV1,
        > {
            let state = self.state.lock().expect("archive state");
            let Some(latest_head) = state.latest_head else {
                return Ok(None);
            };
            state
                .records
                .values()
                .find(|readback| readback.receipt.head_digest() == latest_head)
                .map(|readback| Some(readback.receipt))
                .ok_or(PorFinalizedReplayArchiveExternalErrorV1::Rejected)
        }
        fn append(
            &self,
            record: &PorFinalizedReplayArchiveRecordV1,
            expected_previous_head: Option<[u8; 32]>,
        ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            let mut state = self.state.lock().expect("archive state");
            state.append_calls = state.append_calls.saturating_add(1);
            if let Some(existing) = state.records.get(&record.challenge_id()) {
                return if &existing.record == record
                    && existing.receipt.previous_head_digest == expected_previous_head
                {
                    Ok(existing.receipt)
                } else {
                    Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected)
                };
            }
            if state.latest_head != expected_previous_head {
                return Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected);
            }
            let digest = PorFinalizedReplayArchiveReceiptV1::signing_digest(
                self.binding,
                record,
                expected_previous_head,
            )
            .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
            let receipt = PorFinalizedReplayArchiveReceiptV1::try_new(
                self.binding,
                record,
                expected_previous_head,
                self.signing_key.sign(&digest).to_bytes(),
            )
            .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
            state.latest_head = Some(receipt.head_digest());
            state.records.insert(
                record.challenge_id(),
                PorFinalizedReplayArchiveReadbackV1 {
                    record: record.clone(),
                    receipt,
                    successor_receipts: Vec::new(),
                },
            );
            Ok(receipt)
        }
        fn lookup(
            &self,
            challenge_id: [u8; 32],
            expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
            proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
        ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            let state = self.state.lock().expect("archive state");
            let current_head = state
                .records
                .values()
                .find(|readback| Some(readback.receipt.head_digest()) == state.latest_head)
                .map(|readback| readback.receipt)
                .ok_or(PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
            if current_head != expected_checkpoint_head {
                return Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected);
            }
            let Some(mut readback) = state.records.get(&challenge_id).cloned() else {
                let digest = PorFinalizedReplayArchiveAbsenceProofV1::signing_digest(
                    self.binding,
                    challenge_id,
                    expected_checkpoint_head,
                )
                .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
                return PorFinalizedReplayArchiveAbsenceProofV1::try_new(
                    self.binding,
                    challenge_id,
                    expected_checkpoint_head,
                    self.signing_key.sign(&digest).to_bytes(),
                )
                .map(Box::new)
                .map(PorFinalizedReplayArchiveLookupV1::Absent)
                .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected);
            };
            let successor_count = current_head
                .reputation_sequence
                .checked_sub(readback.receipt.reputation_sequence)
                .and_then(|count| usize::try_from(count).ok())
                .ok_or(PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
            if successor_count > proof_bounds.max_successor_receipts() {
                return Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected);
            }
            let mut successors = state
                .records
                .values()
                .map(|candidate| candidate.receipt)
                .filter(|receipt| {
                    receipt.reputation_sequence > readback.receipt.reputation_sequence
                })
                .collect::<Vec<_>>();
            successors.sort_by_key(|receipt| receipt.reputation_sequence);
            readback.successor_receipts = successors;
            readback
                .validate_at_checkpoint(self.binding, expected_checkpoint_head, proof_bounds)
                .map_err(|_| PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
            Ok(PorFinalizedReplayArchiveLookupV1::Found(Box::new(readback)))
        }
    }
    #[derive(Debug)]
    struct BindingDriftReplayArchive<'a> {
        inner: &'a MemoryReplayArchive,
        binding_calls: AtomicUsize,
    }
    impl<'a> BindingDriftReplayArchive<'a> {
        fn new(inner: &'a MemoryReplayArchive) -> Self {
            Self {
                inner,
                binding_calls: AtomicUsize::new(0),
            }
        }
        fn substituted_binding(&self) -> PorFinalizedReplayArchiveBindingV1 {
            let mut binding = self.inner.binding;
            binding.policy_digest[0] ^= 1;
            binding
        }
    }
    impl PorFinalizedReplayArchiveV1 for BindingDriftReplayArchive<'_> {
        fn runtime_handle(&self) -> &str {
            self.inner.runtime_handle()
        }
        fn binding(
            &self,
        ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            if self.binding_calls.fetch_add(1, Ordering::Relaxed) == 0 {
                Ok(self.inner.binding)
            } else {
                Ok(self.substituted_binding())
            }
        }
        fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1> {
            self.inner.check_readiness()
        }
        fn current_head(
            &self,
        ) -> Result<
            Option<PorFinalizedReplayArchiveReceiptV1>,
            PorFinalizedReplayArchiveExternalErrorV1,
        > {
            self.inner.current_head()
        }
        fn append(
            &self,
            record: &PorFinalizedReplayArchiveRecordV1,
            expected_previous_head: Option<[u8; 32]>,
        ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            self.inner.append(record, expected_previous_head)
        }
        fn lookup(
            &self,
            challenge_id: [u8; 32],
            expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
            proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
        ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            self.inner
                .lookup(challenge_id, expected_checkpoint_head, proof_bounds)
        }
    }
    #[derive(Debug)]
    struct StaleHeadReplayArchive<'a> {
        inner: &'a MemoryReplayArchive,
    }
    impl PorFinalizedReplayArchiveV1 for StaleHeadReplayArchive<'_> {
        fn runtime_handle(&self) -> &str {
            self.inner.runtime_handle()
        }
        fn binding(
            &self,
        ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            self.inner.binding()
        }
        fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1> {
            self.inner.check_readiness()
        }
        fn current_head(
            &self,
        ) -> Result<
            Option<PorFinalizedReplayArchiveReceiptV1>,
            PorFinalizedReplayArchiveExternalErrorV1,
        > {
            Ok(None)
        }
        fn append(
            &self,
            record: &PorFinalizedReplayArchiveRecordV1,
            expected_previous_head: Option<[u8; 32]>,
        ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            self.inner.append(record, expected_previous_head)
        }
        fn lookup(
            &self,
            challenge_id: [u8; 32],
            expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
            proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
        ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1>
        {
            self.inner
                .lookup(challenge_id, expected_checkpoint_head, proof_bounds)
        }
    }
    fn next_challenge(base: &PorChallengeV1, delta: u64) -> PorChallengeV1 {
        let mut challenge = base.clone();
        challenge.epoch_id = challenge.epoch_id.saturating_add(delta);
        challenge.drand_round = challenge.drand_round.saturating_add(delta);
        challenge.issued_at = challenge.issued_at.saturating_add(delta);
        challenge.deadline_at = challenge.deadline_at.saturating_add(delta);
        challenge.seed = derive_challenge_seed(
            &challenge.drand_randomness,
            challenge.vrf_output.as_ref(),
            &challenge.manifest_digest,
            challenge.epoch_id,
        );
        challenge.challenge_id = derive_challenge_id(
            &challenge.seed,
            &challenge.manifest_digest,
            &challenge.provider_id,
            challenge.epoch_id,
            challenge.drand_round,
        );
        challenge
    }
    fn build_mock_tree(small_count: usize, large_count: usize) -> PorMerkleTree {
        let leaf_count = small_count
            .checked_add(large_count)
            .expect("test leaf count fits");
        let payload_len = leaf_count
            .checked_mul(POR_LEAF_SIZE)
            .expect("test payload length fits");
        let payload = vec![0xA5; payload_len];
        let chunks = [StoredChunk {
            offset: 0,
            length: u32::try_from(payload_len).expect("test payload fits in one chunk"),
            blake3: blake3::hash(&payload).into(),
        }];
        PorMerkleTree::try_from_payload(&payload, &chunks).expect("canonical test PoR tree")
    }
    #[test]
    fn determine_sample_plan_matches_spec_tiers() {
        let edge = determine_sample_plan(5 * GIB, 1);
        assert_eq!(edge.tier, SAMPLE_TIER_EDGE);
        assert_eq!(edge.small_target, 64);
        assert_eq!(edge.large_target, 0);
        let standard = determine_sample_plan(50 * GIB, 1);
        assert_eq!(standard.tier, SAMPLE_TIER_STANDARD);
        assert_eq!(standard.small_target, 96);
        assert_eq!(standard.large_target, 32);
        let archival = determine_sample_plan(200 * GIB, 1);
        assert_eq!(archival.tier, SAMPLE_TIER_ARCHIVAL);
        assert_eq!(archival.small_target, 0);
        assert_eq!(archival.large_target, 256);
    }
    #[test]
    fn determine_sample_plan_applies_multiplier() {
        let plan = determine_sample_plan(50 * GIB, 3);
        assert_eq!(plan.tier, SAMPLE_TIER_STANDARD);
        assert_eq!(plan.small_target, 288);
        assert_eq!(plan.large_target, 96);
    }
    #[test]
    fn sample_policy_defaults_without_metadata() {
        let metadata = Metadata::default();
        let policy = PorSamplePolicy::from_metadata([0u8; 32], &metadata).expect("default policy");
        assert_eq!(policy.multiplier_for("sorafs.sf1@1.0.0"), 1);
    }
    #[test]
    fn sample_policy_parses_numeric_overrides() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(SAMPLE_MULTIPLIER_METADATA_KEY).expect("valid metadata key"),
            r#"{"default":2,"sorafs.sf1@1.0.0":3,"sorafs.sf2@1.0.0":"4"}"#,
        );
        let policy =
            PorSamplePolicy::from_metadata([0x11; 32], &metadata).expect("policy overrides");
        assert_eq!(policy.multiplier_for("sorafs.sf1@1.0.0"), 3);
        assert_eq!(policy.multiplier_for("sorafs.sf2@1.0.0"), 4);
        assert_eq!(policy.multiplier_for("sorafs.sf3@1.0.0"), 2);
    }
    #[test]
    fn sample_policy_rejects_out_of_range_multiplier() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(SAMPLE_MULTIPLIER_METADATA_KEY).expect("valid metadata key"),
            0u64,
        );
        let err = PorSamplePolicy::from_metadata([0xAA; 32], &metadata).expect_err("should fail");
        match err {
            PorChallengePlannerError::InvalidSampleMultiplier { reason, .. } => {
                assert!(
                    reason.contains("between 1 and"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn sample_leaf_indices_respects_targets() {
        let seed = [0xAB; 32];
        let edge_tree = build_mock_tree(128, 0);
        let edge_plan = SamplePlan {
            tier: SAMPLE_TIER_EDGE,
            small_target: 64,
            large_target: 0,
        };
        let edge_selection = sample_leaf_indices(&edge_tree, seed, edge_plan).unwrap();
        assert_eq!(edge_selection.indices.len(), 64);
        assert_eq!(edge_selection.duplicate_count, 0);
        let standard_tree = build_mock_tree(200, 64);
        let standard_plan = SamplePlan {
            tier: SAMPLE_TIER_STANDARD,
            small_target: 96,
            large_target: 32,
        };
        let standard_selection = sample_leaf_indices(&standard_tree, seed, standard_plan).unwrap();
        assert_eq!(standard_selection.indices.len(), 128);
        assert_eq!(standard_selection.duplicate_count, 0);
        let archival_tree = build_mock_tree(0, 512);
        let archival_plan = SamplePlan {
            tier: SAMPLE_TIER_ARCHIVAL,
            small_target: 0,
            large_target: 256,
        };
        let archival_selection = sample_leaf_indices(&archival_tree, seed, archival_plan).unwrap();
        assert_eq!(archival_selection.indices.len(), 256);
        assert_eq!(archival_selection.duplicate_count, 0);
    }
    #[test]
    fn tracker_happy_path() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        tracker.record_challenge(&challenge).unwrap();
        let proof = sample_proof(&challenge);
        let digest = proof.proof_digest();
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let verdict = sample_verdict(&challenge, digest);
        let stats = tracker
            .record_verdict(&verdict, &sample_auditor_keys(), 1)
            .unwrap();
        assert_eq!(
            stats,
            PorVerdictStats {
                success_samples: 2,
                failed_samples: 0
            }
        );
    }
    #[test]
    fn reputation_projection_preserves_success_repair_and_zero_latency() {
        for (index, expected_status) in
            [PorTerminalStatusV1::Verified, PorTerminalStatusV1::Repaired]
                .into_iter()
                .enumerate()
        {
            let tracker = PorTracker::default();
            let challenge = next_challenge(&sample_challenge(), index as u64);
            tracker.record_challenge(&challenge).unwrap();
            let mut proof = sample_proof(&challenge);
            proof.submitted_at = 1_700_000_300;
            resign_sample_proof(&mut proof);
            tracker
                .record_proof(&proof, &sample_provider_key())
                .unwrap();
            let mut verdict = sample_verdict(&challenge, proof.proof_digest());
            verdict.outcome = if index == 0 {
                AuditOutcomeV1::Success
            } else {
                AuditOutcomeV1::Repaired
            };
            verdict.failure_reason = (index == 1).then(|| "repair recovered service".to_owned());
            resign_sample_verdict(&mut verdict);
            let transition = tracker
                .record_verdict_with(&verdict, &sample_auditor_keys(), 1, |_| {
                    panic!("successful terminal must not invoke repair handoff")
                })
                .expect("project proof-bearing success");
            assert_eq!(transition.reputation_work.sequence, 1);
            assert_eq!(
                transition.reputation_work.provider_id,
                challenge.provider_id
            );
            assert_eq!(transition.reputation_work.terminal.status, expected_status);
            assert_eq!(
                transition.reputation_work.terminal.verifier_latency_ms,
                Some(0)
            );
            assert_eq!(
                tracker.next_reputation_terminal_work().unwrap(),
                Some(transition.reputation_work)
            );
        }
    }
    #[test]
    fn reputation_projection_maps_only_retained_typed_failure_facts() {
        let proof_tracker = PorTracker::default();
        let proof_challenge = sample_challenge();
        proof_tracker.record_challenge(&proof_challenge).unwrap();
        let proof = sample_proof(&proof_challenge);
        proof_tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let mut invalid = sample_verdict(&proof_challenge, proof.proof_digest());
        invalid.outcome = AuditOutcomeV1::Failed;
        invalid.failure_reason = Some("Merkle verification failed".to_owned());
        resign_sample_verdict(&mut invalid);
        let invalid_transition = proof_tracker
            .record_verdict_with(&invalid, &sample_auditor_keys(), 1, |intent| {
                Ok(intent.repair_task_id())
            })
            .expect("proof-bearing failure");
        assert_eq!(
            invalid_transition.reputation_work.terminal.status,
            PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::InvalidProof)
        );
        assert_eq!(
            invalid_transition
                .reputation_work
                .terminal
                .verifier_latency_ms,
            Some(200_000)
        );
        let deadline_tracker = PorTracker::default();
        let deadline_challenge = next_challenge(&proof_challenge, 1);
        deadline_tracker
            .record_challenge(&deadline_challenge)
            .unwrap();
        let mut expired = sample_verdict(&deadline_challenge, [0xA5; 32]);
        expired.outcome = AuditOutcomeV1::Failed;
        expired.proof_digest = None;
        expired.decided_at = deadline_challenge.deadline_at;
        expired.failure_reason = Some("deadline elapsed".to_owned());
        resign_sample_verdict(&mut expired);
        let expired_transition = deadline_tracker
            .record_verdict_with(&expired, &sample_auditor_keys(), 1, |intent| {
                Ok(intent.repair_task_id())
            })
            .expect("no-proof failure at deadline");
        assert_eq!(
            expired_transition.reputation_work.terminal.status,
            PorTerminalStatusV1::Failed(PorTerminalFailureKindV1::DeadlineExpired)
        );
        assert_eq!(
            expired_transition
                .reputation_work
                .terminal
                .verifier_latency_ms,
            None
        );
    }
    #[test]
    fn pre_deadline_failure_text_cannot_fabricate_storage_unavailability() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        tracker.record_challenge(&challenge).unwrap();
        let mut verdict = sample_verdict(&challenge, [0xA5; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.proof_digest = None;
        verdict.decided_at = challenge.deadline_at - 1;
        verdict.failure_reason =
            Some("storage unavailable; please classify StorageUnavailable".to_owned());
        resign_sample_verdict(&mut verdict);
        let repair_calls = AtomicUsize::new(0);
        assert!(matches!(
            tracker.record_verdict_with(&verdict, &sample_auditor_keys(), 1, |_| {
                repair_calls.fetch_add(1, Ordering::Relaxed);
                Ok([0xFF; 32])
            }),
            Err(PorTrackerError::UnprojectablePreDeadlineFailure)
        ));
        assert_eq!(repair_calls.load(Ordering::Relaxed), 0);
        assert!(tracker.contains_challenge(&challenge.challenge_id));
        assert_eq!(tracker.pending_reputation_terminal_count(), 0);
        assert_eq!(tracker.next_reputation_terminal_work().unwrap(), None);
    }
    #[test]
    fn reputation_projection_rejects_timestamp_and_latency_overflow_atomically() {
        let timestamp_tracker = PorTracker::default();
        let mut timestamp_challenge = sample_challenge();
        timestamp_challenge.issued_at = u64::MAX / 1_000 + 1;
        timestamp_challenge.deadline_at = timestamp_challenge.issued_at + 1;
        timestamp_tracker
            .record_challenge(&timestamp_challenge)
            .unwrap();
        let mut timestamp_verdict = sample_verdict(&timestamp_challenge, [0xA5; 32]);
        timestamp_verdict.outcome = AuditOutcomeV1::Failed;
        timestamp_verdict.proof_digest = None;
        timestamp_verdict.decided_at = timestamp_challenge.deadline_at;
        timestamp_verdict.failure_reason = Some("deadline elapsed".to_owned());
        resign_sample_verdict(&mut timestamp_verdict);
        assert!(matches!(
            timestamp_tracker.record_verdict_with(
                &timestamp_verdict,
                &sample_auditor_keys(),
                1,
                |intent| Ok(intent.repair_task_id()),
            ),
            Err(PorTrackerError::ReputationTimestampOverflow { .. })
        ));
        assert!(timestamp_tracker.contains_challenge(&timestamp_challenge.challenge_id));
        let latency_tracker = PorTracker::default();
        let latency_challenge = sample_challenge();
        latency_tracker
            .record_challenge(&latency_challenge)
            .unwrap();
        let proof = sample_proof(&latency_challenge);
        latency_tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let mut latency_verdict = sample_verdict(&latency_challenge, proof.proof_digest());
        latency_verdict.decided_at = proof.submitted_at + u64::from(u32::MAX) / 1_000 + 1;
        resign_sample_verdict(&mut latency_verdict);
        assert!(matches!(
            latency_tracker.record_verdict_with(
                &latency_verdict,
                &sample_auditor_keys(),
                1,
                |_| panic!("success must not invoke repair handoff"),
            ),
            Err(PorTrackerError::ReputationLatencyOverflow)
        ));
        assert!(latency_tracker.contains_challenge(&latency_challenge.challenge_id));
        let sequence_tracker = PorTracker::default();
        let sequence_challenge = sample_challenge();
        let sequence_proof = sample_proof(&sequence_challenge);
        sequence_tracker
            .record_challenge(&sequence_challenge)
            .unwrap();
        sequence_tracker
            .record_proof(&sequence_proof, &sample_provider_key())
            .unwrap();
        sequence_tracker
            .inner
            .write()
            .expect("tracker lock")
            .last_reputation_sequence = u64::MAX;
        assert!(matches!(
            sequence_tracker.record_verdict_with(
                &sample_verdict(&sequence_challenge, sequence_proof.proof_digest()),
                &sample_auditor_keys(),
                1,
                |_| panic!("sequence overflow precedes repair handoff"),
            ),
            Err(PorTrackerError::ReputationSequenceOverflow)
        ));
        assert!(sequence_tracker.contains_challenge(&sequence_challenge.challenge_id));
    }
    #[test]
    fn tracker_reports_vrf_seed_and_proof_latency_metrics_without_replay_inflation() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        tracker.record_challenge(&challenge).unwrap();
        tracker
            .record_challenge(&challenge)
            .expect("exact challenge replay is idempotent");
        let proof = sample_proof(&challenge);
        tracker
            .record_proof(&proof, &sample_provider_key())
            .expect("record proof");
        assert!(matches!(
            tracker.record_proof(&proof, &sample_provider_key()),
            Ok(PorProofRecordOutcomeV1::ExactReplay(_))
        ));
        let mut forced = next_challenge(&challenge, 1);
        forced.forced = true;
        forced.vrf_output = None;
        forced.vrf_proof = None;
        forced.seed = derive_challenge_seed(
            &forced.drand_randomness,
            None,
            &forced.manifest_digest,
            forced.epoch_id,
        );
        forced.challenge_id = derive_challenge_id(
            &forced.seed,
            &forced.manifest_digest,
            &forced.provider_id,
            forced.epoch_id,
            forced.drand_round,
        );
        tracker
            .record_challenge(&forced)
            .expect("record forced challenge");
        let mut invalid_seed = next_challenge(&forced, 1);
        invalid_seed.seed[0] ^= 1;
        assert!(matches!(
            tracker.record_challenge(&invalid_seed),
            Err(PorTrackerError::ChallengeInvalid(
                PorChallengeValidationError::SeedMismatch
            ))
        ));
        assert_eq!(
            tracker.protocol_metrics(),
            PorProtocolMetricsSnapshot {
                challenges_total: 2,
                vrf_challenges: 1,
                forced_challenges: 1,
                seed_bindings_validated: 2,
                seed_binding_failures: 1,
                proofs_accepted: 1,
                proof_latency_samples: 1,
                proof_latency_total_ms: 100_000,
                proof_latency_max_ms: 100_000,
            }
        );
    }
    #[test]
    fn tracker_accepts_exact_finalized_replay_and_rejects_conflict() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        let proof = sample_proof(&challenge);
        let verdict = sample_verdict(&challenge, proof.proof_digest());
        tracker.record_challenge(&challenge).unwrap();
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        tracker
            .record_verdict(&verdict, &sample_auditor_keys(), 1)
            .unwrap();
        tracker
            .record_challenge(&challenge)
            .expect("exact finalized replay is idempotent");
        let mut conflicting = challenge.clone();
        conflicting.deadline_at = conflicting.deadline_at.saturating_add(1);
        assert!(matches!(
            tracker.record_challenge(&conflicting),
            Err(PorTrackerError::ChallengeConflict)
        ));
        assert!(tracker.backlog_entries().is_empty());
    }
    #[test]
    fn reputation_work_is_ordered_deduplicated_and_strictly_acknowledged() {
        let tracker = PorTracker::default();
        let first_challenge = sample_challenge();
        let first_proof = sample_proof(&first_challenge);
        let first_verdict = sample_verdict(&first_challenge, first_proof.proof_digest());
        tracker.record_challenge(&first_challenge).unwrap();
        tracker
            .record_proof(&first_proof, &sample_provider_key())
            .unwrap();
        let first = tracker
            .record_verdict_with(&first_verdict, &sample_auditor_keys(), 1, |_| {
                panic!("success must not invoke repair handoff")
            })
            .unwrap()
            .reputation_work;
        let replay = tracker
            .record_verdict_with(&first_verdict, &sample_auditor_keys(), 1, |_| {
                panic!("exact replay must not invoke repair handoff")
            })
            .expect("exact verdict replay");
        assert!(!replay.newly_finalized);
        assert_eq!(replay.reputation_work, first);
        assert_eq!(tracker.pending_reputation_terminal_count(), 1);
        let second_challenge = next_challenge(&first_challenge, 1);
        let second_proof = sample_proof(&second_challenge);
        let second_verdict = sample_verdict(&second_challenge, second_proof.proof_digest());
        tracker.record_challenge(&second_challenge).unwrap();
        tracker
            .record_proof(&second_proof, &sample_provider_key())
            .unwrap();
        let second = tracker
            .record_verdict_with(&second_verdict, &sample_auditor_keys(), 1, |_| {
                panic!("success must not invoke repair handoff")
            })
            .unwrap()
            .reputation_work;
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(
            tracker.next_reputation_terminal_work().unwrap(),
            Some(first)
        );
        assert!(matches!(
            tracker.acknowledge_reputation_terminal(second.sequence, second.work_digest),
            Err(PorTrackerError::SkippedReputationAcknowledgement {
                expected: 1,
                received: 2
            })
        ));
        let mut substituted = first.work_digest;
        substituted[0] ^= 0x80;
        assert!(matches!(
            tracker.acknowledge_reputation_terminal(first.sequence, substituted),
            Err(PorTrackerError::ReputationAcknowledgementDigestMismatch)
        ));
        assert_eq!(
            tracker
                .acknowledge_reputation_terminal(first.sequence, first.work_digest)
                .unwrap(),
            PorReputationTerminalAckOutcomeV1::Advanced
        );
        assert_eq!(
            tracker
                .acknowledge_reputation_terminal(first.sequence, first.work_digest)
                .unwrap(),
            PorReputationTerminalAckOutcomeV1::ExactReplay
        );
        assert_eq!(
            tracker.next_reputation_terminal_work().unwrap(),
            Some(second)
        );
        assert_eq!(
            tracker
                .acknowledge_reputation_terminal(second.sequence, second.work_digest)
                .unwrap(),
            PorReputationTerminalAckOutcomeV1::Advanced
        );
        assert!(matches!(
            tracker.acknowledge_reputation_terminal(3, [0xA3; 32]),
            Err(PorTrackerError::UnknownReputationTerminalWork { sequence: 3 })
        ));
        assert!(matches!(
            tracker.acknowledge_reputation_terminal(first.sequence, first.work_digest),
            Err(PorTrackerError::StaleReputationAcknowledgement {
                acknowledged: 2,
                received: 1
            })
        ));
        assert_eq!(tracker.pending_reputation_terminal_count(), 0);
        assert_eq!(tracker.next_reputation_terminal_work().unwrap(), None);
    }
    include!("por/tests/reputation_archive.rs");
    include!("por/tests/tracker_failure.rs");
}
