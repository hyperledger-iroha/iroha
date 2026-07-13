//! PoR challenge/proof tracking for the embedded storage node.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use iroha_data_model::metadata::Metadata;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use norito::json::Value as JsonValue;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sorafs_car::PorMerkleTree;
use sorafs_manifest::por::{
    AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1, PorProofValidationError,
    derive_challenge_id, derive_challenge_seed,
};
use thiserror::Error;

use crate::{repair::RepairStoreError, store::StoredManifest};

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
const DEFAULT_TRACKER_ENTRY_LIMIT: usize = 65_536;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PorVerdictStats {
    /// Number of successful samples recorded by the verdict.
    pub success_samples: u64,
    /// Number of failed samples recorded by the verdict.
    pub failed_samples: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ChallengeState {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
    proof_submitted_at: Option<u64>,
}

#[derive(Debug)]
struct PorTrackerState {
    pending: HashMap<[u8; 32], ChallengeState>,
    finalized: HashMap<[u8; 32], PorChallengeV1>,
    entry_limit: usize,
}

impl Default for PorTrackerState {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            finalized: HashMap::new(),
            entry_limit: DEFAULT_TRACKER_ENTRY_LIMIT,
        }
    }
}

/// Canonical durable snapshot of PoR challenge replay-protection state.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct PorTrackerCheckpointV1 {
    pending: Vec<ChallengeState>,
    finalized: Vec<PorChallengeV1>,
}

/// Tracks the lifecycle of PoR challenges, proofs, and verdicts.
#[derive(Debug, Default, Clone)]
pub struct PorTracker {
    inner: Arc<RwLock<PorTrackerState>>,
}

impl PorTracker {
    /// Construct a tracker with a hard ceiling for pending and finalized entries.
    #[must_use]
    pub(crate) fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PorTrackerState {
                entry_limit: entry_limit.max(1),
                ..PorTrackerState::default()
            })),
        }
    }

    /// Register a new PoR challenge.
    pub(crate) fn record_challenge(
        &self,
        challenge: &PorChallengeV1,
    ) -> Result<(), PorTrackerError> {
        challenge
            .validate()
            .map_err(PorTrackerError::ChallengeInvalid)?;
        let mut state = self.inner.write().expect("por tracker poisoned");
        if let Some(finalized) = state.finalized.get(&challenge.challenge_id) {
            return if finalized == challenge {
                Ok(())
            } else {
                Err(PorTrackerError::ChallengeConflict)
            };
        }
        if !state.pending.contains_key(&challenge.challenge_id)
            && state.pending.len() >= state.entry_limit
        {
            return Err(PorTrackerError::PendingRetentionExhausted {
                limit: state.entry_limit,
            });
        }
        let entry = state.pending.entry(challenge.challenge_id);
        match entry {
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(ChallengeState {
                    challenge: challenge.clone(),
                    proof_digest: None,
                    proof_submitted_at: None,
                });
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(occupied) => {
                // Allow idempotent replays of the same challenge but reject mismatched payloads.
                if occupied.get().challenge == *challenge {
                    Ok(())
                } else {
                    Err(PorTrackerError::ChallengeConflict)
                }
            }
        }
    }

    /// Register a PoR proof response authenticated by provider admission.
    pub(crate) fn record_proof(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorTrackerError> {
        proof.validate().map_err(PorTrackerError::ProofInvalid)?;
        proof
            .verify_signature_for_provider(admitted_provider_key)
            .map_err(PorTrackerError::ProofSignatureInvalid)?;
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        let state = tracker
            .pending
            .get_mut(&proof.challenge_id)
            .ok_or(PorTrackerError::UnknownChallenge)?;
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
        if state.proof_digest.is_some() {
            return Err(PorTrackerError::DuplicateProof);
        }
        state.proof_digest = Some(proof.proof_digest());
        state.proof_submitted_at = Some(proof.submitted_at);
        Ok(())
    }

    /// Finalise a challenge using an audit verdict.
    #[cfg(test)]
    pub(crate) fn record_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<PorVerdictStats, PorTrackerError> {
        self.record_verdict_with(verdict, trusted_auditor_keys, auditor_threshold, |_| Ok(()))
            .map(|(stats, ())| stats)
    }

    /// Finalise a challenge only after `before_commit` succeeds.
    ///
    /// The tracker write lock remains held while the callback runs. This makes
    /// the in-memory state transition atomic with a fallible durable side
    /// effect such as repair-history persistence: callback failures leave the
    /// challenge and proof available for a safe retry.
    pub(crate) fn record_verdict_with<T>(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        before_commit: impl FnOnce(PorVerdictStats) -> Result<T, RepairStoreError>,
    ) -> Result<(PorVerdictStats, T), PorTrackerError> {
        verdict
            .validate()
            .map_err(PorTrackerError::VerdictInvalid)?;
        verdict
            .verify_signatures_with_policy(trusted_auditor_keys, auditor_threshold)
            .map_err(PorTrackerError::VerdictSignatureInvalid)?;
        let mut tracker = self.inner.write().expect("por tracker poisoned");
        let state = tracker
            .pending
            .get(&verdict.challenge_id)
            .ok_or(PorTrackerError::UnknownChallenge)?;
        let stats = validate_verdict_transition(state, verdict)?;
        if tracker.finalized.len() >= tracker.entry_limit {
            return Err(PorTrackerError::FinalizedRetentionExhausted {
                limit: tracker.entry_limit,
            });
        }
        let callback_value = before_commit(stats)?;
        let finalized = tracker
            .pending
            .remove(&verdict.challenge_id)
            .expect("validated PoR challenge must remain while write lock is held");
        tracker
            .finalized
            .insert(verdict.challenge_id, finalized.challenge);
        Ok((stats, callback_value))
    }

    /// Export pending and finalized challenge state in deterministic order.
    pub(crate) fn checkpoint(&self) -> PorTrackerCheckpointV1 {
        let tracker = self.inner.read().expect("por tracker poisoned");
        let mut pending = tracker.pending.values().cloned().collect::<Vec<_>>();
        pending.sort_by_key(|state| state.challenge.challenge_id);
        let mut finalized = tracker.finalized.values().cloned().collect::<Vec<_>>();
        finalized.sort_by_key(|challenge| challenge.challenge_id);
        PorTrackerCheckpointV1 { pending, finalized }
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
        let mut previous_finalized_id = None;
        for challenge in checkpoint.finalized {
            challenge
                .validate()
                .map_err(PorTrackerError::ChallengeInvalid)?;
            let challenge_id = challenge.challenge_id;
            if previous_finalized_id.is_some_and(|previous| previous >= challenge_id) {
                return Err(PorTrackerError::InvalidCheckpoint(
                    "finalized challenges must be strictly ordered by challenge id".to_owned(),
                ));
            }
            previous_finalized_id = Some(challenge_id);
            if finalized.insert(challenge_id, challenge).is_some() {
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
        tracker.pending = pending;
        tracker.finalized = finalized;
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
    #[error("finalized PoR challenge retention exhausted (limit {limit})")]
    FinalizedRetentionExhausted {
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Durable tracker checkpoint is malformed or internally inconsistent.
    #[error("invalid PoR tracker checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// Durable auxiliary runtime checkpoint could not be committed.
    #[error("PoR runtime checkpoint failed: {0}")]
    RuntimeCheckpoint(String),
    /// Exact provider-bond penalty arithmetic could not be represented.
    #[error("PoR penalty arithmetic failed: {0}")]
    PenaltyArithmetic(String),
    /// Challenge id is unknown to the tracker.
    #[error("unknown challenge id")]
    UnknownChallenge,
    /// Proof references a different manifest digest.
    #[error("proof manifest digest does not match recorded challenge")]
    MismatchManifest,
    /// Proof references a different provider id.
    #[error("proof provider id does not match recorded challenge")]
    MismatchProvider,
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
    /// Repair store failed while recording PoR failure history.
    #[error(transparent)]
    RepairStore(#[from] RepairStoreError),
}

#[cfg(test)]
/// Utilities used only in tests to build attested POR inputs.
pub mod test_support {
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use sorafs_manifest::{
        por::{AUDIT_VERDICT_VERSION_V1, POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1},
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };

    use super::*;

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
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, str::FromStr};

    use super::*;
    use crate::por::test_support::{
        resign_sample_proof, resign_sample_verdict, sample_auditor_keys, sample_challenge,
        sample_proof, sample_provider_key, sample_verdict,
    };
    use iroha_data_model::{metadata::Metadata, name::Name};
    use sorafs_car::{POR_LEAF_SIZE, PorMerkleTree, StoredChunk};

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
    fn tracker_refuses_pending_and_finalized_retention_exhaustion() {
        let tracker = PorTracker::with_entry_limit(1);
        let first = sample_challenge();
        let second = next_challenge(&first, 1);
        tracker.record_challenge(&first).unwrap();
        assert!(matches!(
            tracker.record_challenge(&second),
            Err(PorTrackerError::PendingRetentionExhausted { limit: 1 })
        ));

        let proof = sample_proof(&first);
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        tracker
            .record_verdict(
                &sample_verdict(&first, proof.proof_digest()),
                &sample_auditor_keys(),
                1,
            )
            .unwrap();
        tracker.record_challenge(&second).unwrap();
        let proof = sample_proof(&second);
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        assert!(matches!(
            tracker.record_verdict(
                &sample_verdict(&second, proof.proof_digest()),
                &sample_auditor_keys(),
                1,
            ),
            Err(PorTrackerError::FinalizedRetentionExhausted { limit: 1 })
        ));
        assert!(tracker.contains_challenge(&second.challenge_id));
    }

    #[test]
    fn tracker_checkpoint_preserves_pending_proofs_and_finalized_payloads() {
        let source = PorTracker::with_entry_limit(4);
        let finalized = sample_challenge();
        let finalized_proof = sample_proof(&finalized);
        source.record_challenge(&finalized).unwrap();
        source
            .record_proof(&finalized_proof, &sample_provider_key())
            .unwrap();
        source
            .record_verdict(
                &sample_verdict(&finalized, finalized_proof.proof_digest()),
                &sample_auditor_keys(),
                1,
            )
            .unwrap();
        let pending = next_challenge(&finalized, 1);
        let pending_proof = sample_proof(&pending);
        source.record_challenge(&pending).unwrap();
        source
            .record_proof(&pending_proof, &sample_provider_key())
            .unwrap();

        let checkpoint = source.checkpoint();
        let encoded = norito::to_bytes(&checkpoint).unwrap();
        let checkpoint = norito::decode_from_bytes(&encoded).unwrap();
        let restored = PorTracker::with_entry_limit(4);
        restored.restore_checkpoint(checkpoint).unwrap();
        restored
            .record_challenge(&finalized)
            .expect("restored finalized challenge is exactly idempotent");
        let mut conflicting = finalized.clone();
        conflicting.deadline_at = conflicting.deadline_at.saturating_add(1);
        assert!(matches!(
            restored.record_challenge(&conflicting),
            Err(PorTrackerError::ChallengeConflict)
        ));
        restored
            .record_verdict(
                &sample_verdict(&pending, pending_proof.proof_digest()),
                &sample_auditor_keys(),
                1,
            )
            .unwrap();
    }

    #[test]
    fn tracker_handles_failure_verdict() {
        let tracker = PorTracker::default();
        let mut challenge = sample_challenge();
        challenge.sample_count = 1;
        challenge.sample_indices = vec![0];
        tracker.record_challenge(&challenge).unwrap();
        let mut verdict = sample_verdict(&challenge, [1; 32]);
        verdict.proof_digest = None;
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.decided_at = 1_700_000_400;
        resign_sample_verdict(&mut verdict);
        let stats = tracker
            .record_verdict(&verdict, &sample_auditor_keys(), 1)
            .unwrap();
        assert_eq!(
            stats,
            PorVerdictStats {
                success_samples: 0,
                failed_samples: 1
            }
        );
    }

    #[test]
    fn tracker_detects_mismatched_proof() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        tracker.record_challenge(&challenge).unwrap();
        let mut proof = sample_proof(&challenge);
        proof.manifest_digest = [99; 32];
        resign_sample_proof(&mut proof);
        let err = tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap_err();
        assert!(matches!(err, PorTrackerError::MismatchManifest));
        assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);

        tracker
            .record_proof(&sample_proof(&challenge), &sample_provider_key())
            .expect("mismatched proof must not consume the challenge");
    }

    #[test]
    fn tracker_rejects_wrong_sample_coverage_and_late_or_predated_proofs() {
        let challenge = sample_challenge();
        for mutation in 0..3 {
            let tracker = PorTracker::default();
            tracker.record_challenge(&challenge).unwrap();
            let mut proof = sample_proof(&challenge);
            match mutation {
                0 => proof.samples.swap(0, 1),
                1 => proof.submitted_at = challenge.issued_at - 1,
                2 => proof.submitted_at = challenge.deadline_at + 1,
                _ => unreachable!(),
            }
            resign_sample_proof(&mut proof);

            let error = tracker
                .record_proof(&proof, &sample_provider_key())
                .expect_err("adversarial proof must fail");
            assert!(
                matches!(
                    (mutation, &error),
                    (0, PorTrackerError::SampleIndicesMismatch)
                        | (1 | 2, PorTrackerError::ProofOutsideChallengeWindow { .. })
                ),
                "unexpected mutation result {mutation}: {error:?}"
            );
            assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);
        }
    }

    #[test]
    fn tracker_rejects_cross_bound_verdict_without_consuming_challenge() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        let proof = sample_proof(&challenge);
        tracker.record_challenge(&challenge).unwrap();
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let valid = sample_verdict(&challenge, proof.proof_digest());

        for mutation in 0..5 {
            let mut forged = valid.clone();
            match mutation {
                0 => forged.provider_id[0] ^= 1,
                1 => forged.manifest_digest[0] ^= 1,
                2 => forged.proof_digest = Some([0xEE; 32]),
                3 => forged.proof_digest = None,
                4 => forged.decided_at = proof.submitted_at - 1,
                _ => unreachable!(),
            }
            resign_sample_verdict(&mut forged);
            assert!(
                tracker
                    .record_verdict(&forged, &sample_auditor_keys(), 1)
                    .is_err()
            );
            assert!(
                tracker.contains_challenge(&challenge.challenge_id),
                "mutation {mutation} must not consume challenge state"
            );
            assert_eq!(
                tracker.proof_digest(&challenge.challenge_id),
                Some(proof.proof_digest())
            );
        }

        tracker
            .record_verdict(&valid, &sample_auditor_keys(), 1)
            .expect("valid verdict remains retryable after forged attempts");
        assert!(!tracker.contains_challenge(&challenge.challenge_id));
    }

    #[test]
    fn tracker_enforces_provider_admission_and_auditor_threshold_at_commit_boundary() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        let proof = sample_proof(&challenge);
        tracker.record_challenge(&challenge).unwrap();

        assert!(matches!(
            tracker.record_proof(&proof, &[0xFE; 32]),
            Err(PorTrackerError::ProofSignatureInvalid(
                sorafs_manifest::por::PorSignatureVerificationError::ProviderSignerMismatch
            ))
        ));
        assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);
        tracker
            .record_proof(&proof, &sample_provider_key())
            .expect("admitted provider proof");

        let verdict = sample_verdict(&challenge, proof.proof_digest());
        assert!(matches!(
            tracker.record_verdict(&verdict, &[vec![0xFD; 32]], 1),
            Err(PorTrackerError::VerdictSignatureInvalid(
                sorafs_manifest::por::PorSignatureVerificationError::UntrustedAuditorSigner
            ))
        ));
        let mut two_auditors = sample_auditor_keys();
        two_auditors.push(vec![0xFC; 32]);
        assert!(matches!(
            tracker.record_verdict(&verdict, &two_auditors, 2),
            Err(PorTrackerError::VerdictSignatureInvalid(
                sorafs_manifest::por::PorSignatureVerificationError::InsufficientTrustedAuditorSignatures {
                    actual: 1,
                    required: 2,
                }
            ))
        ));
        assert!(tracker.contains_challenge(&challenge.challenge_id));
        tracker
            .record_verdict(&verdict, &sample_auditor_keys(), 1)
            .expect("trusted auditor threshold");
    }

    #[test]
    fn tracker_requires_proof_for_success_but_allows_failure_without_one() {
        let challenge = sample_challenge();
        let tracker = PorTracker::default();
        tracker.record_challenge(&challenge).unwrap();
        let success = sample_verdict(&challenge, [0x55; 32]);
        assert!(matches!(
            tracker.record_verdict(&success, &sample_auditor_keys(), 1),
            Err(PorTrackerError::UnexpectedVerdictProofDigest)
        ));
        assert!(tracker.contains_challenge(&challenge.challenge_id));

        let mut success_without_digest = success.clone();
        success_without_digest.proof_digest = None;
        resign_sample_verdict(&mut success_without_digest);
        assert!(matches!(
            tracker.record_verdict(&success_without_digest, &sample_auditor_keys(), 1),
            Err(PorTrackerError::MissingProofForSuccessfulVerdict)
        ));

        let mut failure = success_without_digest;
        failure.outcome = AuditOutcomeV1::Failed;
        failure.failure_reason = Some("provider missed deadline".to_owned());
        resign_sample_verdict(&mut failure);
        tracker
            .record_verdict(&failure, &sample_auditor_keys(), 1)
            .expect("failure without proof is a valid terminal transition");
    }

    #[test]
    fn tracker_callback_failure_is_atomic_and_retryable() {
        let tracker = PorTracker::default();
        let challenge = sample_challenge();
        let proof = sample_proof(&challenge);
        tracker.record_challenge(&challenge).unwrap();
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let verdict = sample_verdict(&challenge, proof.proof_digest());

        let error = tracker
            .record_verdict_with(&verdict, &sample_auditor_keys(), 1, |_| {
                Err::<(), _>(RepairStoreError::Other(
                    "injected durable-store failure".to_owned(),
                ))
            })
            .expect_err("injected callback failure must abort transition");
        assert!(matches!(error, PorTrackerError::RepairStore(_)));
        assert!(tracker.contains_challenge(&challenge.challenge_id));
        assert_eq!(
            tracker.proof_digest(&challenge.challenge_id),
            Some(proof.proof_digest())
        );

        tracker
            .record_verdict(&verdict, &sample_auditor_keys(), 1)
            .expect("verdict must succeed after durable store recovers");
    }
}
