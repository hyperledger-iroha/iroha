//! Owner-bound, public synthetic cost samples for qualified Metal work.

use super::{MetalMerkleWork, SimdChoice};
use crate::{byte_merkle_tree::sha256_oneblock32, cache_memory::MemoryReservation};
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_crypto::MerkleTree;
use rayon::prelude::*;
use std::time::{Duration, Instant};

const SAMPLE_LEAVES: [usize; 4] = [8_192, 16_384, 32_768, 65_536];
const TRIALS: usize = 3;
const MAX_CALIBRATION: Duration = Duration::from_secs(8);
const RETRY_AFTER_TRANSIENT_FAILURE: Duration = Duration::from_secs(30);
// Reserve before constructing the largest public sample and its CPU/GPU
// buffers. This reservation is active work and never enters cache retention.
const CALIBRATION_RESERVATION_BYTES: usize = 32 * 1024 * 1024;
pub(super) const BATCH_FAMILIES: usize = 5;
const AES_BATCH_ITEMS: [usize; 4] = [32, 128, 512, 2_048];
const ED25519_BATCH_ITEMS: [usize; 4] = [16, 32, 128, 512];
const BATCH_CALIBRATION_RESERVATION_BYTES: usize = 4 * 1024 * 1024;
const BATCH_MAX_ROUNDS: usize = 64;

#[derive(Clone, Copy, Debug, Default)]
struct CostSample {
    leaves: usize,
    cpu_leaves_ns: u64,
    metal_leaves_ns: u64,
    cpu_root_ns: u64,
    metal_root_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CalibrationFailure {
    /// Public samples exceeded their bounded wall-time budget.
    Deadline,
    /// Synthetic sample allocation or CPU tree construction failed.
    Allocation,
    /// A qualified Metal operation could not complete this attempt.
    BackendUnavailable,
    /// Timing samples varied too much for a reliable path decision.
    Overloaded,
    /// A completed Metal result disagreed with the CPU reference.
    ParityMismatch,
}

/// Successful profiles live for the owner lifetime; transient failures only
/// delay the next bounded attempt. Callers use a nonblocking lock and run on
/// CPU while another thread calibrates or the retry cooldown is active.
#[derive(Default)]
pub(super) struct MetalMerkleCostCache {
    profile: Option<MetalMerkleCostProfile>,
    retry_after: Option<Instant>,
    quarantined: bool,
}

impl MetalMerkleCostCache {
    /// Return the qualified profile or schedule a bounded retry after a transient failure.
    pub(super) fn get_or_calibrate(
        &mut self,
        now: Instant,
        run: impl FnOnce() -> Result<MetalMerkleCostProfile, CalibrationFailure>,
    ) -> Option<MetalMerkleCostProfile> {
        if let Some(profile) = self.profile {
            if profile.cpu_choice == super::simd_choice() {
                return Some(profile);
            }
            // A file-configured CPU/SIMD policy change changes the comparison
            // baseline. Requalify rather than freezing a stale cost decision.
            self.profile = None;
            self.retry_after = None;
        }
        if self.quarantined || self.retry_after.is_some_and(|deadline| now < deadline) {
            return None;
        }
        match run() {
            Ok(profile) => {
                self.profile = Some(profile);
                self.retry_after = None;
                Some(profile)
            }
            Err(CalibrationFailure::ParityMismatch) => {
                self.quarantined = true;
                None
            }
            Err(
                CalibrationFailure::Deadline
                | CalibrationFailure::Allocation
                | CalibrationFailure::BackendUnavailable
                | CalibrationFailure::Overloaded,
            ) => {
                self.retry_after = Some(now + RETRY_AFTER_TRANSIENT_FAILURE);
                None
            }
        }
    }
}

/// A local profile that cannot be supplied by a transaction or configuration.
///
/// Its private constructor runs only after the owning Metal state passed its
/// kernel self-tests. Every measured output must also equal the CPU result.
#[derive(Clone, Copy, Debug)]
pub(super) struct MetalMerkleCostProfile {
    samples: [CostSample; SAMPLE_LEAVES.len()],
    cpu_choice: SimdChoice,
}

impl MetalMerkleCostProfile {
    pub(super) fn prefer_metal(self, work: MetalMerkleWork, leaves: usize) -> bool {
        if self.cpu_choice != super::simd_choice() || leaves < SAMPLE_LEAVES[0] {
            return false;
        }
        let index = self
            .samples
            .partition_point(|sample| sample.leaves <= leaves)
            .saturating_sub(1);
        let sample = self.samples[index];
        match work {
            MetalMerkleWork::Leaves => sample.metal_leaves_ns < sample.cpu_leaves_ns,
            MetalMerkleWork::Root => sample.metal_root_ns < sample.cpu_root_ns,
        }
    }
}

fn synthetic_data(leaves: usize) -> Option<Vec<u8>> {
    let bytes = leaves.checked_mul(32)?;
    let mut data = Vec::new();
    data.try_reserve_exact(bytes).ok()?;
    data.resize(bytes, 0);
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = (index as u8)
            .wrapping_mul(31)
            .wrapping_add((index >> 8) as u8)
            .wrapping_add(7);
    }
    Some(data)
}

fn padded_blocks(data: &[u8]) -> Option<Vec<[u8; 64]>> {
    let mut blocks = Vec::new();
    blocks.try_reserve_exact(data.len() / 32).ok()?;
    for chunk in data.chunks_exact(32) {
        let mut block = [0u8; 64];
        block[..32].copy_from_slice(chunk);
        block[32] = 0x80;
        block[56..64].copy_from_slice(&256u64.to_be_bytes());
        blocks.push(block);
    }
    Some(blocks)
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn median(mut values: [u64; TRIALS]) -> u64 {
    values.sort_unstable();
    values[TRIALS / 2]
}

fn stable_trials(values: [u64; TRIALS]) -> bool {
    let minimum = values.into_iter().min().expect("three fixed trials");
    let maximum = values.into_iter().max().expect("three fixed trials");
    minimum > 0 && maximum <= minimum.saturating_mul(8)
}

/// A distinct, qualified pipeline family. The round count is public call
/// geometry; no signature, key, state byte, or witness value enters selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetalBatchWork {
    AesEnc,
    AesDec,
    AesEncRounds(usize),
    AesDecRounds(usize),
    Ed25519,
}

impl MetalBatchWork {
    pub(super) const fn family_index(self) -> usize {
        match self {
            Self::AesEnc => 0,
            Self::AesDec => 1,
            Self::AesEncRounds(_) => 2,
            Self::AesDecRounds(_) => 3,
            Self::Ed25519 => 4,
        }
    }

    pub(super) const fn calibration_supported(self) -> bool {
        match self {
            Self::AesEncRounds(rounds) | Self::AesDecRounds(rounds) => {
                rounds > 0 && rounds <= BATCH_MAX_ROUNDS
            }
            Self::AesEnc | Self::AesDec | Self::Ed25519 => true,
        }
    }

    pub(super) const fn min_items(self) -> usize {
        match self {
            Self::Ed25519 => ED25519_BATCH_ITEMS[0],
            Self::AesEnc | Self::AesDec | Self::AesEncRounds(_) | Self::AesDecRounds(_) => {
                AES_BATCH_ITEMS[0]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct BatchCostSample {
    items: usize,
    cpu_ns: u64,
    metal_ns: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MetalBatchCostProfile {
    work: MetalBatchWork,
    samples: [BatchCostSample; AES_BATCH_ITEMS.len()],
    cpu_choice: SimdChoice,
}

impl MetalBatchCostProfile {
    pub(super) fn prefer_metal(self, items: usize) -> bool {
        if self.cpu_choice != super::simd_choice() || items < self.samples[0].items {
            return false;
        }
        let upper = self.samples.partition_point(|sample| sample.items < items);
        let (cpu_ns, metal_ns) = if upper == 0 {
            (self.samples[0].cpu_ns, self.samples[0].metal_ns)
        } else if upper == self.samples.len() {
            let last = self.samples[self.samples.len() - 1];
            (last.cpu_ns, last.metal_ns)
        } else {
            let lower = self.samples[upper - 1];
            let higher = self.samples[upper];
            let interpolate = |low: u64, high: u64| {
                (u128::from(low)
                    + u128::from(high.saturating_sub(low)) * (items - lower.items) as u128
                        / (higher.items - lower.items) as u128)
                    .min(u128::from(u64::MAX)) as u64
            };
            (
                interpolate(lower.cpu_ns, higher.cpu_ns),
                interpolate(lower.metal_ns, higher.metal_ns),
            )
        };
        // Require a clear measured win so close/noisy samples stay on CPU.
        u128::from(metal_ns) * 10 < u128::from(cpu_ns) * 9
    }
}

/// A profile belongs to the process-lived, self-tested Metal owner. Changing
/// the public fused-round geometry or CPU policy requires another comparison.
#[derive(Default)]
pub(super) struct MetalBatchCostCache {
    profile: Option<MetalBatchCostProfile>,
    retry_after: Option<Instant>,
    last_calibration: Option<Instant>,
    quarantined: bool,
}

impl MetalBatchCostCache {
    pub(super) fn get_or_calibrate(
        &mut self,
        now: Instant,
        work: MetalBatchWork,
        run: impl FnOnce() -> Result<MetalBatchCostProfile, CalibrationFailure>,
    ) -> Option<MetalBatchCostProfile> {
        if let Some(profile) = self.profile {
            if profile.cpu_choice != super::simd_choice() {
                self.profile = None;
                self.last_calibration = None;
                self.retry_after = None;
            } else if profile.work == work {
                return Some(profile);
            }
        }
        if self.quarantined
            || self.retry_after.is_some_and(|deadline| now < deadline)
            || self.last_calibration.is_some_and(|last| {
                now.saturating_duration_since(last) < RETRY_AFTER_TRANSIENT_FAILURE
            })
        {
            return None;
        }
        self.last_calibration = Some(now);
        match run() {
            Ok(profile) if profile.work == work && profile.cpu_choice == super::simd_choice() => {
                self.profile = Some(profile);
                self.retry_after = None;
                Some(profile)
            }
            Ok(_) | Err(CalibrationFailure::ParityMismatch) => {
                self.quarantined = true;
                None
            }
            Err(
                CalibrationFailure::Deadline
                | CalibrationFailure::Allocation
                | CalibrationFailure::BackendUnavailable
                | CalibrationFailure::Overloaded,
            ) => {
                self.retry_after = Some(now + RETRY_AFTER_TRANSIENT_FAILURE);
                None
            }
        }
    }
}

fn synthetic_aes_blocks(items: usize) -> Option<Vec<[u8; 16]>> {
    let mut blocks = Vec::new();
    blocks.try_reserve_exact(items).ok()?;
    for index in 0..items {
        let mut block = [0u8; 16];
        for (byte_index, byte) in block.iter_mut().enumerate() {
            *byte = (index as u8)
                .wrapping_mul(37)
                .wrapping_add(byte_index as u8);
        }
        blocks.push(block);
    }
    Some(blocks)
}

fn synthetic_aes_keys(rounds: usize) -> Option<Vec<[u8; 16]>> {
    let mut keys = Vec::new();
    keys.try_reserve_exact(rounds).ok()?;
    for round in 0..rounds {
        let mut key = [0u8; 16];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = (round as u8)
                .wrapping_mul(19)
                .wrapping_add(index as u8)
                .wrapping_add(11);
        }
        keys.push(key);
    }
    Some(keys)
}

fn aes_cpu_result(work: MetalBatchWork, blocks: &[[u8; 16]], keys: &[[u8; 16]]) -> Vec<[u8; 16]> {
    let mut result = blocks.to_vec();
    for &key in keys {
        for block in &mut result {
            *block = match work {
                MetalBatchWork::AesEnc | MetalBatchWork::AesEncRounds(_) => {
                    crate::aes::aesenc(*block, key)
                }
                MetalBatchWork::AesDec | MetalBatchWork::AesDecRounds(_) => {
                    crate::aes::aesdec(*block, key)
                }
                MetalBatchWork::Ed25519 => unreachable!("Ed25519 has no AES round keys"),
            };
        }
    }
    result
}

fn aes_metal_into(
    work: MetalBatchWork,
    blocks: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    let (decrypt, fused) = match work {
        MetalBatchWork::AesEnc => (false, false),
        MetalBatchWork::AesDec => (true, false),
        MetalBatchWork::AesEncRounds(_) => (false, true),
        MetalBatchWork::AesDecRounds(_) => (true, true),
        MetalBatchWork::Ed25519 => return false,
    };
    super::metal_aes::with_receipt_into(blocks, keys, destination, decrypt, fused, None)
}

fn calibrate_aes(
    work: MetalBatchWork,
    started: Instant,
) -> Result<[BatchCostSample; AES_BATCH_ITEMS.len()], CalibrationFailure> {
    let rounds = match work {
        MetalBatchWork::AesEnc | MetalBatchWork::AesDec => 1,
        MetalBatchWork::AesEncRounds(rounds) | MetalBatchWork::AesDecRounds(rounds) => rounds,
        MetalBatchWork::Ed25519 => unreachable!("Ed25519 uses its own calibration"),
    };
    if !work.calibration_supported() {
        return Err(CalibrationFailure::BackendUnavailable);
    }
    let keys = synthetic_aes_keys(rounds).ok_or(CalibrationFailure::Allocation)?;
    let mut samples = [BatchCostSample::default(); AES_BATCH_ITEMS.len()];
    for (index, items) in AES_BATCH_ITEMS.into_iter().enumerate() {
        let blocks = synthetic_aes_blocks(items).ok_or(CalibrationFailure::Allocation)?;
        let mut cpu_ns = [0; TRIALS];
        let mut metal_ns = [0; TRIALS];
        for trial in 0..TRIALS {
            if started.elapsed() >= MAX_CALIBRATION {
                return Err(CalibrationFailure::Deadline);
            }
            let cpu_start = Instant::now();
            let expected = aes_cpu_result(work, &blocks, &keys);
            cpu_ns[trial] = elapsed_ns(cpu_start);
            let mut actual = Vec::new();
            actual
                .try_reserve_exact(items)
                .map_err(|_| CalibrationFailure::Allocation)?;
            actual.resize(items, [0; 16]);
            let metal_start = Instant::now();
            if !aes_metal_into(work, &blocks, &keys, &mut actual) {
                return Err(CalibrationFailure::BackendUnavailable);
            }
            metal_ns[trial] = elapsed_ns(metal_start);
            if actual != expected {
                super::record_metal_disable("synthetic AES batch parity mismatch");
                return Err(CalibrationFailure::ParityMismatch);
            }
        }
        if !stable_trials(cpu_ns) || !stable_trials(metal_ns) {
            return Err(CalibrationFailure::Overloaded);
        }
        samples[index] = BatchCostSample {
            items,
            cpu_ns: median(cpu_ns),
            metal_ns: median(metal_ns),
        };
    }
    Ok(samples)
}

fn calibrate_ed25519(
    started: Instant,
) -> Result<[BatchCostSample; ED25519_BATCH_ITEMS.len()], CalibrationFailure> {
    let signing_key = SigningKey::from_bytes(&[0x43; 32]);
    let public_key = signing_key.verifying_key().to_bytes();
    let mut samples = [BatchCostSample::default(); ED25519_BATCH_ITEMS.len()];
    for (index, items) in ED25519_BATCH_ITEMS.into_iter().enumerate() {
        let mut messages = Vec::new();
        let mut signatures = Vec::new();
        let mut public_keys = Vec::new();
        messages
            .try_reserve_exact(items)
            .map_err(|_| CalibrationFailure::Allocation)?;
        signatures
            .try_reserve_exact(items)
            .map_err(|_| CalibrationFailure::Allocation)?;
        public_keys
            .try_reserve_exact(items)
            .map_err(|_| CalibrationFailure::Allocation)?;
        for item in 0..items {
            let mut message = [0u8; 32];
            message[..8].copy_from_slice(&(item as u64).to_le_bytes());
            let signature = signing_key.sign(&message).to_bytes();
            if item % 4 == 3 {
                message[8] ^= 1; // Structurally valid but false verification.
            }
            messages.push(message);
            signatures.push(signature);
            public_keys.push(public_key);
        }
        let mut cpu_ns = [0; TRIALS];
        let mut metal_ns = [0; TRIALS];
        for trial in 0..TRIALS {
            if started.elapsed() >= MAX_CALIBRATION {
                return Err(CalibrationFailure::Deadline);
            }
            let cpu_start = Instant::now();
            let expected: Vec<bool> = signatures
                .iter()
                .zip(&public_keys)
                .zip(&messages)
                .map(|((signature, key), message)| {
                    crate::signature::verify_signature(
                        crate::signature::SignatureScheme::Ed25519,
                        message,
                        signature,
                        key,
                    )
                })
                .collect();
            cpu_ns[trial] = elapsed_ns(cpu_start);
            let metal_start = Instant::now();
            let mut hrams = Vec::new();
            hrams
                .try_reserve_exact(items)
                .map_err(|_| CalibrationFailure::Allocation)?;
            for ((signature, key), message) in signatures.iter().zip(&public_keys).zip(&messages) {
                // Include the production staging/parsing cost, not only the
                // GPU launch and readback, in this public comparison.
                if crate::signature::signature_has_invalid_ed25519_r(signature)
                    || crate::signature::parse_ed25519_public_key_for_verification(key).is_none()
                {
                    return Err(CalibrationFailure::ParityMismatch);
                }
                hrams.push(crate::signature::ed25519_challenge_scalar_bytes(
                    signature, key, message,
                ));
            }
            let mut actual = vec![false; items];
            if !super::metal_ed25519_verify_batch_with_receipt_into(
                &signatures,
                &public_keys,
                &hrams,
                &mut actual,
                None,
            ) {
                return Err(CalibrationFailure::BackendUnavailable);
            }
            metal_ns[trial] = elapsed_ns(metal_start);
            if actual != expected {
                super::record_metal_disable("synthetic Ed25519 batch parity mismatch");
                return Err(CalibrationFailure::ParityMismatch);
            }
        }
        if !stable_trials(cpu_ns) || !stable_trials(metal_ns) {
            return Err(CalibrationFailure::Overloaded);
        }
        samples[index] = BatchCostSample {
            items,
            cpu_ns: median(cpu_ns),
            metal_ns: median(metal_ns),
        };
    }
    Ok(samples)
}

pub(super) fn calibrate_batch(
    work: MetalBatchWork,
) -> Result<MetalBatchCostProfile, CalibrationFailure> {
    let _reservation = MemoryReservation::active(BATCH_CALIBRATION_RESERVATION_BYTES);
    let started = Instant::now();
    let cpu_choice = super::simd_choice();
    let samples = match work {
        MetalBatchWork::Ed25519 => calibrate_ed25519(started)?,
        _ => calibrate_aes(work, started)?,
    };
    if started.elapsed() >= MAX_CALIBRATION || cpu_choice != super::simd_choice() {
        return Err(CalibrationFailure::Deadline);
    }
    Ok(MetalBatchCostProfile {
        work,
        samples,
        cpu_choice,
    })
}

pub(super) fn calibrate() -> Result<MetalMerkleCostProfile, CalibrationFailure> {
    let _reservation = MemoryReservation::active(CALIBRATION_RESERVATION_BYTES);
    let started = Instant::now();
    let mut samples = [CostSample::default(); SAMPLE_LEAVES.len()];
    let cpu_choice = super::simd_choice();
    for (index, leaves) in SAMPLE_LEAVES.into_iter().enumerate() {
        let data = synthetic_data(leaves).ok_or(CalibrationFailure::Allocation)?;
        let mut cpu_leaves_ns = [0; TRIALS];
        let mut metal_leaves_ns = [0; TRIALS];
        let mut cpu_root_ns = [0; TRIALS];
        let mut metal_root_ns = [0; TRIALS];
        for trial in 0..TRIALS {
            if started.elapsed() >= MAX_CALIBRATION {
                return Err(CalibrationFailure::Deadline);
            }
            let cpu_start = Instant::now();
            let cpu_leaves: Vec<[u8; 32]> =
                data.par_chunks_exact(32).map(sha256_oneblock32).collect();
            cpu_leaves_ns[trial] = elapsed_ns(cpu_start);

            // The timed Metal path includes public block preparation, host
            // allocation, upload, launch, completion, download and decoding.
            let metal_start = Instant::now();
            let blocks = padded_blocks(&data).ok_or(CalibrationFailure::Allocation)?;
            let metal_leaves = super::metal_sha256_leaves_with_receipt(&blocks, None)
                .ok_or(CalibrationFailure::BackendUnavailable)?;
            metal_leaves_ns[trial] = elapsed_ns(metal_start);
            if metal_leaves != cpu_leaves {
                super::record_metal_disable("synthetic Merkle leaf parity mismatch");
                return Err(CalibrationFailure::ParityMismatch);
            }

            let cpu_start = Instant::now();
            let cpu_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32)
                .map_err(|_| CalibrationFailure::Allocation)?;
            let cpu_root = *cpu_tree
                .root()
                .ok_or(CalibrationFailure::BackendUnavailable)?
                .as_ref();
            cpu_root_ns[trial] = elapsed_ns(cpu_start);

            let metal_start = Instant::now();
            let metal_root =
                super::metal_sha256_pairs_reduce_with_receipt(&metal_leaves, None, true)
                    .ok_or(CalibrationFailure::BackendUnavailable)?;
            metal_root_ns[trial] = metal_leaves_ns[trial].saturating_add(elapsed_ns(metal_start));
            if metal_root != cpu_root {
                super::record_metal_disable("synthetic Merkle root parity mismatch");
                return Err(CalibrationFailure::ParityMismatch);
            }
            if started.elapsed() >= MAX_CALIBRATION {
                return Err(CalibrationFailure::Deadline);
            }
        }
        if !stable_trials(cpu_leaves_ns)
            || !stable_trials(metal_leaves_ns)
            || !stable_trials(cpu_root_ns)
            || !stable_trials(metal_root_ns)
        {
            return Err(CalibrationFailure::Overloaded);
        }
        samples[index] = CostSample {
            leaves,
            cpu_leaves_ns: median(cpu_leaves_ns),
            metal_leaves_ns: median(metal_leaves_ns),
            cpu_root_ns: median(cpu_root_ns),
            metal_root_ns: median(metal_root_ns),
        };
    }
    Ok(MetalMerkleCostProfile {
        samples,
        cpu_choice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_uses_bounded_middle_trial() {
        assert_eq!(median([9, 3, 7]), 7);
        assert!(stable_trials([9, 3, 7]));
        assert!(!stable_trials([100, 3, 7]));
    }

    #[test]
    fn profile_selects_from_public_geometry_only() {
        let samples = SAMPLE_LEAVES.map(|leaves| CostSample {
            leaves,
            cpu_leaves_ns: 20,
            metal_leaves_ns: if leaves >= 16_384 { 10 } else { 30 },
            cpu_root_ns: 30,
            metal_root_ns: if leaves >= 32_768 { 10 } else { 40 },
        });
        let profile = MetalMerkleCostProfile {
            samples,
            cpu_choice: super::super::simd_choice(),
        };
        assert!(!profile.prefer_metal(MetalMerkleWork::Leaves, 8_191));
        assert!(!profile.prefer_metal(MetalMerkleWork::Leaves, 8_192));
        assert!(profile.prefer_metal(MetalMerkleWork::Leaves, 16_384));
        assert!(!profile.prefer_metal(MetalMerkleWork::Root, 16_384));
        assert!(profile.prefer_metal(MetalMerkleWork::Root, 32_768));
        assert!(profile.prefer_metal(MetalMerkleWork::Root, usize::MAX));
    }

    #[test]
    fn qualified_owner_generates_parity_checked_profile() {
        if !super::super::metal_available() {
            return;
        }
        // Parallel library tests may consume the eight-second public sampling
        // budget. That is a CPU-fallback/retry outcome, not a parity failure.
        if super::super::metal_merkle_cost_profile().is_none() {
            assert!(!super::super::metal_merkle_prefer_gpu(
                MetalMerkleWork::Leaves,
                65_536
            ));
        }
    }

    #[test]
    fn transient_calibration_is_retryable_but_parity_failure_is_quarantined() {
        let now = Instant::now();
        let mut cache = MetalMerkleCostCache::default();
        assert!(
            cache
                .get_or_calibrate(now, || Err(CalibrationFailure::Deadline))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now + RETRY_AFTER_TRANSIENT_FAILURE / 2, || {
                    panic!("cooldown must not calibrate")
                })
                .is_none()
        );
        let profile = MetalMerkleCostProfile {
            samples: SAMPLE_LEAVES.map(|leaves| CostSample {
                leaves,
                cpu_leaves_ns: 20,
                metal_leaves_ns: 10,
                cpu_root_ns: 30,
                metal_root_ns: 10,
            }),
            cpu_choice: super::super::simd_choice(),
        };
        assert!(
            cache
                .get_or_calibrate(now + RETRY_AFTER_TRANSIENT_FAILURE, || Ok(profile))
                .is_some()
        );
        assert!(
            cache
                .get_or_calibrate(now, || panic!("qualified profile must be cached"))
                .is_some()
        );
        cache.profile.as_mut().expect("cached profile").cpu_choice =
            match super::super::simd_choice() {
                SimdChoice::Scalar => SimdChoice::Neon,
                _ => SimdChoice::Scalar,
            };
        let mut recalibrated = false;
        assert!(
            cache
                .get_or_calibrate(now, || {
                    recalibrated = true;
                    Ok(profile)
                })
                .is_some(),
            "a changed CPU policy must trigger a fresh comparison"
        );
        assert!(recalibrated);

        let mut cache = MetalMerkleCostCache::default();
        assert!(
            cache
                .get_or_calibrate(now, || Err(CalibrationFailure::ParityMismatch))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now + RETRY_AFTER_TRANSIENT_FAILURE * 2, || {
                    panic!("parity failure must remain quarantined")
                })
                .is_none()
        );
    }

    #[test]
    fn batch_profile_uses_only_public_family_rounds_and_count() {
        let samples = AES_BATCH_ITEMS.map(|items| BatchCostSample {
            items,
            cpu_ns: (items * 100) as u64,
            metal_ns: if items >= 512 {
                (items * 50) as u64
            } else {
                (items * 120) as u64
            },
        });
        let profile = MetalBatchCostProfile {
            work: MetalBatchWork::AesEncRounds(10),
            samples,
            cpu_choice: super::super::simd_choice(),
        };
        assert!(!profile.prefer_metal(31));
        assert!(!profile.prefer_metal(32));
        assert!(!profile.prefer_metal(128));
        assert!(profile.prefer_metal(512));
        assert!(profile.prefer_metal(2_048));
        assert_eq!(MetalBatchWork::AesEncRounds(2).family_index(), 2);
        assert_eq!(MetalBatchWork::AesEncRounds(10).family_index(), 2);
        assert_ne!(
            MetalBatchWork::AesEncRounds(2),
            MetalBatchWork::AesEncRounds(10)
        );
        assert!(MetalBatchWork::AesEncRounds(64).calibration_supported());
        assert!(!MetalBatchWork::AesEncRounds(65).calibration_supported());
        assert_eq!(MetalBatchWork::AesEnc.min_items(), 32);
        assert_eq!(MetalBatchWork::Ed25519.min_items(), 16);
    }

    #[test]
    fn batch_profile_retries_transient_failures_without_parity_reuse() {
        let now = Instant::now();
        let work = MetalBatchWork::AesDec;
        let profile = MetalBatchCostProfile {
            work,
            samples: AES_BATCH_ITEMS.map(|items| BatchCostSample {
                items,
                cpu_ns: 100,
                metal_ns: 50,
            }),
            cpu_choice: super::super::simd_choice(),
        };
        let mut cache = MetalBatchCostCache::default();
        assert!(
            cache
                .get_or_calibrate(now, work, || Err(CalibrationFailure::Deadline))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now, work, || panic!("cooldown"))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now + RETRY_AFTER_TRANSIENT_FAILURE, work, || Ok(profile))
                .is_some()
        );
        assert!(
            cache
                .get_or_calibrate(now, work, || panic!("cached"))
                .is_some()
        );
        assert!(
            cache
                .get_or_calibrate(now, MetalBatchWork::AesEnc, || panic!(
                    "round/family must requalify"
                ))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now, work, || panic!("original profile remains cached"))
                .is_some()
        );
        let mut cache = MetalBatchCostCache::default();
        assert!(
            cache
                .get_or_calibrate(now, work, || Err(CalibrationFailure::ParityMismatch))
                .is_none()
        );
        assert!(
            cache
                .get_or_calibrate(now + RETRY_AFTER_TRANSIENT_FAILURE * 2, work, || panic!(
                    "quarantined"
                ))
                .is_none()
        );
    }

    #[test]
    fn qualified_owner_can_sample_aes_and_ed25519_without_dispatch_receipts() {
        if !super::super::metal_available() {
            return;
        }
        // A loaded host can miss the bounded deadline; CPU remains the valid
        // path and a later attempt may retry. Parity faults disable Metal.
        let _ = super::super::metal_batch_prefer_gpu(MetalBatchWork::AesEnc, 2_048);
        let _ = super::super::metal_batch_prefer_gpu(MetalBatchWork::Ed25519, 512);
        assert!(super::super::metal_parity_ok());
    }
}
