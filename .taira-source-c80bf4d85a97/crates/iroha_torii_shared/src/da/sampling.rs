//! Deterministic sampling and validator assignment helpers for data availability.
//!
//! The sampler derives a reproducible chunk selection and assignment hash from
//! the block hash and manifest identifiers so Torii, validators, and SDKs share
//! the same probe set.

use iroha_crypto::Hash;
use iroha_data_model::da::{
    manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
    types::BlobDigest,
};
use norito::json::{Map, Value};
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};

/// Summary of the deterministic sampling plan for a DA manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaSamplingPlan {
    /// Hash binding the validator assignment to the block/manifest.
    pub assignment_hash: BlobDigest,
    /// Number of samples that should be probed for this manifest.
    pub sample_window: u16,
    /// Seed used to derive the shuffled sample set.
    pub sample_seed: [u8; 32],
    /// Chunks selected for probing.
    pub samples: Vec<DaSampledChunk>,
}

impl DaSamplingPlan {
    /// Derive a 64-bit seed suitable for `PoR` leaf sampling from the plan seed.
    #[must_use]
    pub fn por_seed(&self) -> u64 {
        u64::from_le_bytes(
            self.sample_seed[..8]
                .try_into()
                .expect("seed length enforced"),
        )
    }
}

/// Individual sampled chunk entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DaSampledChunk {
    /// Chunk index within the manifest.
    pub chunk_index: u32,
    /// Role of the chunk in the erasure layout.
    pub role: ChunkRole,
    /// Stripe/group identifier (row/column index for parity roles).
    pub group_id: u32,
}

/// Compute how many samples should be taken for the given payload size.
///
/// The window scales with the payload size but is clamped to `[32, 256]`.
#[must_use]
pub fn compute_sample_window(total_size: u64) -> u16 {
    const CHUNK_UNIT: u64 = 64 * 1024 * 1024;
    const MIN_SAMPLES: u64 = 32;
    const MAX_SAMPLES: u64 = 256;

    if total_size == 0 {
        return u16::try_from(MIN_SAMPLES).expect("min samples fits in u16");
    }
    let buckets = total_size.div_ceil(CHUNK_UNIT);
    let clamped = buckets.clamp(MIN_SAMPLES, MAX_SAMPLES);
    u16::try_from(clamped).unwrap_or(u16::MAX)
}

/// Build a deterministic sampling plan from the manifest and block hash.
///
/// # Panics
///
/// Panics if the manifest advertises more than `u32::MAX` chunks.
#[must_use]
pub fn build_sampling_plan(manifest: &DaManifestV1, block_hash: &Hash) -> DaSamplingPlan {
    let sample_window = compute_sample_window(manifest.total_size);
    let chunk_count = manifest.chunks.len();
    let sample_count = usize::min(chunk_count, sample_window as usize);

    let seed = derive_sampling_seed(block_hash, &manifest.client_blob_id);
    let mut rng = StdRng::from_seed(seed);
    let mut positions: Vec<usize> = (0..chunk_count).collect();
    positions.shuffle(&mut rng);
    positions.truncate(sample_count);
    positions.sort_unstable();

    let samples = positions
        .into_iter()
        .filter_map(|pos| manifest.chunks.get(pos))
        .map(|chunk| DaSampledChunk {
            chunk_index: chunk.index,
            role: effective_chunk_role(chunk),
            group_id: chunk.group_id,
        })
        .collect();

    DaSamplingPlan {
        assignment_hash: derive_assignment_hash(block_hash, manifest),
        sample_window,
        sample_seed: seed,
        samples,
    }
}

fn chunk_role_label(role: ChunkRole) -> &'static str {
    match role {
        ChunkRole::Data => "data",
        ChunkRole::LocalParity => "local_parity",
        ChunkRole::GlobalParity => "global_parity",
        ChunkRole::StripeParity => "stripe_parity",
    }
}

/// Render a sampling plan into JSON for HTTP responses.
#[must_use]
pub fn sampling_plan_to_value(plan: &DaSamplingPlan) -> Value {
    let mut samples = Vec::with_capacity(plan.samples.len());
    for sample in &plan.samples {
        let mut entry = Map::new();
        entry.insert("index".into(), Value::from(sample.chunk_index));
        entry.insert("role".into(), Value::from(chunk_role_label(sample.role)));
        entry.insert("group".into(), Value::from(sample.group_id));
        samples.push(Value::Object(entry));
    }

    let mut root = Map::new();
    root.insert(
        "assignment_hash".into(),
        Value::from(hex::encode(plan.assignment_hash.as_bytes())),
    );
    root.insert("sample_window".into(), Value::from(plan.sample_window));
    root.insert(
        "sample_seed".into(),
        Value::from(hex::encode(plan.sample_seed)),
    );
    root.insert("samples".into(), Value::Array(samples));
    Value::Object(root)
}

fn effective_chunk_role(commitment: &ChunkCommitment) -> ChunkRole {
    if commitment.parity && matches!(commitment.role, ChunkRole::Data) {
        ChunkRole::GlobalParity
    } else {
        commitment.role
    }
}

fn derive_sampling_seed(block_hash: &Hash, client_blob_id: &BlobDigest) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(block_hash.as_ref());
    hasher.update(client_blob_id.as_bytes());
    *hasher.finalize().as_bytes()
}

fn derive_assignment_hash(block_hash: &Hash, manifest: &DaManifestV1) -> BlobDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(block_hash.as_ref());
    hasher.update(manifest.client_blob_id.as_bytes());
    hasher.update(manifest.chunk_root.as_bytes());
    hasher.update(manifest.ipa_commitment.as_bytes());
    BlobDigest::from_hash(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use iroha_data_model::{
        da::{
            manifest::ChunkRole,
            types::{
                BlobClass, BlobCodec, DaRentQuote, ErasureProfile, ExtraMetadata, RetentionPolicy,
                StorageTicketId,
            },
        },
        nexus::LaneId,
    };

    use super::*;

    fn sample_manifest(chunk_count: u32, total_size: u64) -> DaManifestV1 {
        let chunks = (0..chunk_count)
            .map(|idx| {
                let idx_byte = u8::try_from(idx).expect("chunk index fits in u8");
                ChunkCommitment::new_with_role(
                    idx,
                    u64::from(idx) * 1024,
                    1024,
                    BlobDigest::new([idx_byte; 32]),
                    if idx % 5 == 0 {
                        ChunkRole::GlobalParity
                    } else {
                        ChunkRole::Data
                    },
                    idx / 2,
                )
            })
            .collect();

        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0xAA; 32]),
            lane_id: LaneId::new(7),
            epoch: 1,
            blob_class: BlobClass::NexusLaneSidecar,
            codec: BlobCodec::new("test/binary"),
            blob_hash: BlobDigest::new([0xBB; 32]),
            chunk_root: BlobDigest::new([0xCC; 32]),
            storage_ticket: StorageTicketId::new([0xDD; 32]),
            total_size,
            chunk_size: 1024,
            total_stripes: chunk_count,
            shards_per_stripe: 6,
            erasure_profile: ErasureProfile::default(),
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks,
            ipa_commitment: BlobDigest::new([0xEF; 32]),
            metadata: ExtraMetadata::default(),
            issued_at_unix: 1_701_000_000,
        }
    }

    #[test]
    fn sample_window_clamps_and_scales() {
        assert_eq!(compute_sample_window(0), 32);
        assert_eq!(compute_sample_window(1), 32);
        assert_eq!(compute_sample_window(64 * 1024 * 1024), 32);
        assert_eq!(compute_sample_window(65 * 1024 * 1024), 32);
        assert_eq!(compute_sample_window(1024 * 1024 * 1024 * 1024), 256);
    }

    #[test]
    fn plan_respects_window_and_has_unique_samples() {
        let manifest = sample_manifest(20, 64 * 1024 * 1024 * 5);
        let block_hash = Hash::new(b"block-assign-clamp");
        let plan = build_sampling_plan(&manifest, &block_hash);
        let expected_window = compute_sample_window(manifest.total_size);
        let expected_len = usize::min(manifest.chunks.len(), expected_window as usize);

        assert_eq!(plan.sample_window, expected_window);
        assert_eq!(plan.samples.len(), expected_len);

        let mut seen = HashSet::new();
        for sample in &plan.samples {
            assert!(seen.insert(sample.chunk_index));
        }
    }

    #[test]
    fn plan_is_deterministic_for_same_seed() {
        let manifest = sample_manifest(6, 2 * 1024 * 1024);
        let block_hash = Hash::new(b"block-deterministic");
        let first = build_sampling_plan(&manifest, &block_hash);
        let second = build_sampling_plan(&manifest, &block_hash);

        assert_eq!(first.assignment_hash, second.assignment_hash);
        assert_eq!(first.samples, second.samples);
        assert_eq!(first.sample_seed, second.sample_seed);
        assert_eq!(first.por_seed(), second.por_seed());
    }

    #[test]
    fn plan_changes_with_block_hash() {
        let manifest = sample_manifest(6, 2 * 1024 * 1024);
        let first = build_sampling_plan(&manifest, &Hash::new(b"block-a"));
        let second = build_sampling_plan(&manifest, &Hash::new(b"block-b"));

        assert_ne!(first.assignment_hash, second.assignment_hash);
        assert_ne!(first.sample_seed, second.sample_seed);
    }
}
