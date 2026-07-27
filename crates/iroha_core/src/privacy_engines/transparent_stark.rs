//! Protocol-neutral transparent Goldilocks STARK primitives.
//!
//! This module contains only proof-system substrate: canonical Goldilocks
//! arithmetic, FFT/coset evaluation, zero-knowledge trace masking, framed
//! Fiat–Shamir, SHA-256 Merkle commitments, binary FRI folding, grinding, and
//! exact byte readers/writers.  Protocol relations and AIR constraints do not
//! belong here.  ZK-ACE, zk-X509, private IVM, and PQ actions can therefore
//! share one audited implementation without sharing or weakening relations.
//!
//! The historical generic `crate::zk_stark` development envelope is not used:
//! its query schedule does not establish knowledge of the witness-bearing row.
//! Callers of this substrate must commit and query every masked witness column,
//! bind composition quotients to those same openings, and perform the complete
//! FRI terminal-degree check.

use std::collections::BTreeSet;

use rand::TryRngCore;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Goldilocks prime `2^64 - 2^32 + 1`.
pub(crate) const GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;
const GOLDILOCKS_MODULUS_U128_V1: u128 = GOLDILOCKS_MODULUS_V1 as u128;
/// Canonical generator used for every compiled domain and coset.
pub(crate) const GOLDILOCKS_GENERATOR_V1: u64 = 7;
/// Two-adicity of the Goldilocks multiplicative group.
pub(crate) const GOLDILOCKS_TWO_ADICITY_V1: u32 = 32;
const TRANSCRIPT_FRAME_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:frame:v1";
const TRANSCRIPT_INIT_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:init:v1";
const TRANSCRIPT_ABSORB_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:absorb:v1";
const TRANSCRIPT_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:challenge:v1";
const QUERY_INDEX_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:query-index:v1";
const GRINDING_DOMAIN_V1: &[u8] = b"iroha:privacy:transparent-stark:grinding:v1";
const MAX_FIELD_REJECTION_ATTEMPTS_V1: u64 = 16;

/// Canonical Goldilocks field element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoldilocksFieldV1(pub(crate) u64);

impl GoldilocksFieldV1 {
    /// Additive identity.
    pub(crate) const ZERO: Self = Self(0);
    /// Multiplicative identity.
    pub(crate) const ONE: Self = Self(1);

    /// Decode one canonical residue.
    pub(crate) fn canonical(value: u64) -> Option<Self> {
        (value < GOLDILOCKS_MODULUS_V1).then_some(Self(value))
    }

    /// Reduce a 128-bit value modulo the Goldilocks prime.
    pub(crate) fn reduce(value: u128) -> Self {
        Self((value % GOLDILOCKS_MODULUS_U128_V1) as u64)
    }

    /// Canonical residue as a `u64`.
    pub(crate) const fn value(self) -> u64 {
        self.0
    }

    /// Field addition.
    pub(crate) fn add(self, rhs: Self) -> Self {
        Self::reduce(u128::from(self.0) + u128::from(rhs.0))
    }

    /// Field subtraction.
    pub(crate) fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            Self(GOLDILOCKS_MODULUS_V1 - (rhs.0 - self.0))
        }
    }

    /// Field multiplication.
    pub(crate) fn mul(self, rhs: Self) -> Self {
        Self::reduce(u128::from(self.0) * u128::from(rhs.0))
    }

    /// Exponentiation by repeated squaring.
    pub(crate) fn pow(mut self, mut exponent: u128) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }

    /// Multiplicative inverse, absent for zero.
    pub(crate) fn inv(self) -> Option<Self> {
        (self != Self::ZERO).then(|| self.pow(u128::from(GOLDILOCKS_MODULUS_V1 - 2)))
    }
}

/// Failure in protocol-neutral transparent-proof machinery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum TransparentStarkErrorV1 {
    /// A power-of-two domain shape or degree bound is invalid.
    #[error("transparent STARK domain shape is invalid")]
    InvalidDomain,
    /// The requested FFT domain exceeds Goldilocks two-adicity.
    #[error("transparent STARK domain exceeds Goldilocks two-adicity")]
    DomainTooLarge,
    /// A required inverse does not exist.
    #[error("transparent STARK attempted to invert zero")]
    DivisionByZero,
    /// An encoded field value is not canonical.
    #[error("transparent STARK field encoding is non-canonical")]
    NonCanonicalField,
    /// A Merkle tree or opening has an invalid shape.
    #[error("transparent STARK Merkle shape is invalid")]
    InvalidMerkleShape,
    /// A Merkle opening does not match its root.
    #[error("transparent STARK Merkle opening is invalid")]
    InvalidMerkleOpening,
    /// Canonical transcript framing overflowed.
    #[error("transparent STARK transcript frame length overflow")]
    FrameLengthOverflow,
    /// Fiat–Shamir sampling exhausted its fixed rejection bound.
    #[error("transparent STARK Fiat-Shamir rejection bound exhausted")]
    ChallengeSamplingExhausted,
    /// Unique query-index derivation exhausted its fixed work bound.
    #[error("transparent STARK query-index derivation exhausted")]
    QuerySamplingExhausted,
    /// The operating-system or injected random source failed.
    #[error("transparent STARK masking randomness is unavailable")]
    RandomnessUnavailable,
    /// A complete terminal polynomial exceeds the required degree.
    #[error("transparent STARK FRI terminal degree is too high")]
    FriDegree,
    /// A proof byte stream is truncated or has a trailing suffix.
    #[error("transparent STARK proof bytes are malformed")]
    MalformedProof,
    /// Exact bounded allocation failed.
    #[error("transparent STARK bounded allocation failed")]
    AllocationFailure,
    /// The configured grinding nonce does not meet its bit target.
    #[error("transparent STARK grinding nonce is invalid")]
    InvalidGrinding,
}

/// Compute the primitive root for an exact power-of-two order.
pub(crate) fn goldilocks_primitive_root_v1(
    log_size: u8,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    if u32::from(log_size) > GOLDILOCKS_TWO_ADICITY_V1 {
        return Err(TransparentStarkErrorV1::DomainTooLarge);
    }
    let order = 1_u128 << log_size;
    let root = GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1)
        .pow((u128::from(GOLDILOCKS_MODULUS_V1) - 1) / order);
    if root.pow(order) != GoldilocksFieldV1::ONE
        || (order > 1 && root.pow(order / 2) == GoldilocksFieldV1::ONE)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    Ok(root)
}

/// In-place radix-two FFT.
pub(crate) fn goldilocks_fft_v1(
    values: &mut [GoldilocksFieldV1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    let size = values.len();
    if size == 0
        || !size.is_power_of_two()
        || root.pow(size as u128) != GoldilocksFieldV1::ONE
        || (size > 1 && root.pow((size / 2) as u128) == GoldilocksFieldV1::ONE)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    if values.iter().any(|value| value.0 >= GOLDILOCKS_MODULUS_V1) {
        return Err(TransparentStarkErrorV1::NonCanonicalField);
    }
    let mut reversed = 0_usize;
    for index in 1..size {
        let mut bit = size >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut width = 2_usize;
    while width <= size {
        let step = root.pow((size / width) as u128);
        for chunk in values.chunks_exact_mut(width) {
            let mut twiddle = GoldilocksFieldV1::ONE;
            let (left, right) = chunk.split_at_mut(width / 2);
            for (even, odd) in left.iter_mut().zip(right.iter_mut()) {
                let scaled_odd = (*odd).mul(twiddle);
                let original_even = *even;
                *even = original_even.add(scaled_odd);
                *odd = original_even.sub(scaled_odd);
                twiddle = twiddle.mul(step);
            }
        }
        width <<= 1;
    }
    Ok(())
}

/// In-place inverse radix-two FFT.
pub(crate) fn goldilocks_ifft_v1(
    values: &mut [GoldilocksFieldV1],
    root: GoldilocksFieldV1,
) -> Result<(), TransparentStarkErrorV1> {
    goldilocks_fft_v1(
        values,
        root.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?,
    )?;
    let inverse_size = GoldilocksFieldV1::reduce(values.len() as u128)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    for value in values {
        *value = value.mul(inverse_size);
    }
    Ok(())
}

/// Evaluate coefficients over one shifted radix-two domain.
pub(crate) fn goldilocks_evaluate_coset_v1(
    coefficients: &[GoldilocksFieldV1],
    size: usize,
    root: GoldilocksFieldV1,
    shift: GoldilocksFieldV1,
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    if coefficients.len() > size || size == 0 || !size.is_power_of_two() {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let mut evaluations = vec![GoldilocksFieldV1::ZERO; size];
    let mut shift_power = GoldilocksFieldV1::ONE;
    for (target, coefficient) in evaluations.iter_mut().zip(coefficients.iter().copied()) {
        *target = coefficient.mul(shift_power);
        shift_power = shift_power.mul(shift);
    }
    goldilocks_fft_v1(&mut evaluations, root)?;
    Ok(evaluations)
}

/// Batch-invert a non-empty collection using one field inversion.
pub(crate) fn goldilocks_batch_invert_v1(
    values: &mut [GoldilocksFieldV1],
) -> Result<(), TransparentStarkErrorV1> {
    let mut prefixes = Vec::new();
    prefixes
        .try_reserve_exact(values.len())
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    let mut product = GoldilocksFieldV1::ONE;
    for value in values.iter().copied() {
        if value == GoldilocksFieldV1::ZERO {
            return Err(TransparentStarkErrorV1::DivisionByZero);
        }
        prefixes.push(product);
        product = product.mul(value);
    }
    let mut inverse = product
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    for index in (0..values.len()).rev() {
        let value = values[index];
        values[index] = inverse.mul(prefixes[index]);
        inverse = inverse.mul(value);
    }
    Ok(())
}

/// Draw one unbiased canonical Goldilocks field element.
pub(crate) fn random_goldilocks_v1<R: TryRngCore>(
    rng: &mut R,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    for _ in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
        let mut bytes = [0_u8; 8];
        rng.try_fill_bytes(&mut bytes)
            .map_err(|_| TransparentStarkErrorV1::RandomnessUnavailable)?;
        if let Some(value) = GoldilocksFieldV1::canonical(u64::from_le_bytes(bytes)) {
            return Ok(value);
        }
    }
    Err(TransparentStarkErrorV1::RandomnessUnavailable)
}

/// Interpolate and mask one trace column before evaluating its LDE.
///
/// The mask is `r(X) * (X^n - 1)`, so every base-domain trace value is
/// unchanged while all queried coset values are randomized.  `mask_degree`
/// is inclusive.
pub(crate) fn masked_trace_lde_column_v1<R: TryRngCore>(
    base_column: &[GoldilocksFieldV1],
    base_log_size: u8,
    lde_log_size: u8,
    mask_degree: usize,
    rng: &mut R,
) -> Result<Vec<GoldilocksFieldV1>, TransparentStarkErrorV1> {
    let base_size = 1_usize
        .checked_shl(u32::from(base_log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let lde_size = 1_usize
        .checked_shl(u32::from(lde_log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if base_column.len() != base_size
        || lde_size <= base_size
        || base_size
            .checked_add(mask_degree)
            .is_none_or(|highest| highest >= lde_size)
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let base_root = goldilocks_primitive_root_v1(base_log_size)?;
    let lde_root = goldilocks_primitive_root_v1(lde_log_size)?;
    let shift = GoldilocksFieldV1(GOLDILOCKS_GENERATOR_V1);
    if shift.pow(base_size as u128) == GoldilocksFieldV1::ONE
        || shift.pow(lde_size as u128) == GoldilocksFieldV1::ONE
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let mut coefficients = base_column.to_vec();
    goldilocks_ifft_v1(&mut coefficients, base_root)?;
    coefficients.resize(lde_size, GoldilocksFieldV1::ZERO);
    for degree in 0..=mask_degree {
        let random = random_goldilocks_v1(rng)?;
        coefficients[degree] = coefficients[degree].sub(random);
        coefficients[base_size + degree] = coefficients[base_size + degree].add(random);
    }
    goldilocks_evaluate_coset_v1(&coefficients, lde_size, lde_root, shift)
}

/// Domain-separated binary SHA-256 Merkle tree.
#[derive(Clone, Debug)]
pub(crate) struct Sha256MerkleTreeV1 {
    levels: Vec<Vec<[u8; 32]>>,
    node_domain: &'static [u8],
}

impl Sha256MerkleTreeV1 {
    /// Commit a non-empty power-of-two leaf vector.
    pub(crate) fn from_leaves(
        leaves: Vec<[u8; 32]>,
        node_domain: &'static [u8],
    ) -> Result<Self, TransparentStarkErrorV1> {
        if leaves.is_empty() || !leaves.len().is_power_of_two() || node_domain.is_empty() {
            return Err(TransparentStarkErrorV1::InvalidMerkleShape);
        }
        let mut levels = vec![leaves];
        while levels.last().map_or(0, Vec::len) > 1 {
            let previous = levels
                .last()
                .ok_or(TransparentStarkErrorV1::InvalidMerkleShape)?;
            let next = previous
                .chunks_exact(2)
                .map(|pair| sha256_merkle_node_v1(node_domain, &pair[0], &pair[1]))
                .collect();
            levels.push(next);
        }
        Ok(Self {
            levels,
            node_domain,
        })
    }

    /// Root digest.
    pub(crate) fn root(&self) -> [u8; 32] {
        self.levels[self.levels.len() - 1][0]
    }

    /// Leaf-to-root sibling path.
    pub(crate) fn path(&self, mut index: usize) -> Result<Vec<[u8; 32]>, TransparentStarkErrorV1> {
        if index >= self.levels[0].len() {
            return Err(TransparentStarkErrorV1::InvalidMerkleShape);
        }
        let mut path = Vec::new();
        path.try_reserve_exact(self.levels.len() - 1)
            .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
        for level in &self.levels[..self.levels.len() - 1] {
            path.push(level[index ^ 1]);
            index >>= 1;
        }
        Ok(path)
    }

    /// Domain used for internal nodes.
    pub(crate) const fn node_domain(&self) -> &'static [u8] {
        self.node_domain
    }
}

/// Hash one binary Merkle node with an engine-fixed role domain.
pub(crate) fn sha256_merkle_node_v1(
    node_domain: &[u8],
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    sha256_frame_v1(node_domain, &[left, right])
        .expect("two fixed hashes and a static domain are representable")
}

/// Verify one exact binary Merkle path.
pub(crate) fn verify_sha256_merkle_path_v1(
    node_domain: &[u8],
    root: &[u8; 32],
    mut leaf: [u8; 32],
    mut index: usize,
    path: &[[u8; 32]],
    expected_depth: usize,
) -> Result<(), TransparentStarkErrorV1> {
    if node_domain.is_empty() || path.len() != expected_depth {
        return Err(TransparentStarkErrorV1::InvalidMerkleShape);
    }
    for sibling in path {
        leaf = if index & 1 == 0 {
            sha256_merkle_node_v1(node_domain, &leaf, sibling)
        } else {
            sha256_merkle_node_v1(node_domain, sibling, &leaf)
        };
        index >>= 1;
    }
    if index != 0 || leaf != *root {
        return Err(TransparentStarkErrorV1::InvalidMerkleOpening);
    }
    Ok(())
}

/// Hash an unambiguous domain-and-field frame.
pub(crate) fn sha256_frame_v1(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<[u8; 32], TransparentStarkErrorV1> {
    let domain_len =
        u16::try_from(domain.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
    let field_count =
        u16::try_from(fields.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
    let mut hash = Sha256::new();
    hash.update(TRANSCRIPT_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(domain);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let length =
            u64::try_from(field.len()).map_err(|_| TransparentStarkErrorV1::FrameLengthOverflow)?;
        hash.update(length.to_be_bytes());
        hash.update(field);
    }
    Ok(hash.finalize().into())
}

/// Stateful framed Fiat–Shamir transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransparentTranscriptV1 {
    state: [u8; 32],
    challenge_counter: u64,
}

impl TransparentTranscriptV1 {
    /// Initialize with the engine suite, complete profile digest, and exact
    /// public-input digest.
    pub(crate) fn new(
        engine_suite: &[u8],
        profile_digest: &[u8; 32],
        public_input_digest: &[u8; 32],
    ) -> Result<Self, TransparentStarkErrorV1> {
        if engine_suite.is_empty() {
            return Err(TransparentStarkErrorV1::FrameLengthOverflow);
        }
        Ok(Self {
            state: sha256_frame_v1(
                TRANSCRIPT_INIT_DOMAIN_V1,
                &[engine_suite, profile_digest, public_input_digest],
            )?,
            challenge_counter: 0,
        })
    }

    /// Absorb one labeled message and reset the local challenge counter.
    pub(crate) fn absorb(
        &mut self,
        label: &[u8],
        fields: &[&[u8]],
    ) -> Result<(), TransparentStarkErrorV1> {
        let message = sha256_frame_v1(label, fields)?;
        self.state = sha256_frame_v1(TRANSCRIPT_ABSORB_DOMAIN_V1, &[&self.state, label, &message])?;
        self.challenge_counter = 0;
        Ok(())
    }

    /// Current transcript state for query/grinding derivation.
    pub(crate) const fn state(&self) -> [u8; 32] {
        self.state
    }

    /// Derive one unbiased nonzero Goldilocks challenge.
    pub(crate) fn challenge_field(
        &mut self,
        label: &[u8],
    ) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
        for attempt in 0..MAX_FIELD_REJECTION_ATTEMPTS_V1 {
            let digest = sha256_frame_v1(
                TRANSCRIPT_CHALLENGE_DOMAIN_V1,
                &[
                    &self.state,
                    label,
                    &self.challenge_counter.to_be_bytes(),
                    &attempt.to_be_bytes(),
                ],
            )?;
            let candidate = u64::from_be_bytes(
                digest[..8]
                    .try_into()
                    .expect("SHA-256 prefix is exactly eight bytes"),
            );
            if let Some(field) = GoldilocksFieldV1::canonical(candidate)
                && field != GoldilocksFieldV1::ZERO
            {
                self.challenge_counter = self
                    .challenge_counter
                    .checked_add(1)
                    .ok_or(TransparentStarkErrorV1::ChallengeSamplingExhausted)?;
                self.state =
                    sha256_frame_v1(TRANSCRIPT_ABSORB_DOMAIN_V1, &[&self.state, label, &digest])?;
                return Ok(field);
            }
        }
        Err(TransparentStarkErrorV1::ChallengeSamplingExhausted)
    }
}

/// Derive unique unbiased query indices for a power-of-two domain.
pub(crate) fn derive_unique_query_indices_v1(
    seed: &[u8; 32],
    domain_size: usize,
    query_count: usize,
) -> Result<Vec<usize>, TransparentStarkErrorV1> {
    if domain_size == 0
        || !domain_size.is_power_of_two()
        || query_count == 0
        || query_count > domain_size
    {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let max_attempts = domain_size
        .checked_mul(2)
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(query_count)
        .map_err(|_| TransparentStarkErrorV1::AllocationFailure)?;
    let mut seen = BTreeSet::new();
    for counter in 0..max_attempts {
        let counter =
            u64::try_from(counter).map_err(|_| TransparentStarkErrorV1::QuerySamplingExhausted)?;
        let digest = sha256_frame_v1(QUERY_INDEX_DOMAIN_V1, &[seed, &counter.to_be_bytes()])?;
        let raw = u64::from_be_bytes(
            digest[..8]
                .try_into()
                .expect("SHA-256 prefix is exactly eight bytes"),
        );
        let index = (raw as usize) & (domain_size - 1);
        if seen.insert(index) {
            indices.push(index);
            if indices.len() == query_count {
                return Ok(indices);
            }
        }
    }
    Err(TransparentStarkErrorV1::QuerySamplingExhausted)
}

/// Compute one binary FRI fold.
pub(crate) fn fri_fold_pair_v1(
    low: GoldilocksFieldV1,
    high: GoldilocksFieldV1,
    beta: GoldilocksFieldV1,
    x: GoldilocksFieldV1,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    let inverse_x = x.inv().ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    fri_fold_pair_with_inverse_x_v1(low, high, beta, inverse_x)
}

/// Compute one binary FRI fold when the caller already tracks `x^-1`.
///
/// Provers fold an entire multiplicative coset in order and can update the
/// inverse point with one multiplication per entry.  Keeping that optimization
/// here avoids duplicating the consensus-critical fold equation in each
/// relation-specific engine.
pub(crate) fn fri_fold_pair_with_inverse_x_v1(
    low: GoldilocksFieldV1,
    high: GoldilocksFieldV1,
    beta: GoldilocksFieldV1,
    inverse_x: GoldilocksFieldV1,
) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
    let inverse_two = GoldilocksFieldV1(2)
        .inv()
        .ok_or(TransparentStarkErrorV1::DivisionByZero)?;
    let even = low.add(high).mul(inverse_two);
    let odd = low.sub(high).mul(inverse_two).mul(inverse_x);
    Ok(even.add(beta.mul(odd)))
}

/// Check the entire terminal FRI polynomial against an exact degree bound.
pub(crate) fn ensure_fri_terminal_degree_v1(
    values: &[GoldilocksFieldV1],
    log_size: u8,
    degree_bound: usize,
) -> Result<(), TransparentStarkErrorV1> {
    let expected = 1_usize
        .checked_shl(u32::from(log_size))
        .ok_or(TransparentStarkErrorV1::InvalidDomain)?;
    if values.len() != expected || degree_bound >= expected {
        return Err(TransparentStarkErrorV1::InvalidDomain);
    }
    let root = goldilocks_primitive_root_v1(log_size)?;
    let mut coefficients = values.to_vec();
    goldilocks_ifft_v1(&mut coefficients, root)?;
    if coefficients[degree_bound + 1..]
        .iter()
        .any(|coefficient| *coefficient != GoldilocksFieldV1::ZERO)
    {
        return Err(TransparentStarkErrorV1::FriDegree);
    }
    Ok(())
}

/// Search for the smallest nonce meeting an exact leading-zero-bit target.
pub(crate) fn grind_nonce_v1(
    transcript_seed: &[u8; 32],
    grinding_bits: u8,
) -> Result<u64, TransparentStarkErrorV1> {
    if grinding_bits > 63 {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    for nonce in 0..=u64::MAX {
        if verify_grinding_nonce_v1(transcript_seed, grinding_bits, nonce).is_ok() {
            return Ok(nonce);
        }
    }
    Err(TransparentStarkErrorV1::InvalidGrinding)
}

/// Verify a transcript grinding nonce.
pub(crate) fn verify_grinding_nonce_v1(
    transcript_seed: &[u8; 32],
    grinding_bits: u8,
    nonce: u64,
) -> Result<(), TransparentStarkErrorV1> {
    if grinding_bits > 63 {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    let digest = sha256_frame_v1(GRINDING_DOMAIN_V1, &[transcript_seed, &nonce.to_be_bytes()])?;
    if leading_zero_bits_v1(&digest) < u32::from(grinding_bits) {
        return Err(TransparentStarkErrorV1::InvalidGrinding);
    }
    Ok(())
}

fn leading_zero_bits_v1(bytes: &[u8]) -> u32 {
    let mut count = 0_u32;
    for byte in bytes {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// Strict fixed-shape proof reader.
pub(crate) struct ExactProofReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ExactProofReaderV1<'a> {
    /// Construct over one size-capped proof slice.
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Read an exact byte array.
    pub(crate) fn take<const N: usize>(&mut self) -> Result<[u8; N], TransparentStarkErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(TransparentStarkErrorV1::MalformedProof)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(TransparentStarkErrorV1::MalformedProof)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| TransparentStarkErrorV1::MalformedProof)
    }

    /// Read big-endian `u16`.
    pub(crate) fn u16(&mut self) -> Result<u16, TransparentStarkErrorV1> {
        self.take().map(u16::from_be_bytes)
    }

    /// Read big-endian `u32`.
    pub(crate) fn u32(&mut self) -> Result<u32, TransparentStarkErrorV1> {
        self.take().map(u32::from_be_bytes)
    }

    /// Read big-endian `u64`.
    pub(crate) fn u64(&mut self) -> Result<u64, TransparentStarkErrorV1> {
        self.take().map(u64::from_be_bytes)
    }

    /// Read one canonical Goldilocks value.
    pub(crate) fn field(&mut self) -> Result<GoldilocksFieldV1, TransparentStarkErrorV1> {
        GoldilocksFieldV1::canonical(self.u64()?).ok_or(TransparentStarkErrorV1::NonCanonicalField)
    }

    /// Require exact end-of-input.
    pub(crate) fn finish(self) -> Result<(), TransparentStarkErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(TransparentStarkErrorV1::MalformedProof)
        }
    }
}

/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u16_v1(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u32_v1(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

/// Append big-endian fixed integers to a canonical proof.
pub(crate) fn append_u64_v1(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng as _, rngs::StdRng};

    use super::*;

    #[test]
    fn fft_roundtrips_every_small_power_of_two_domain() {
        for log_size in 0..=12 {
            let root = goldilocks_primitive_root_v1(log_size).expect("primitive root");
            let size = 1_usize << log_size;
            let mut values = (0..size)
                .map(|index| GoldilocksFieldV1::reduce((index as u128 + 7).pow(3)))
                .collect::<Vec<_>>();
            let original = values.clone();
            goldilocks_fft_v1(&mut values, root).expect("FFT");
            goldilocks_ifft_v1(&mut values, root).expect("IFFT");
            assert_eq!(values, original);
        }

        let mut wrong_order = vec![GoldilocksFieldV1::ONE; 8];
        assert_eq!(
            goldilocks_fft_v1(&mut wrong_order, GoldilocksFieldV1::ONE),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
        let root = goldilocks_primitive_root_v1(3).expect("primitive root");
        let mut noncanonical = vec![GoldilocksFieldV1::ZERO; 8];
        noncanonical[3] = GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            goldilocks_fft_v1(&mut noncanonical, root),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }

    #[test]
    fn masked_trace_lde_is_randomized_and_preserves_degree_capacity() {
        let base = (0..16)
            .map(|index| GoldilocksFieldV1(index))
            .collect::<Vec<_>>();
        let mut first_rng = StdRng::from_seed([0x11; 32]);
        let mut second_rng = StdRng::from_seed([0x22; 32]);
        let first = masked_trace_lde_column_v1(&base, 4, 7, 7, &mut first_rng).expect("first mask");
        let second =
            masked_trace_lde_column_v1(&base, 4, 7, 7, &mut second_rng).expect("second mask");
        assert_eq!(first.len(), 128);
        assert_ne!(first, second);
        assert_eq!(
            masked_trace_lde_column_v1(&base, 4, 4, 0, &mut first_rng),
            Err(TransparentStarkErrorV1::InvalidDomain)
        );
    }

    #[test]
    fn merkle_paths_bind_domain_index_leaf_order_and_depth() {
        let domain = b"iroha:test:transparent-stark:node:v1";
        let leaves = (0..8)
            .map(|index| sha256_frame_v1(b"leaf", &[&[index]]).expect("leaf hash"))
            .collect::<Vec<_>>();
        let tree = Sha256MerkleTreeV1::from_leaves(leaves.clone(), domain).expect("tree");
        for (index, leaf) in leaves.iter().copied().enumerate() {
            verify_sha256_merkle_path_v1(
                domain,
                &tree.root(),
                leaf,
                index,
                &tree.path(index).expect("path"),
                3,
            )
            .expect("opening");
        }
        let mut path = tree.path(3).expect("path");
        path[0][0] ^= 1;
        assert_eq!(
            verify_sha256_merkle_path_v1(domain, &tree.root(), leaves[3], 3, &path, 3),
            Err(TransparentStarkErrorV1::InvalidMerkleOpening)
        );
        assert_eq!(
            verify_sha256_merkle_path_v1(b"other", &tree.root(), leaves[3], 3, &path, 3),
            Err(TransparentStarkErrorV1::InvalidMerkleOpening)
        );
    }

    #[test]
    fn transcript_is_framed_ordered_and_deterministic() {
        let mut first =
            TransparentTranscriptV1::new(b"suite", &[1; 32], &[2; 32]).expect("transcript");
        let mut second = first;
        first.absorb(b"root", &[b"ab", b"c"]).expect("absorb");
        second.absorb(b"root", &[b"a", b"bc"]).expect("absorb");
        assert_ne!(first.state(), second.state());
        let challenge = first.challenge_field(b"alpha").expect("challenge");
        let mut replay =
            TransparentTranscriptV1::new(b"suite", &[1; 32], &[2; 32]).expect("transcript");
        replay.absorb(b"root", &[b"ab", b"c"]).expect("absorb");
        assert_eq!(
            replay.challenge_field(b"alpha").expect("challenge"),
            challenge
        );
    }

    #[test]
    fn query_indices_are_unique_deterministic_and_in_domain() {
        let first = derive_unique_query_indices_v1(&[9; 32], 1 << 12, 56).expect("queries");
        let second = derive_unique_query_indices_v1(&[9; 32], 1 << 12, 56).expect("queries");
        assert_eq!(first, second);
        assert!(first.iter().all(|index| *index < 1 << 12));
        assert_eq!(
            first.iter().copied().collect::<BTreeSet<_>>().len(),
            first.len()
        );
    }

    #[test]
    fn fri_terminal_check_rejects_high_degree_values() {
        let root = goldilocks_primitive_root_v1(4).expect("root");
        let linear = (0..16)
            .scan(GoldilocksFieldV1::ONE, |point, _| {
                let value = GoldilocksFieldV1(7).add(GoldilocksFieldV1(3).mul(*point));
                *point = point.mul(root);
                Some(value)
            })
            .collect::<Vec<_>>();
        ensure_fri_terminal_degree_v1(&linear, 4, 1).expect("linear");
        let mut high = linear;
        high[3] = high[3].add(GoldilocksFieldV1::ONE);
        assert_eq!(
            ensure_fri_terminal_degree_v1(&high, 4, 1),
            Err(TransparentStarkErrorV1::FriDegree)
        );

        let low = GoldilocksFieldV1(11);
        let high = GoldilocksFieldV1(19);
        let beta = GoldilocksFieldV1(23);
        let point = GoldilocksFieldV1(29);
        assert_eq!(
            fri_fold_pair_v1(low, high, beta, point).expect("fold"),
            fri_fold_pair_with_inverse_x_v1(low, high, beta, point.inv().expect("nonzero point"))
                .expect("optimized fold")
        );
        assert_eq!(
            fri_fold_pair_v1(low, high, beta, GoldilocksFieldV1::ZERO),
            Err(TransparentStarkErrorV1::DivisionByZero)
        );
    }

    #[test]
    fn grinding_and_exact_reader_fail_closed() {
        let nonce = grind_nonce_v1(&[0x42; 32], 8).expect("grind");
        verify_grinding_nonce_v1(&[0x42; 32], 8, nonce).expect("valid nonce");
        if nonce > 0 {
            assert_eq!(
                verify_grinding_nonce_v1(&[0x42; 32], 8, nonce - 1),
                Err(TransparentStarkErrorV1::InvalidGrinding)
            );
        }

        let mut bytes = Vec::new();
        append_u16_v1(&mut bytes, 7);
        append_u32_v1(&mut bytes, 11);
        append_u64_v1(&mut bytes, 13);
        append_u64_v1(&mut bytes, GOLDILOCKS_MODULUS_V1 - 1);
        let mut reader = ExactProofReaderV1::new(&bytes);
        assert_eq!(reader.u16().expect("u16"), 7);
        assert_eq!(reader.u32().expect("u32"), 11);
        assert_eq!(reader.u64().expect("u64"), 13);
        assert_eq!(
            reader.field().expect("field"),
            GoldilocksFieldV1(GOLDILOCKS_MODULUS_V1 - 1)
        );
        reader.finish().expect("exact end");

        let mut noncanonical = Vec::new();
        append_u64_v1(&mut noncanonical, GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            ExactProofReaderV1::new(&noncanonical).field(),
            Err(TransparentStarkErrorV1::NonCanonicalField)
        );
    }
}
