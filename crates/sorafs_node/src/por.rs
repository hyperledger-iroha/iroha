//! PoR challenge/proof tracking for the embedded storage node.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use iroha_data_model::metadata::Metadata;
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

/// Randomness bundle sourced for a PoR epoch.
#[derive(Debug, Clone)]
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
    pub drand_signature: Vec<u8>,
}

/// Provider VRF output/proof for a manifest/epoch pair.
#[derive(Debug, Clone)]
pub struct ManifestVrfBundle {
    /// VRF output bytes.
    pub output: [u8; 32],
    /// Proof bytes attesting to the VRF output.
    pub proof: Vec<u8>,
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
    /// drand signature missing from randomness bundle.
    #[error("drand signature must be provided")]
    MissingDrandSignature,
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
) -> Result<PlannedChallenge, PorChallengePlannerError> {
    if randomness.drand_signature.is_empty() {
        return Err(PorChallengePlannerError::MissingDrandSignature);
    }

    let chunk_profile = manifest.chunk_profile_handle();
    if chunk_profile.is_empty() {
        return Err(PorChallengePlannerError::EmptyChunkProfile);
    }

    let multiplier = policy.multiplier_for(chunk_profile);
    let plan = determine_sample_plan(manifest.content_length(), multiplier);
    let (vrf_output, vrf_proof, forced) = match vrf {
        Some(bundle) => (Some(bundle.output), Some(bundle.proof.clone()), false),
        None => (None, None, true),
    };

    let manifest_digest = *manifest.manifest_digest();
    let seed = derive_challenge_seed(
        &randomness.drand_randomness,
        vrf_output.as_ref(),
        &manifest_digest,
        randomness.epoch_id,
    );
    let selection = sample_leaf_indices(&manifest.por_tree(), seed, plan)?;
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
        drand_signature: randomness.drand_signature.clone(),
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

#[derive(Debug, Clone)]
struct ChallengeState {
    challenge: PorChallengeV1,
    proof_digest: Option<[u8; 32]>,
}

/// Tracks the lifecycle of PoR challenges, proofs, and verdicts.
#[derive(Debug, Default, Clone)]
pub struct PorTracker {
    inner: Arc<RwLock<HashMap<[u8; 32], ChallengeState>>>,
}

impl PorTracker {
    /// Register a new PoR challenge.
    pub fn record_challenge(&self, challenge: &PorChallengeV1) -> Result<(), PorTrackerError> {
        challenge
            .validate()
            .map_err(PorTrackerError::ChallengeInvalid)?;
        let mut map = self.inner.write().expect("por tracker poisoned");
        let entry = map.entry(challenge.challenge_id);
        match entry {
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(ChallengeState {
                    challenge: challenge.clone(),
                    proof_digest: None,
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

    /// Register a PoR proof response.
    pub fn record_proof(&self, proof: &PorProofV1) -> Result<(), PorTrackerError> {
        proof.validate().map_err(PorTrackerError::ProofInvalid)?;
        let mut map = self.inner.write().expect("por tracker poisoned");
        let state = map
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
                actual: proof.samples.len() as u16,
            });
        }
        if state.proof_digest.is_some() {
            return Err(PorTrackerError::DuplicateProof);
        }
        state.proof_digest = Some(proof.proof_digest());
        Ok(())
    }

    /// Finalise a challenge using an audit verdict.
    pub fn record_verdict(
        &self,
        verdict: &AuditVerdictV1,
    ) -> Result<PorVerdictStats, PorTrackerError> {
        verdict
            .validate()
            .map_err(PorTrackerError::VerdictInvalid)?;
        let mut map = self.inner.write().expect("por tracker poisoned");
        let state = map
            .remove(&verdict.challenge_id)
            .ok_or(PorTrackerError::UnknownChallenge)?;
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
        if let Some(ref expected_digest) = state.proof_digest
            && let Some(ref verdict_digest) = verdict.proof_digest
            && verdict_digest != expected_digest
        {
            return Err(PorTrackerError::ProofDigestMismatch);
        }
        let samples = u64::from(state.challenge.sample_count);
        let stats = match verdict.outcome {
            AuditOutcomeV1::Success | AuditOutcomeV1::Repaired => PorVerdictStats {
                success_samples: samples,
                failed_samples: 0,
            },
            AuditOutcomeV1::Failed => PorVerdictStats {
                success_samples: 0,
                failed_samples: samples,
            },
        };
        Ok(stats)
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

        let map = self.inner.read().expect("por tracker poisoned");
        let mut grouped: HashMap<([u8; 32], [u8; 32]), PorBacklogEntry> = HashMap::new();
        for state in map.values() {
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
    /// Audit verdict failed validation.
    #[error("verdict invalid: {0}")]
    VerdictInvalid(#[source] sorafs_manifest::por::AuditVerdictValidationError),
    /// Challenge already recorded with differing payload.
    #[error("challenge with identical id already exists")]
    ChallengeConflict,
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
    /// Proof already recorded for the challenge.
    #[error("proof already recorded for this challenge")]
    DuplicateProof,
    /// Verdict proof digest does not match the previously recorded proof.
    #[error("proof digest reported by verdict does not match recorded proof")]
    ProofDigestMismatch,
    /// Repair store failed while recording PoR failure history.
    #[error(transparent)]
    RepairStore(#[from] RepairStoreError),
}

#[cfg(test)]
/// Utilities used only in tests to build attested POR inputs.
pub mod test_support {
    use sorafs_manifest::{
        por::{AUDIT_VERDICT_VERSION_V1, POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1},
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };

    use super::*;

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
            drand_signature: vec![0x61; 96],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(vec![0x71; 80]),
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
        PorProofV1 {
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
                public_key: vec![11; 32],
                signature: vec![12; 64],
            },
            submitted_at: 1_700_000_100,
        }
    }

    /// Deterministic verdict helper stitched to [`sample_challenge`].
    pub fn sample_verdict(challenge: &PorChallengeV1, digest: [u8; 32]) -> AuditVerdictV1 {
        AuditVerdictV1 {
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
                public_key: vec![13; 32],
                signature: vec![14; 64],
            }],
            metadata: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, str::FromStr};

    use iroha_data_model::{metadata::Metadata, name::Name};
    use sorafs_car::{PorChunkTree, PorLeaf, PorMerkleTree, PorSegment};
    use sorafs_manifest::{
        por::AUDIT_VERDICT_VERSION_V1,
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };

    use super::*;
    use crate::por::test_support::{sample_challenge, sample_proof, sample_verdict};

    const LARGE_LEAF_LEN: u32 = 64 * 1024;

    fn build_mock_tree(small_count: usize, large_count: usize) -> PorMerkleTree {
        let mut segments = Vec::with_capacity(small_count + large_count);
        let mut offset = 0u64;
        for _ in 0..small_count {
            segments.push(PorSegment {
                offset,
                length: SMALL_LEAF_MAX_LEN,
                digest: [0u8; 32],
                leaves: vec![PorLeaf {
                    offset,
                    length: SMALL_LEAF_MAX_LEN,
                    digest: [0u8; 32],
                }],
            });
            offset = offset.saturating_add(u64::from(SMALL_LEAF_MAX_LEN));
        }
        for _ in 0..large_count {
            segments.push(PorSegment {
                offset,
                length: LARGE_LEAF_LEN,
                digest: [1u8; 32],
                leaves: vec![PorLeaf {
                    offset,
                    length: LARGE_LEAF_LEN,
                    digest: [1u8; 32],
                }],
            });
            offset = offset.saturating_add(u64::from(LARGE_LEAF_LEN));
        }
        let total_len = offset;
        let chunk = PorChunkTree {
            chunk_index: 0,
            offset: 0,
            length: u32::try_from(total_len).expect("total length fits in u32"),
            chunk_digest: [0u8; 32],
            root: [0u8; 32],
            segments,
        };
        PorMerkleTree::from_chunks(vec![chunk], vec![[0u8; 32]], total_len)
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
        tracker.record_proof(&proof).unwrap();
        let verdict = sample_verdict(&challenge, digest);
        let stats = tracker.record_verdict(&verdict).unwrap();
        assert_eq!(
            stats,
            PorVerdictStats {
                success_samples: 2,
                failed_samples: 0
            }
        );
    }

    #[test]
    fn tracker_handles_failure_verdict() {
        let tracker = PorTracker::default();
        let mut challenge = sample_challenge();
        challenge.sample_count = 1;
        challenge.sample_indices = vec![0];
        tracker.record_challenge(&challenge).unwrap();
        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            challenge_id: challenge.challenge_id,
            proof_digest: None,
            outcome: AuditOutcomeV1::Failed,
            failure_reason: Some("timeout".to_string()),
            decided_at: 1_700_000_400,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![21; 32],
                signature: vec![22; 64],
            }],
            metadata: Vec::new(),
        };
        let stats = tracker.record_verdict(&verdict).unwrap();
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
        let err = tracker.record_proof(&proof).unwrap_err();
        matches!(err, PorTrackerError::MismatchManifest);
    }
}
