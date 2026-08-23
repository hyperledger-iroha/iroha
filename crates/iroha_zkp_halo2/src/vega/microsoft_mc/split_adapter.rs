//! Dependency-free Figure 9 split provenance and assignment validator.
//!
//! This is a first-party port of the application-circuit schedule previously
//! implemented in `vega/canonical_mc.rs` at repository commit `60b24f71eb`.
//! The rest-witness replay below is a dependency-free value-order port of the
//! Bellpepper 0.4.1 SHA gadget selected by the historical `bellpepper = "0.4.0"`
//! dependency. It stops before any commitment or proof operation.

use std::collections::BTreeSet;

use super::super::{
    VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1,
    VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1, VegaT256ScalarV1 as Scalar,
    engine::VegaMdlProofDimensionsV1,
    figure9::{
        Figure9McMaterial, Figure9McTopology, VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1,
        VEGA_MDL_FIGURE9_SHA256_STEPS_V1,
    },
    r1cs::{Shape, SparseMatrix},
};
use super::{
    Figure9SplitAdapterError, canonical_figure9_dimensions, sha256,
    verifier_key::{SparseMatrixWire, SplitShapeWire},
};

const SHA256_BLOCK_BYTES: usize = 64;
const SHA256_BLOCK_BITS: usize = SHA256_BLOCK_BYTES * 8;
const SHA256_STATE_WORDS: usize = 8;
const SHA256_WORD_BITS: usize = 32;
const MICROSOFT_COMMITMENT_WIDTH: usize = 2_048;
const CONTEXT_PUBLIC_SCALARS: usize = 4;
const CORE_PUBLIC_SCALARS: usize = VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1 + CONTEXT_PUBLIC_SCALARS;
const PINNED_BIRTH_SHA_ROWS: core::ops::Range<usize> = 3_958..120_706;
const PINNED_ISSUER_SHA_ROWS: core::ops::Range<usize> = 125_048..473_936;
const PINNED_CORE_REPLAYED_ROWS: usize = 183_382;
const PINNED_CORE_UNPADDED_CONSTRAINTS: usize = 183_400;
const PINNED_STEP_REST_CAP: usize = 522_240;
const PINNED_SELECTION_REST_VALUES: usize = 8_296;
const PINNED_SHA_REST_VALUES: usize = 26_166;
const PINNED_STEP_REST_UNPADDED: usize = PINNED_SELECTION_REST_VALUES + PINNED_SHA_REST_VALUES;

// Read-only source provenance for the exact value-order port. The historical
// manifest requested 0.4.0; Cargo's compatible resolution selected 0.4.1 for
// `bellpepper` and 0.4.0 for `bellpepper-core`.
#[cfg(test)]
const BELLPEPPER_SHA256_SOURCE_SHA256: &str =
    "e02e54f9a3a4a81c2d241cb75a42199c388d98ee1b9eea796e8bc08c9e099df3";
#[cfg(test)]
const BELLPEPPER_UINT32_SOURCE_SHA256: &str =
    "98dcc6388d44291f0fecb9adfef156e69ff1d386aa002001dcaf2bee0f876768";
#[cfg(test)]
const BELLPEPPER_MULTIEQ_SOURCE_SHA256: &str =
    "f016e3e5d33c15e459f8c9e51a71fc5495f69ad33b83d32f04c4418b70c7260e";
#[cfg(test)]
const BELLPEPPER_CORE_BOOLEAN_SOURCE_SHA256: &str =
    "8471d4b24b03662d96137c24ddf0adebc097e573e73f64b75796eb81230468e0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BitSource {
    Shared(usize),
    Constant(bool),
}

impl BitSource {
    fn resolve(self, shared_witness: &[Scalar]) -> Result<bool, Figure9SplitAdapterError> {
        match self {
            Self::Constant(value) => Ok(value),
            Self::Shared(index) => match shared_witness.get(index).copied() {
                Some(value) if value == Scalar::zero() => Ok(false),
                Some(value) if value == Scalar::one() => Ok(true),
                _ => Err(Figure9SplitAdapterError::InvalidMetadata),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayBoolean {
    Constant(bool),
    Is(bool),
    Not(bool),
}

impl ReplayBoolean {
    const fn value(self) -> bool {
        match self {
            Self::Constant(value) | Self::Is(value) => value,
            Self::Not(value) => !value,
        }
    }

    const fn raw_value(self) -> Option<bool> {
        match self {
            Self::Constant(_) => None,
            Self::Is(value) | Self::Not(value) => Some(value),
        }
    }

    const fn is_constant(self) -> bool {
        matches!(self, Self::Constant(_))
    }

    const fn not(self) -> Self {
        match self {
            Self::Constant(value) => Self::Constant(!value),
            Self::Is(value) => Self::Not(value),
            Self::Not(value) => Self::Is(value),
        }
    }
}

/// Move-only owner for Figure 9 witness scalars which clears every retained
/// element on normal return, error, or unwind.
pub(super) struct Figure9SecretScalars(Vec<Scalar>);

impl Figure9SecretScalars {
    pub(super) fn from_vec(values: Vec<Scalar>) -> Self {
        Self(values)
    }

    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub(super) fn push(&mut self, mut value: Scalar) {
        if self.0.len() >= self.0.capacity() {
            value.clear_secret();
            panic!("Figure 9 secret scalar owner exceeded its exact capacity");
        }
        self.0.push(value);
        // `Scalar` is `Copy`; clear this callee-owned parameter slot after the
        // retained vector element has been written.
        value.clear_secret();
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn as_slice(&self) -> &[Scalar] {
        &self.0
    }

    pub(super) fn clear_secret(&mut self) {
        clear_secret_scalars(&mut self.0);
    }
}

impl core::ops::Deref for Figure9SecretScalars {
    type Target = [Scalar];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl Drop for Figure9SecretScalars {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_values = !self.0.is_empty();
        self.clear_secret();
        #[cfg(test)]
        if had_values && self.0.iter().all(|value| value.is_zero()) {
            let _ = FIGURE9_SECRET_SCALAR_OWNER_DROPS.try_with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}

fn clear_secret_scalars(values: &mut [Scalar]) {
    let values = core::hint::black_box(values);
    for value in values.iter_mut() {
        value.clear_secret();
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}

#[cfg(test)]
std::thread_local! {
    static FIGURE9_SECRET_SCALAR_OWNER_DROPS: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
pub(super) fn figure9_secret_scalar_owner_drop_count() -> usize {
    FIGURE9_SECRET_SCALAR_OWNER_DROPS
        .try_with(core::cell::Cell::get)
        .unwrap_or(0)
}

struct RestWitnessRecorder {
    values: Vec<Scalar>,
    maximum: usize,
}

impl RestWitnessRecorder {
    fn new(maximum: usize) -> Result<Self, Figure9SplitAdapterError> {
        if maximum > PINNED_STEP_REST_CAP {
            return Err(Figure9SplitAdapterError::InvalidShape);
        }
        Ok(Self {
            // Exact capacity prevents a secret-bearing allocation from being
            // abandoned by a mid-replay Vec growth.
            values: Vec::with_capacity(maximum),
            maximum,
        })
    }

    fn allocate_scalar(&mut self, mut value: Scalar) -> Result<(), Figure9SplitAdapterError> {
        if self.values.len() >= self.maximum {
            value.clear_secret();
            return Err(Figure9SplitAdapterError::InvalidShape);
        }
        self.values.push(value);
        value.clear_secret();
        Ok(())
    }

    fn allocate_bit(&mut self, value: bool) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
        self.allocate_scalar(if value { Scalar::one() } else { Scalar::zero() })?;
        Ok(ReplayBoolean::Is(value))
    }

    fn finish(mut self) -> Figure9SecretScalars {
        Figure9SecretScalars::from_vec(core::mem::take(&mut self.values))
    }
}

impl Drop for RestWitnessRecorder {
    fn drop(&mut self) {
        clear_secret_scalars(&mut self.values);
    }
}

fn replay_xor(
    recorder: &mut RestWitnessRecorder,
    a: ReplayBoolean,
    b: ReplayBoolean,
) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
    match (a, b) {
        (ReplayBoolean::Constant(false), value) | (value, ReplayBoolean::Constant(false)) => {
            Ok(value)
        }
        (ReplayBoolean::Constant(true), value) | (value, ReplayBoolean::Constant(true)) => {
            Ok(value.not())
        }
        (ReplayBoolean::Is(a), ReplayBoolean::Not(b))
        | (ReplayBoolean::Not(b), ReplayBoolean::Is(a)) => Ok(recorder.allocate_bit(a ^ b)?.not()),
        (ReplayBoolean::Is(a), ReplayBoolean::Is(b))
        | (ReplayBoolean::Not(a), ReplayBoolean::Not(b)) => recorder.allocate_bit(a ^ b),
    }
}

fn replay_and(
    recorder: &mut RestWitnessRecorder,
    a: ReplayBoolean,
    b: ReplayBoolean,
) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
    match (a, b) {
        (ReplayBoolean::Constant(false), _) | (_, ReplayBoolean::Constant(false)) => {
            Ok(ReplayBoolean::Constant(false))
        }
        (ReplayBoolean::Constant(true), value) | (value, ReplayBoolean::Constant(true)) => {
            Ok(value)
        }
        (ReplayBoolean::Is(a), ReplayBoolean::Not(b))
        | (ReplayBoolean::Not(b), ReplayBoolean::Is(a)) => recorder.allocate_bit(a & !b),
        (ReplayBoolean::Not(a), ReplayBoolean::Not(b)) => recorder.allocate_bit(!a & !b),
        (ReplayBoolean::Is(a), ReplayBoolean::Is(b)) => recorder.allocate_bit(a & b),
    }
}

fn replay_sha256_ch(
    recorder: &mut RestWitnessRecorder,
    a: ReplayBoolean,
    b: ReplayBoolean,
    c: ReplayBoolean,
) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
    let value = (a.value() & b.value()) ^ (!a.value() & c.value());
    if a.is_constant() && b.is_constant() && c.is_constant() {
        return Ok(ReplayBoolean::Constant(value));
    }
    if a == ReplayBoolean::Constant(false) {
        return Ok(c);
    }
    if b == ReplayBoolean::Constant(false) {
        return replay_and(recorder, a.not(), c);
    }
    if c == ReplayBoolean::Constant(false) {
        return replay_and(recorder, a, b);
    }
    if c == ReplayBoolean::Constant(true) {
        return Ok(replay_and(recorder, a, b.not())?.not());
    }
    if b == ReplayBoolean::Constant(true) {
        return Ok(replay_and(recorder, a.not(), c.not())?.not());
    }
    // Bellpepper deliberately lets `a == Constant(true)` fall through to
    // this single-allocation formula unless an earlier simplification fired.
    recorder.allocate_bit(value)
}

fn replay_sha256_maj(
    recorder: &mut RestWitnessRecorder,
    a: ReplayBoolean,
    b: ReplayBoolean,
    c: ReplayBoolean,
) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
    let value = (a.value() & b.value()) ^ (a.value() & c.value()) ^ (b.value() & c.value());
    if a.is_constant() && b.is_constant() && c.is_constant() {
        return Ok(ReplayBoolean::Constant(value));
    }
    if a == ReplayBoolean::Constant(false) {
        return replay_and(recorder, b, c);
    }
    if b == ReplayBoolean::Constant(false) {
        return replay_and(recorder, a, c);
    }
    if c == ReplayBoolean::Constant(false) {
        return replay_and(recorder, a, b);
    }
    if c == ReplayBoolean::Constant(true) {
        return Ok(replay_and(recorder, a.not(), b.not())?.not());
    }
    if b == ReplayBoolean::Constant(true) {
        return Ok(replay_and(recorder, a.not(), c.not())?.not());
    }
    if a == ReplayBoolean::Constant(true) {
        return Ok(replay_and(recorder, b.not(), c.not())?.not());
    }

    // Exact Bellpepper order: allocate `maj` first, then allocate `b & c`
    // for its quadratic constraint. The latter value is not returned but is
    // still part of the witness vector.
    let result = recorder.allocate_bit(value)?;
    let _bc = replay_and(recorder, b, c)?;
    Ok(result)
}

#[derive(Clone, Copy)]
struct ReplayWord {
    bits_le: [ReplayBoolean; SHA256_WORD_BITS],
    value: u32,
}

impl ReplayWord {
    fn constant(value: u32) -> Self {
        Self {
            bits_le: core::array::from_fn(|bit| ReplayBoolean::Constant(value & (1 << bit) != 0)),
            value,
        }
    }

    fn from_bits_be(bits: &[ReplayBoolean]) -> Result<Self, Figure9SplitAdapterError> {
        let bits_be: [ReplayBoolean; SHA256_WORD_BITS] = bits
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
        let bits_le = core::array::from_fn(|bit| bits_be[SHA256_WORD_BITS - 1 - bit]);
        Ok(Self::from_bits_le(bits_le))
    }

    fn from_bits_le(bits_le: [ReplayBoolean; SHA256_WORD_BITS]) -> Self {
        let value = bits_le
            .iter()
            .enumerate()
            .fold(0_u32, |value, (bit, source)| {
                value | if source.value() { 1 << bit } else { 0 }
            });
        Self { bits_le, value }
    }

    fn rotr(self, by: usize) -> Self {
        let by = by % SHA256_WORD_BITS;
        Self {
            bits_le: core::array::from_fn(|bit| self.bits_le[(bit + by) % SHA256_WORD_BITS]),
            value: self.value.rotate_right(by as u32),
        }
    }

    fn shr(self, by: usize) -> Self {
        let by = by % SHA256_WORD_BITS;
        Self {
            bits_le: core::array::from_fn(|bit| {
                self.bits_le
                    .get(bit + by)
                    .copied()
                    .unwrap_or(ReplayBoolean::Constant(false))
            }),
            value: self.value >> by,
        }
    }

    fn xor(
        self,
        recorder: &mut RestWitnessRecorder,
        other: Self,
    ) -> Result<Self, Figure9SplitAdapterError> {
        let bits_le = self
            .bits_le
            .into_iter()
            .zip(other.bits_le)
            .map(|(a, b)| replay_xor(recorder, a, b))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
        Ok(Self {
            bits_le,
            value: self.value ^ other.value,
        })
    }

    fn sha256_ch(
        recorder: &mut RestWitnessRecorder,
        a: Self,
        b: Self,
        c: Self,
    ) -> Result<Self, Figure9SplitAdapterError> {
        let bits_le = a
            .bits_le
            .into_iter()
            .zip(b.bits_le)
            .zip(c.bits_le)
            .map(|((a, b), c)| replay_sha256_ch(recorder, a, b, c))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
        Ok(Self {
            bits_le,
            value: (a.value & b.value) ^ (!a.value & c.value),
        })
    }

    fn sha256_maj(
        recorder: &mut RestWitnessRecorder,
        a: Self,
        b: Self,
        c: Self,
    ) -> Result<Self, Figure9SplitAdapterError> {
        let bits_le = a
            .bits_le
            .into_iter()
            .zip(b.bits_le)
            .zip(c.bits_le)
            .map(|((a, b), c)| replay_sha256_maj(recorder, a, b, c))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
        Ok(Self {
            bits_le,
            value: (a.value & b.value) ^ (a.value & c.value) ^ (b.value & c.value),
        })
    }

    fn addmany(
        recorder: &mut RestWitnessRecorder,
        operands: &[Self],
    ) -> Result<Self, Figure9SplitAdapterError> {
        if !(2..=10).contains(&operands.len()) {
            return Err(Figure9SplitAdapterError::InvalidShape);
        }
        let sum = operands
            .iter()
            .try_fold(0_u64, |sum, operand| {
                sum.checked_add(u64::from(operand.value))
            })
            .ok_or(Figure9SplitAdapterError::InvalidShape)?;
        if operands
            .iter()
            .flat_map(|operand| operand.bits_le)
            .all(ReplayBoolean::is_constant)
        {
            return Ok(Self::constant(sum as u32));
        }

        let mut maximum = u64::try_from(operands.len())
            .ok()
            .and_then(|count| count.checked_mul(u64::from(u32::MAX)))
            .ok_or(Figure9SplitAdapterError::InvalidShape)?;
        let mut bits = Vec::with_capacity(35);
        let mut bit = 0_usize;
        while maximum != 0 {
            bits.push(recorder.allocate_bit((sum >> bit) & 1 == 1)?);
            maximum >>= 1;
            bit += 1;
        }
        bits.truncate(SHA256_WORD_BITS);
        let bits_le = bits
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
        Ok(Self {
            bits_le,
            value: sum as u32,
        })
    }
}

enum DeferredWord {
    Deferred(Vec<ReplayWord>),
    Concrete(ReplayWord),
}

impl DeferredWord {
    fn compute(
        self,
        recorder: &mut RestWitnessRecorder,
        others: &[ReplayWord],
    ) -> Result<ReplayWord, Figure9SplitAdapterError> {
        match self {
            Self::Concrete(value) => Ok(value),
            Self::Deferred(mut values) => {
                values.extend_from_slice(others);
                ReplayWord::addmany(recorder, &values)
            }
        }
    }
}

#[allow(clippy::unreadable_literal)]
const SHA256_ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn replay_sha256_compression(
    recorder: &mut RestWitnessRecorder,
    input: &[ReplayBoolean; SHA256_BLOCK_BITS],
    current_hash: &[ReplayWord; SHA256_STATE_WORDS],
) -> Result<[ReplayWord; SHA256_STATE_WORDS], Figure9SplitAdapterError> {
    let mut w = input
        .chunks_exact(SHA256_WORD_BITS)
        .map(ReplayWord::from_bits_be)
        .collect::<Result<Vec<_>, _>>()?;
    if w.len() != 16 {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    for index in 16..64 {
        let mut s0 = w[index - 15].rotr(7);
        s0 = s0.xor(recorder, w[index - 15].rotr(18))?;
        s0 = s0.xor(recorder, w[index - 15].shr(3))?;
        let mut s1 = w[index - 2].rotr(17);
        s1 = s1.xor(recorder, w[index - 2].rotr(19))?;
        s1 = s1.xor(recorder, w[index - 2].shr(10))?;
        w.push(ReplayWord::addmany(
            recorder,
            &[w[index - 16], s0, w[index - 7], s1],
        )?);
    }

    let mut a = DeferredWord::Concrete(current_hash[0]);
    let mut b = current_hash[1];
    let mut c = current_hash[2];
    let mut d = current_hash[3];
    let mut e = DeferredWord::Concrete(current_hash[4]);
    let mut f = current_hash[5];
    let mut g = current_hash[6];
    let mut h = current_hash[7];

    for index in 0..64 {
        let new_e = e.compute(recorder, &[])?;
        let mut s1 = new_e.rotr(6);
        s1 = s1.xor(recorder, new_e.rotr(11))?;
        s1 = s1.xor(recorder, new_e.rotr(25))?;
        let ch = ReplayWord::sha256_ch(recorder, new_e, f, g)?;
        let temp1 = [
            h,
            s1,
            ch,
            ReplayWord::constant(SHA256_ROUND_CONSTANTS[index]),
            w[index],
        ];

        let new_a = a.compute(recorder, &[])?;
        let mut s0 = new_a.rotr(2);
        s0 = s0.xor(recorder, new_a.rotr(13))?;
        s0 = s0.xor(recorder, new_a.rotr(22))?;
        let maj = ReplayWord::sha256_maj(recorder, new_a, b, c)?;
        let temp2 = [s0, maj];

        h = g;
        g = f;
        f = new_e;
        e = DeferredWord::Deferred(temp1.iter().copied().chain([d]).collect());
        d = c;
        c = b;
        b = new_a;
        a = DeferredWord::Deferred(temp1.iter().copied().chain(temp2).collect());
    }

    Ok([
        a.compute(recorder, &[current_hash[0]])?,
        ReplayWord::addmany(recorder, &[current_hash[1], b])?,
        ReplayWord::addmany(recorder, &[current_hash[2], c])?,
        ReplayWord::addmany(recorder, &[current_hash[3], d])?,
        e.compute(recorder, &[current_hash[4]])?,
        ReplayWord::addmany(recorder, &[current_hash[5], f])?,
        ReplayWord::addmany(recorder, &[current_hash[6], g])?,
        ReplayWord::addmany(recorder, &[current_hash[7], h])?,
    ])
}

/// Which private hash owns one uniform compression step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Figure9CompressionOwner {
    Birth,
    Issuer,
}

/// One exact source mapping for the historical uniform compression circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Figure9CompressionStep {
    index: usize,
    owner: Figure9CompressionOwner,
    owner_block_index: usize,
    block_be: [BitSource; SHA256_BLOCK_BITS],
    state_before_le: [[BitSource; SHA256_WORD_BITS]; SHA256_STATE_WORDS],
    state_after_le: [[BitSource; SHA256_WORD_BITS]; SHA256_STATE_WORDS],
}

impl Figure9CompressionStep {
    #[cfg(test)]
    pub(super) const fn index(&self) -> usize {
        self.index
    }

    #[cfg(test)]
    pub(super) const fn owner(&self) -> Figure9CompressionOwner {
        self.owner
    }

    #[cfg(test)]
    pub(super) const fn owner_block_index(&self) -> usize {
        self.owner_block_index
    }

    fn resolve_block(
        &self,
        shared_witness: &[Scalar],
    ) -> Result<[u8; SHA256_BLOCK_BYTES], Figure9SplitAdapterError> {
        let mut block = [0_u8; SHA256_BLOCK_BYTES];
        for (byte_index, byte) in block.iter_mut().enumerate() {
            for bit_from_msb in 0..8 {
                if self.block_be[byte_index * 8 + bit_from_msb].resolve(shared_witness)? {
                    *byte |= 1 << (7 - bit_from_msb);
                }
            }
        }
        Ok(block)
    }

    fn resolve_state(
        sources: &[[BitSource; SHA256_WORD_BITS]; SHA256_STATE_WORDS],
        shared_witness: &[Scalar],
    ) -> Result<[u32; SHA256_STATE_WORDS], Figure9SplitAdapterError> {
        let mut state = [0_u32; SHA256_STATE_WORDS];
        for (word_index, word) in state.iter_mut().enumerate() {
            for (bit, source) in sources[word_index].iter().enumerate() {
                if source.resolve(shared_witness)? {
                    *word |= 1 << bit;
                }
            }
        }
        Ok(state)
    }

    fn validate_assignment(
        &self,
        shared_witness: &[Scalar],
    ) -> Result<(), Figure9SplitAdapterError> {
        let block = self.resolve_block(shared_witness)?;
        let mut actual = Self::resolve_state(&self.state_before_le, shared_witness)?;
        let expected = Self::resolve_state(&self.state_after_le, shared_witness)?;
        sha256::compress(&mut actual, &block);
        if actual != expected {
            return Err(Figure9SplitAdapterError::UnsatisfiedStep);
        }
        Ok(())
    }
}

/// Validated shared/core projection for the exact Figure 9 Microsoft split.
///
/// The adapter borrows the only native witness owner and is intentionally not
/// `Clone`.  It is structural prover input, not proof or verifier authority.
pub(super) struct Figure9SplitWitnessAdapter<'a> {
    material: &'a Figure9McMaterial,
    steps: [Figure9CompressionStep; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    core_public_values: &'a [Scalar],
    #[cfg(test)]
    core_replayed_rows: usize,
    core_unpadded_constraints: usize,
}

/// Complete equation-checked application witness sections, before blinding.
///
/// Only the eight unpadded SHA rest sections are owned.  The 524,288-scalar
/// shared assignment remains borrowed, and padding is represented by shape
/// lengths rather than materialized vectors.  This owner is intentionally not
/// `Clone`; it is consumed by the governed commitment preparation stage.
pub(super) struct ValidatedFigure9Witnesses<'a> {
    pub(super) shared_witness: &'a [Scalar],
    pub(super) step_public_values: [Vec<Scalar>; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    pub(super) step_rest_values: [Figure9SecretScalars; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    pub(super) core_public_values: Vec<Scalar>,
}

impl<'a> Figure9SplitWitnessAdapter<'a> {
    pub(super) fn new(
        material: &'a Figure9McMaterial,
        step_public_values: &[Vec<Scalar>],
        core_public_values: &'a [Scalar],
    ) -> Result<Self, Figure9SplitAdapterError> {
        Self::from_parts(
            material,
            &material.topology,
            step_public_values,
            core_public_values,
            &canonical_figure9_dimensions(),
        )
    }

    fn from_parts(
        material: &'a Figure9McMaterial,
        topology: &Figure9McTopology,
        step_public_values: &[Vec<Scalar>],
        core_public_values: &'a [Scalar],
        dimensions: &VegaMdlProofDimensionsV1,
    ) -> Result<Self, Figure9SplitAdapterError> {
        validate_public_schedule(material, step_public_values, core_public_values, dimensions)?;
        validate_topology(material, topology)?;
        let steps = compression_steps(topology)?;
        for (index, step) in steps.iter().enumerate() {
            let (expected_owner, expected_block) = if index < 2 {
                (Figure9CompressionOwner::Birth, index)
            } else {
                (Figure9CompressionOwner::Issuer, index - 2)
            };
            if step.index != index
                || step.owner != expected_owner
                || step.owner_block_index != expected_block
            {
                return Err(Figure9SplitAdapterError::InvalidMetadata);
            }
            step.validate_assignment(&material.assignment.witness)?;
        }
        let core_replayed_rows = validate_core_assignment(material, topology, core_public_values)?;
        let core_unpadded_constraints = core_replayed_rows
            .checked_add(CORE_PUBLIC_SCALARS)
            .ok_or(Figure9SplitAdapterError::InvalidShape)?;
        validate_split_geometry(
            material,
            dimensions,
            core_replayed_rows,
            core_unpadded_constraints,
        )?;
        Ok(Self {
            material,
            steps,
            core_public_values,
            #[cfg(test)]
            core_replayed_rows,
            core_unpadded_constraints,
        })
    }

    pub(super) fn shared_witness(&self) -> &'a [Scalar] {
        &self.material.assignment.witness
    }

    #[cfg(test)]
    fn steps(&self) -> &[Figure9CompressionStep; VEGA_MDL_FIGURE9_SHA256_STEPS_V1] {
        &self.steps
    }

    #[cfg(test)]
    const fn core_replayed_rows(&self) -> usize {
        self.core_replayed_rows
    }

    pub(super) const fn core_unpadded_constraints(&self) -> usize {
        self.core_unpadded_constraints
    }

    pub(super) fn core_public_values(&self) -> &[Scalar] {
        self.core_public_values
    }

    fn reconstruct_step_rest(
        &self,
        selected: usize,
        maximum: usize,
    ) -> Result<Figure9SecretScalars, Figure9SplitAdapterError> {
        if selected >= self.steps.len() {
            return Err(Figure9SplitAdapterError::InvalidMetadata);
        }
        let mut recorder = RestWitnessRecorder::new(maximum)?;
        let selectors: [ReplayBoolean; VEGA_MDL_FIGURE9_SHA256_STEPS_V1] = (0
            ..VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
            .map(|index| recorder.allocate_bit(index == selected))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;

        let block_bits: [ReplayBoolean; SHA256_BLOCK_BITS] = (0..SHA256_BLOCK_BITS)
            .map(|bit| {
                replay_selected_bit(
                    &mut recorder,
                    self.shared_witness(),
                    &selectors,
                    core::array::from_fn(|step| self.steps[step].block_be[bit]),
                    selected,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;

        let current_state: [ReplayWord; SHA256_STATE_WORDS] = (0..SHA256_STATE_WORDS)
            .map(|word| {
                let bits_le = (0..SHA256_WORD_BITS)
                    .map(|bit| {
                        replay_selected_bit(
                            &mut recorder,
                            self.shared_witness(),
                            &selectors,
                            core::array::from_fn(|step| {
                                self.steps[step].state_before_le[word][bit]
                            }),
                            selected,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;
                Ok(ReplayWord::from_bits_le(bits_le))
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;

        let expected_state: [[ReplayBoolean; SHA256_WORD_BITS]; SHA256_STATE_WORDS] = (0
            ..SHA256_STATE_WORDS)
            .map(|word| {
                (0..SHA256_WORD_BITS)
                    .map(|bit| {
                        replay_selected_bit(
                            &mut recorder,
                            self.shared_witness(),
                            &selectors,
                            core::array::from_fn(|step| self.steps[step].state_after_le[word][bit]),
                            selected,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| Figure9SplitAdapterError::InvalidShape)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Figure9SplitAdapterError::InvalidShape)?;

        if recorder.values.len() != PINNED_SELECTION_REST_VALUES {
            return Err(Figure9SplitAdapterError::InvalidShape);
        }

        let actual_state = replay_sha256_compression(&mut recorder, &block_bits, &current_state)?;
        if recorder.values.len() != PINNED_STEP_REST_UNPADDED {
            return Err(Figure9SplitAdapterError::InvalidShape);
        }
        if actual_state.iter().zip(expected_state).any(
            |(actual, expected): (&ReplayWord, [ReplayBoolean; SHA256_WORD_BITS])| {
                actual
                    .bits_le
                    .iter()
                    .zip(expected)
                    .any(|(actual, expected)| actual.value() != expected.value())
            },
        ) {
            return Err(Figure9SplitAdapterError::UnsatisfiedStep);
        }
        Ok(recorder.finish())
    }

    /// Validate all nine governed equations and retain only their compact,
    /// unpadded witness sections for the later randomness boundary.
    pub(super) fn validated_governed_witnesses(
        &self,
        step_public_values: &[Vec<Scalar>],
        step_shape: &SplitShapeWire,
        core_shape: &SplitShapeWire,
    ) -> Result<ValidatedFigure9Witnesses<'a>, Figure9SplitAdapterError> {
        validate_governed_shape_geometry(self, step_shape, core_shape)?;
        let mut reconstructed = Vec::with_capacity(VEGA_MDL_FIGURE9_SHA256_STEPS_V1);
        for (index, public_values) in step_public_values.iter().enumerate() {
            let rest = self.reconstruct_step_rest(index, step_shape.rest_unpadded)?;
            if rest.len() != step_shape.rest_unpadded {
                return Err(Figure9SplitAdapterError::InvalidShape);
            }
            validate_split_witness_equations(
                step_shape,
                self.shared_witness(),
                public_values,
                &rest,
                public_values,
            )
            .map_err(|_| Figure9SplitAdapterError::UnsatisfiedStep)?;
            reconstructed.push(rest);
        }
        validate_split_witness_equations(
            core_shape,
            self.shared_witness(),
            self.core_public_values(),
            &[],
            self.core_public_values(),
        )
        .map_err(|_| Figure9SplitAdapterError::UnsatisfiedCore)?;
        Ok(ValidatedFigure9Witnesses {
            shared_witness: self.shared_witness(),
            step_public_values: step_public_values
                .to_vec()
                .try_into()
                .map_err(|_| Figure9SplitAdapterError::InvalidShape)?,
            step_rest_values: reconstructed
                .try_into()
                .map_err(|_| Figure9SplitAdapterError::InvalidShape)?,
            core_public_values: self.core_public_values().to_vec(),
        })
    }
}

fn replay_selected_bit(
    recorder: &mut RestWitnessRecorder,
    shared: &[Scalar],
    selectors: &[ReplayBoolean; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    candidates: [BitSource; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    selected: usize,
) -> Result<ReplayBoolean, Figure9SplitAdapterError> {
    let output = recorder.allocate_bit(
        candidates
            .get(selected)
            .copied()
            .ok_or(Figure9SplitAdapterError::InvalidMetadata)?
            .resolve(shared)?,
    )?;
    for (selector, candidate) in selectors.iter().copied().zip(candidates) {
        if let BitSource::Shared(shared_index) = candidate {
            let shared_value = *shared
                .get(shared_index)
                .ok_or(Figure9SplitAdapterError::InvalidMetadata)?;
            let selector_value = selector
                .raw_value()
                .ok_or(Figure9SplitAdapterError::InvalidShape)?;
            recorder.allocate_scalar(if selector_value {
                shared_value
            } else {
                Scalar::zero()
            })?;
        }
    }
    Ok(output)
}

fn validate_governed_shape_geometry(
    adapter: &Figure9SplitWitnessAdapter<'_>,
    step: &SplitShapeWire,
    core: &SplitShapeWire,
) -> Result<(), Figure9SplitAdapterError> {
    let dimensions = canonical_figure9_dimensions();
    let common_invalid = |shape: &SplitShapeWire, expected_rest: usize| {
        shape.shared_unpadded != adapter.shared_witness().len()
            || shape.shared != dimensions.shared_variables
            || shape.rest != expected_rest
            || shape.challenges != 0
    };
    if common_invalid(step, dimensions.step_rest_variables)
        || common_invalid(core, dimensions.core_rest_variables)
        || step.constraints != dimensions.step_constraints
        || step.precommitted_unpadded != 1
        || step.precommitted != dimensions.step_precommitted_variables
        || step.rest_unpadded != PINNED_STEP_REST_UNPADDED
        || step.public_values != dimensions.step_public_values
        || core.constraints != dimensions.core_constraints
        || core.constraints_unpadded != adapter.core_unpadded_constraints()
        || core.precommitted_unpadded != adapter.core_public_values().len()
        || core.precommitted != dimensions.core_precommitted_variables
        || core.rest_unpadded != 0
        || core.public_values != dimensions.core_public_values
    {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    Ok(())
}

fn validate_split_witness_equations(
    shape: &SplitShapeWire,
    shared: &[Scalar],
    precommitted: &[Scalar],
    rest: &[Scalar],
    public_values: &[Scalar],
) -> Result<(), Figure9SplitAdapterError> {
    if shared.len() != shape.shared_unpadded
        || precommitted.len() != shape.precommitted_unpadded
        || rest.len() != shape.rest_unpadded
        || public_values.len() != shape.public_values
        || shape.challenges != 0
    {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    for row in 0..shape.constraints {
        let a = evaluate_split_row(
            &shape.a,
            row,
            shape,
            shared,
            precommitted,
            rest,
            public_values,
        )?;
        let b = evaluate_split_row(
            &shape.b,
            row,
            shape,
            shared,
            precommitted,
            rest,
            public_values,
        )?;
        let c = evaluate_split_row(
            &shape.c,
            row,
            shape,
            shared,
            precommitted,
            rest,
            public_values,
        )?;
        if a * b != c {
            return Err(Figure9SplitAdapterError::UnsatisfiedStep);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn evaluate_split_row(
    matrix: &SparseMatrixWire,
    row: usize,
    shape: &SplitShapeWire,
    shared: &[Scalar],
    precommitted: &[Scalar],
    rest: &[Scalar],
    public_values: &[Scalar],
) -> Result<Scalar, Figure9SplitAdapterError> {
    matrix
        .row_entries(row)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?
        .try_fold(Scalar::zero(), |sum, (column, coefficient)| {
            let value =
                split_column_value(column, shape, shared, precommitted, rest, public_values)?;
            Ok(sum + coefficient * value)
        })
}

fn split_column_value(
    column: usize,
    shape: &SplitShapeWire,
    shared: &[Scalar],
    precommitted: &[Scalar],
    rest: &[Scalar],
    public_values: &[Scalar],
) -> Result<Scalar, Figure9SplitAdapterError> {
    let precommitted_start = shape.shared;
    let rest_start = precommitted_start
        .checked_add(shape.precommitted)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    let public_start = rest_start
        .checked_add(shape.rest)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    let value = if column < precommitted_start {
        shared.get(column).copied().unwrap_or_else(Scalar::zero)
    } else if column < rest_start {
        precommitted
            .get(column - precommitted_start)
            .copied()
            .unwrap_or_else(Scalar::zero)
    } else if column < public_start {
        rest.get(column - rest_start)
            .copied()
            .unwrap_or_else(Scalar::zero)
    } else if column == public_start {
        Scalar::one()
    } else {
        public_values
            .get(column - public_start - 1)
            .copied()
            .ok_or(Figure9SplitAdapterError::InvalidShape)?
    };
    Ok(value)
}

fn validate_public_schedule(
    material: &Figure9McMaterial,
    step_public_values: &[Vec<Scalar>],
    core_public_values: &[Scalar],
    dimensions: &VegaMdlProofDimensionsV1,
) -> Result<(), Figure9SplitAdapterError> {
    if dimensions.num_steps != VEGA_MDL_FIGURE9_SHA256_STEPS_V1
        || dimensions.step_public_values != 1
        || dimensions.step_challenges != 0
        || dimensions.core_public_values != CORE_PUBLIC_SCALARS
        || dimensions.core_challenges != 0
        || step_public_values.len() != VEGA_MDL_FIGURE9_SHA256_STEPS_V1
        || step_public_values
            .iter()
            .enumerate()
            .any(|(index, values)| values.as_slice() != [Scalar::from_u64(index as u64)].as_slice())
        || core_public_values.len() != CORE_PUBLIC_SCALARS
        || material.assignment.public_inputs.len() != VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1
        || &core_public_values[..VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1]
            != material.assignment.public_inputs.as_slice()
    {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }
    Ok(())
}

fn validate_topology(
    material: &Figure9McMaterial,
    topology: &Figure9McTopology,
) -> Result<(), Figure9SplitAdapterError> {
    let shape = material.assignment.shape.as_ref();
    let witness = &material.assignment.witness;
    if topology != &material.topology
        || witness.len() != shape.variable_count()
        || topology.issuer_byte_bits_le.len()
            != VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1
        || topology.birth_byte_bits_le.len() != VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1
        || topology.issuer_states_after_blocks_le.len() != 6
        || topology.birth_states_after_blocks_le.len() != 2
    {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }
    let [birth_rows, issuer_rows] = &topology.excluded_sha256_rows;
    if birth_rows != &PINNED_BIRTH_SHA_ROWS
        || issuer_rows != &PINNED_ISSUER_SHA_ROWS
        || birth_rows.is_empty()
        || issuer_rows.is_empty()
        || birth_rows.end > issuer_rows.start
        || issuer_rows.end > shape.constraint_count()
        || topology
            .excluded_sha256_rows
            .iter()
            .flat_map(|rows| rows.clone())
            .any(|row| row_is_empty(shape, row))
    {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }

    let mut byte_indices = BTreeSet::new();
    for (byte, bits) in topology
        .issuer_byte_bits_le
        .iter()
        .chain(&topology.birth_byte_bits_le)
        .enumerate()
    {
        for (bit, index) in bits.iter().copied().enumerate() {
            let expected = byte
                .checked_mul(8)
                .and_then(|index| index.checked_add(bit))
                .ok_or(Figure9SplitAdapterError::InvalidMetadata)?;
            if index != expected || !byte_indices.insert(index) {
                return Err(Figure9SplitAdapterError::InvalidMetadata);
            }
            BitSource::Shared(index).resolve(witness)?;
        }
    }
    let mut state_indices = BTreeSet::new();
    for index in topology
        .issuer_states_after_blocks_le
        .iter()
        .chain(&topology.birth_states_after_blocks_le)
        .flat_map(|state| state.iter())
        .flatten()
        .copied()
    {
        if byte_indices.contains(&index) || !state_indices.insert(index) {
            return Err(Figure9SplitAdapterError::InvalidMetadata);
        }
        BitSource::Shared(index).resolve(witness)?;
    }
    Ok(())
}

fn compression_steps(
    topology: &Figure9McTopology,
) -> Result<[Figure9CompressionStep; VEGA_MDL_FIGURE9_SHA256_STEPS_V1], Figure9SplitAdapterError> {
    let birth_blocks = padded_blocks(&topology.birth_byte_bits_le)?;
    let issuer_blocks = padded_blocks(&topology.issuer_byte_bits_le)?;
    if birth_blocks.len() != 2 || issuer_blocks.len() != 6 {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }
    let mut steps = Vec::with_capacity(VEGA_MDL_FIGURE9_SHA256_STEPS_V1);
    append_hash_steps(
        &mut steps,
        Figure9CompressionOwner::Birth,
        &birth_blocks,
        &topology.birth_states_after_blocks_le,
    )?;
    append_hash_steps(
        &mut steps,
        Figure9CompressionOwner::Issuer,
        &issuer_blocks,
        &topology.issuer_states_after_blocks_le,
    )?;
    steps
        .try_into()
        .map_err(|_| Figure9SplitAdapterError::InvalidMetadata)
}

fn append_hash_steps(
    output: &mut Vec<Figure9CompressionStep>,
    owner: Figure9CompressionOwner,
    blocks: &[[BitSource; SHA256_BLOCK_BITS]],
    states_after: &[[[usize; SHA256_WORD_BITS]; SHA256_STATE_WORDS]],
) -> Result<(), Figure9SplitAdapterError> {
    if blocks.len() != states_after.len() {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }
    for (owner_block_index, block_be) in blocks.iter().copied().enumerate() {
        let state_before_le = if owner_block_index == 0 {
            sha256::INITIAL_STATE
                .map(|word| core::array::from_fn(|bit| BitSource::Constant(word & (1 << bit) != 0)))
        } else {
            states_after[owner_block_index - 1].map(|word| word.map(BitSource::Shared))
        };
        let state_after_le =
            states_after[owner_block_index].map(|word| word.map(BitSource::Shared));
        output.push(Figure9CompressionStep {
            index: output.len(),
            owner,
            owner_block_index,
            block_be,
            state_before_le,
            state_after_le,
        });
    }
    Ok(())
}

fn padded_blocks(
    message: &[[usize; 8]],
) -> Result<Vec<[BitSource; SHA256_BLOCK_BITS]>, Figure9SplitAdapterError> {
    let bit_length = u64::try_from(message.len())
        .ok()
        .and_then(|length| length.checked_mul(8))
        .ok_or(Figure9SplitAdapterError::InvalidMetadata)?;
    let padded_len = message
        .len()
        .checked_add(9)
        .and_then(|length| length.checked_add(SHA256_BLOCK_BYTES - 1))
        .map(|length| length / SHA256_BLOCK_BYTES * SHA256_BLOCK_BYTES)
        .ok_or(Figure9SplitAdapterError::InvalidMetadata)?;
    let mut bytes = message
        .iter()
        .copied()
        .map(|bits| bits.map(BitSource::Shared))
        .collect::<Vec<_>>();
    bytes.push(constant_byte(0x80));
    while bytes.len() + 8 < padded_len {
        bytes.push(constant_byte(0));
    }
    bytes.extend(bit_length.to_be_bytes().map(constant_byte));
    if bytes.len() != padded_len {
        return Err(Figure9SplitAdapterError::InvalidMetadata);
    }
    bytes
        .chunks_exact(SHA256_BLOCK_BYTES)
        .map(|block| {
            block
                .iter()
                .flat_map(|byte| byte.iter().rev().copied())
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| Figure9SplitAdapterError::InvalidMetadata)
        })
        .collect()
}

fn constant_byte(byte: u8) -> [BitSource; 8] {
    core::array::from_fn(|bit| BitSource::Constant(byte & (1 << bit) != 0))
}

fn validate_core_assignment(
    material: &Figure9McMaterial,
    topology: &Figure9McTopology,
    core_public_values: &[Scalar],
) -> Result<usize, Figure9SplitAdapterError> {
    let shape = material.assignment.shape.as_ref();
    let mut replayed = 0_usize;
    for row in 0..shape.constraint_count() {
        if is_excluded_sha256_row(topology, row) || row_is_empty(shape, row) {
            continue;
        }
        let a = evaluate_row(
            &shape.a,
            row,
            shape,
            &material.assignment.witness,
            core_public_values,
        )?;
        let b = evaluate_row(
            &shape.b,
            row,
            shape,
            &material.assignment.witness,
            core_public_values,
        )?;
        let c = evaluate_row(
            &shape.c,
            row,
            shape,
            &material.assignment.witness,
            core_public_values,
        )?;
        if a * b != c {
            return Err(Figure9SplitAdapterError::UnsatisfiedCore);
        }
        replayed = replayed
            .checked_add(1)
            .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    }
    if replayed == 0 {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    Ok(replayed)
}

fn evaluate_row(
    matrix: &SparseMatrix,
    row: usize,
    shape: &Shape,
    shared_witness: &[Scalar],
    core_public_values: &[Scalar],
) -> Result<Scalar, Figure9SplitAdapterError> {
    matrix
        .row_entries(row)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?
        .try_fold(Scalar::zero(), |sum, (column, coefficient)| {
            let value = if column < shape.variable_count() {
                *shared_witness
                    .get(column)
                    .ok_or(Figure9SplitAdapterError::InvalidShape)?
            } else if column == shape.variable_count() {
                Scalar::one()
            } else {
                *core_public_values
                    .get(column - shape.variable_count() - 1)
                    .ok_or(Figure9SplitAdapterError::InvalidShape)?
            };
            Ok(sum + coefficient * value)
        })
}

fn validate_split_geometry(
    material: &Figure9McMaterial,
    dimensions: &VegaMdlProofDimensionsV1,
    core_replayed_rows: usize,
    core_unpadded_constraints: usize,
) -> Result<(), Figure9SplitAdapterError> {
    let shared_unpadded = material.assignment.witness.len();
    let shared = pad_to_width(shared_unpadded, MICROSOFT_COMMITMENT_WIDTH)?;
    let step_precommitted = pad_to_width(1, MICROSOFT_COMMITMENT_WIDTH)?;
    let core_precommitted = pad_to_width(CORE_PUBLIC_SCALARS, MICROSOFT_COMMITMENT_WIDTH)?;
    let core_variables_before_power_of_two = shared
        .checked_add(core_precommitted)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    let core_variables = core_variables_before_power_of_two
        .checked_next_power_of_two()
        .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    let core_rest = core_variables
        .checked_sub(core_variables_before_power_of_two)
        .ok_or(Figure9SplitAdapterError::InvalidShape)?;
    // The monolithic native shape has 1,048,576 padded rows because it still
    // contains both SHA gadgets. The governed Microsoft core intentionally
    // excludes the two pinned SHA ranges and pads only the 183,400 retained
    // core/inputization rows to 262,144. Comparing the native padded row count
    // to `core_constraints` would therefore reject the historical split.
    if core_replayed_rows != PINNED_CORE_REPLAYED_ROWS
        || core_unpadded_constraints != PINNED_CORE_UNPADDED_CONSTRAINTS
        || dimensions.shared_variables != shared
        || dimensions.step_precommitted_variables != step_precommitted
        || dimensions.step_rest_variables != core_rest
        || dimensions.step_variables != core_variables
        || dimensions.step_constraints != 262_144
        || dimensions.core_precommitted_variables != core_precommitted
        || dimensions.core_rest_variables != core_rest
        || dimensions.core_variables != core_variables
        || core_unpadded_constraints
            .checked_next_power_of_two()
            .ok_or(Figure9SplitAdapterError::InvalidShape)?
            != dimensions.core_constraints
    {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    Ok(())
}

fn pad_to_width(value: usize, width: usize) -> Result<usize, Figure9SplitAdapterError> {
    if width == 0 || !width.is_power_of_two() {
        return Err(Figure9SplitAdapterError::InvalidShape);
    }
    value
        .checked_add(width - 1)
        .map(|value| value / width * width)
        .ok_or(Figure9SplitAdapterError::InvalidShape)
}

fn is_excluded_sha256_row(topology: &Figure9McTopology, row: usize) -> bool {
    topology
        .excluded_sha256_rows
        .iter()
        .any(|range| range.contains(&row))
}

fn row_is_empty(shape: &Shape, row: usize) -> bool {
    [&shape.a, &shape.b, &shape.c].into_iter().all(|matrix| {
        matrix
            .row_entries(row)
            .is_some_and(|mut entries| entries.next().is_none())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{
        circuit::CircuitAssignment,
        figure9::{synthesize_figure9_mc_material, tests::baseline_signed_fixture},
    };
    use std::sync::Arc;

    fn public_schedule(material: &Figure9McMaterial) -> (Vec<Vec<Scalar>>, Vec<Scalar>) {
        let steps = (0..VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
            .map(|index| vec![Scalar::from_u64(index as u64)])
            .collect();
        let mut core = material.assignment.public_inputs.clone();
        core.extend((0..CONTEXT_PUBLIC_SCALARS).map(|index| Scalar::from_u64(101 + index as u64)));
        (steps, core)
    }

    fn padded_message_blocks(message: &[u8]) -> Vec<[u8; SHA256_BLOCK_BYTES]> {
        let bit_length = u64::try_from(message.len())
            .expect("fixture length")
            .checked_mul(8)
            .expect("fixture bit length");
        let mut padded = message.to_vec();
        padded.push(0x80);
        while !(padded.len() + 8).is_multiple_of(SHA256_BLOCK_BYTES) {
            padded.push(0);
        }
        padded.extend_from_slice(&bit_length.to_be_bytes());
        padded
            .chunks_exact(SHA256_BLOCK_BYTES)
            .map(|block| block.try_into().expect("exact padded block"))
            .collect()
    }

    fn replace_witness(material: &Figure9McMaterial, witness: Vec<Scalar>) -> Figure9McMaterial {
        Figure9McMaterial {
            assignment: CircuitAssignment {
                shape: Arc::clone(&material.assignment.shape),
                witness,
                public_inputs: material.assignment.public_inputs.clone(),
            },
            topology: material.topology.clone(),
        }
    }

    fn expected_selected_value(source: BitSource, shared: &[Scalar]) -> Scalar {
        match source {
            BitSource::Constant(value) => {
                if value {
                    Scalar::one()
                } else {
                    Scalar::zero()
                }
            }
            BitSource::Shared(index) => shared[index],
        }
    }

    fn expected_selection_rest(
        adapter: &Figure9SplitWitnessAdapter<'_>,
        selected: usize,
    ) -> Vec<Scalar> {
        let shared = adapter.shared_witness();
        let selectors: [bool; VEGA_MDL_FIGURE9_SHA256_STEPS_V1] =
            core::array::from_fn(|index| index == selected);
        let mut expected = selectors
            .map(|value| if value { Scalar::one() } else { Scalar::zero() })
            .to_vec();
        let mut append_selected = |candidates: [BitSource; VEGA_MDL_FIGURE9_SHA256_STEPS_V1]| {
            expected.push(expected_selected_value(candidates[selected], shared));
            for (selector, candidate) in selectors.into_iter().zip(candidates) {
                if let BitSource::Shared(index) = candidate {
                    expected.push(if selector {
                        shared[index]
                    } else {
                        Scalar::zero()
                    });
                }
            }
        };
        for bit in 0..SHA256_BLOCK_BITS {
            append_selected(core::array::from_fn(|step| {
                adapter.steps()[step].block_be[bit]
            }));
        }
        for word in 0..SHA256_STATE_WORDS {
            for bit in 0..SHA256_WORD_BITS {
                append_selected(core::array::from_fn(|step| {
                    adapter.steps()[step].state_before_le[word][bit]
                }));
            }
        }
        for word in 0..SHA256_STATE_WORDS {
            for bit in 0..SHA256_WORD_BITS {
                append_selected(core::array::from_fn(|step| {
                    adapter.steps()[step].state_after_le[word][bit]
                }));
            }
        }
        expected
    }

    #[test]
    fn secret_scalar_owner_zeroizes_on_success_error_and_unwind() {
        fn owner(values: &[u64]) -> Figure9SecretScalars {
            let mut owner = Figure9SecretScalars::with_capacity(values.len());
            for value in values {
                owner.push(Scalar::from_u64(*value));
            }
            owner
        }

        fn error_path() -> Result<(), ()> {
            let _owned = owner(&[3, 5, 7]);
            Err(())
        }

        let before_success = figure9_secret_scalar_owner_drop_count();
        drop(owner(&[1, 2]));
        assert_eq!(figure9_secret_scalar_owner_drop_count(), before_success + 1);

        let before_error = figure9_secret_scalar_owner_drop_count();
        assert_eq!(error_path(), Err(()));
        assert_eq!(figure9_secret_scalar_owner_drop_count(), before_error + 1);

        let before_unwind = figure9_secret_scalar_owner_drop_count();
        let unwind = std::panic::catch_unwind(|| {
            let _owned = owner(&[11, 13]);
            panic!("injected Figure 9 witness-owner unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(figure9_secret_scalar_owner_drop_count(), before_unwind + 1);

        let source = include_str!("split_adapter.rs");
        assert!(source.contains("impl Drop for Figure9SecretScalars"));
        assert!(source.contains("impl Drop for RestWitnessRecorder"));
        assert!(source.contains("values: Vec::with_capacity(maximum)"));
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "full Figure 9 split assignment validation is a release-mode circuit gate"
    )]
    fn exact_split_metadata_steps_core_rows_and_mutations_are_pinned() {
        let fixture = baseline_signed_fixture();
        let material = synthesize_figure9_mc_material(&fixture.public, &fixture.witness())
            .expect("signed native Figure 9 material");
        let (step_public, core_public) = public_schedule(&material);
        let adapter = Figure9SplitWitnessAdapter::new(&material, &step_public, &core_public)
            .expect("exact split projection");

        assert_eq!(adapter.shared_witness().len(), 524_288);
        assert_eq!(adapter.steps().len(), 8);
        assert_eq!(material.assignment.shape.variable_count(), 524_288);
        assert_eq!(material.assignment.shape.constraint_count(), 1_048_576);
        let birth_blocks = padded_message_blocks(&fixture.birth);
        let issuer_blocks = padded_message_blocks(&fixture.issuer);
        for (index, step) in adapter.steps().iter().enumerate() {
            assert_eq!(step.index(), index);
            let (owner, block) = if index < 2 {
                (Figure9CompressionOwner::Birth, index)
            } else {
                (Figure9CompressionOwner::Issuer, index - 2)
            };
            assert_eq!(step.owner(), owner);
            assert_eq!(step.owner_block_index(), block);
            let expected_block = match owner {
                Figure9CompressionOwner::Birth => birth_blocks[block],
                Figure9CompressionOwner::Issuer => issuer_blocks[block],
            };
            assert_eq!(
                step.resolve_block(adapter.shared_witness())
                    .expect("exact shared block"),
                expected_block
            );
            step.validate_assignment(adapter.shared_witness())
                .expect("shared compression transition");

            let rest = adapter
                .reconstruct_step_rest(index, PINNED_STEP_REST_UNPADDED)
                .expect("exact Bellpepper step rest witness");
            assert_eq!(rest.len(), PINNED_STEP_REST_UNPADDED);
            let selection = expected_selection_rest(&adapter, index);
            assert_eq!(selection.len(), PINNED_SELECTION_REST_VALUES);
            assert_eq!(&rest[..PINNED_SELECTION_REST_VALUES], selection);
        }
        assert_eq!(adapter.core_public_values(), core_public);
        assert_eq!(adapter.core_replayed_rows(), PINNED_CORE_REPLAYED_ROWS);
        assert_eq!(
            adapter.core_unpadded_constraints(),
            PINNED_CORE_UNPADDED_CONSTRAINTS
        );
        assert_eq!(
            adapter.core_unpadded_constraints(),
            adapter.core_replayed_rows() + 18
        );
        assert_eq!(
            adapter.core_unpadded_constraints().next_power_of_two(),
            262_144
        );

        let shape = material.assignment.shape.as_ref();
        assert_eq!(
            material.topology.excluded_sha256_rows,
            [PINNED_BIRTH_SHA_ROWS, PINNED_ISSUER_SHA_ROWS]
        );
        let manually_replayed = (0..shape.constraint_count())
            .filter(|row| {
                !is_excluded_sha256_row(&material.topology, *row) && !row_is_empty(shape, *row)
            })
            .count();
        assert_eq!(adapter.core_replayed_rows(), manually_replayed);
        for range in &material.topology.excluded_sha256_rows {
            assert!(!range.is_empty());
            assert!(range.clone().all(|row| !row_is_empty(shape, row)));
        }

        let mut wrong_core = core_public.clone();
        wrong_core[10] += Scalar::one();
        assert_eq!(
            Figure9SplitWitnessAdapter::new(&material, &step_public, &wrong_core).err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );
        let mut wrong_steps = step_public.clone();
        wrong_steps.swap(0, 1);
        assert_eq!(
            Figure9SplitWitnessAdapter::new(&material, &wrong_steps, &core_public).err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );

        let mut malformed = material.topology.clone();
        malformed.birth_byte_bits_le.pop();
        assert_eq!(
            Figure9SplitWitnessAdapter::from_parts(
                &material,
                &malformed,
                &step_public,
                &core_public,
                &canonical_figure9_dimensions(),
            )
            .err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );
        let mut malformed = material.topology.clone();
        malformed.issuer_states_after_blocks_le.pop();
        assert_eq!(
            Figure9SplitWitnessAdapter::from_parts(
                &material,
                &malformed,
                &step_public,
                &core_public,
                &canonical_figure9_dimensions(),
            )
            .err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );
        let mut malformed = material.topology.clone();
        malformed.birth_byte_bits_le[0][0] = material.assignment.witness.len();
        assert_eq!(
            Figure9SplitWitnessAdapter::from_parts(
                &material,
                &malformed,
                &step_public,
                &core_public,
                &canonical_figure9_dimensions(),
            )
            .err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );
        let mut malformed = material.topology.clone();
        malformed.excluded_sha256_rows[0].end = malformed.excluded_sha256_rows[1].start + 1;
        assert_eq!(
            Figure9SplitWitnessAdapter::from_parts(
                &material,
                &malformed,
                &step_public,
                &core_public,
                &canonical_figure9_dimensions(),
            )
            .err(),
            Some(Figure9SplitAdapterError::InvalidMetadata)
        );

        let dimension_mutations: [fn(&mut VegaMdlProofDimensionsV1); 4] = [
            |dimensions| dimensions.shared_variables -= 1,
            |dimensions| dimensions.step_rest_variables -= 1,
            |dimensions| dimensions.step_constraints -= 1,
            |dimensions| dimensions.core_constraints -= 1,
        ];
        for mutate_dimensions in dimension_mutations {
            let mut malformed_dimensions = canonical_figure9_dimensions();
            mutate_dimensions(&mut malformed_dimensions);
            assert_eq!(
                Figure9SplitWitnessAdapter::from_parts(
                    &material,
                    &material.topology,
                    &step_public,
                    &core_public,
                    &malformed_dimensions,
                )
                .err(),
                Some(Figure9SplitAdapterError::InvalidShape)
            );
        }

        let step_bit = material.topology.birth_states_after_blocks_le[0][0][0];
        let mut wrong_step_witness = material.assignment.witness.clone();
        wrong_step_witness[step_bit] = Scalar::one() - wrong_step_witness[step_bit];
        let wrong_step_material = replace_witness(&material, wrong_step_witness);
        assert_eq!(
            Figure9SplitWitnessAdapter::new(&wrong_step_material, &step_public, &core_public,)
                .err(),
            Some(Figure9SplitAdapterError::UnsatisfiedStep)
        );

        let mut core_columns = BTreeSet::new();
        let mut sha_columns = BTreeSet::new();
        for row in 0..shape.constraint_count() {
            let destination = if is_excluded_sha256_row(&material.topology, row) {
                &mut sha_columns
            } else if row_is_empty(shape, row) {
                continue;
            } else {
                &mut core_columns
            };
            for matrix in [&shape.a, &shape.b, &shape.c] {
                destination.extend(
                    matrix
                        .row_entries(row)
                        .expect("bounded native row")
                        .map(|(column, _)| column)
                        .filter(|column| *column < shape.variable_count()),
                );
            }
        }
        let core_only = core_columns
            .difference(&sha_columns)
            .copied()
            .next()
            .expect("core replay owns a private variable outside SHA rows");
        let mut wrong_core_witness = material.assignment.witness.clone();
        wrong_core_witness[core_only] += Scalar::one();
        let wrong_core_material = replace_witness(&material, wrong_core_witness);
        assert_eq!(
            Figure9SplitWitnessAdapter::new(&wrong_core_material, &step_public, &core_public,)
                .err(),
            Some(Figure9SplitAdapterError::UnsatisfiedCore)
        );

        let sha_only = sha_columns
            .difference(&core_columns)
            .copied()
            .find(|index| {
                matches!(
                    material.assignment.witness.get(*index),
                    Some(value) if *value == Scalar::zero() || *value == Scalar::one()
                )
            })
            .expect("SHA rows own at least one private bit");
        let mut mutated_witness = material.assignment.witness.clone();
        mutated_witness[sha_only] = Scalar::one() - mutated_witness[sha_only];
        let mutated = replace_witness(&material, mutated_witness);
        assert!(
            validate_core_assignment(&mutated, &mutated.topology, &core_public).is_ok(),
            "the core replay must not silently re-add an excluded SHA-only row"
        );
        assert!(
            mutated
                .assignment
                .shape
                .validate_strict_assignment(
                    &mutated.assignment.witness,
                    &mutated.assignment.public_inputs,
                )
                .is_err(),
            "the same SHA-only mutation must fail the monolithic relation"
        );
    }

    #[test]
    fn split_witness_stream_maps_sections_and_rejects_equation_mutations() {
        let matrix = |indices: [usize; 2]| SparseMatrixWire {
            data: vec![Scalar::one(); 2],
            indices: indices.to_vec(),
            row_offsets: vec![0, 1, 2],
            columns: 6,
        };
        let shape = SplitShapeWire {
            constraints: 2,
            constraints_unpadded: 2,
            shared_unpadded: 1,
            precommitted_unpadded: 1,
            rest_unpadded: 1,
            shared: 1,
            precommitted: 1,
            rest: 2,
            public_values: 1,
            challenges: 0,
            // row 0: shared * precommitted = rest
            // row 1: precommitted * one = public
            a: matrix([0, 1]),
            b: matrix([1, 4]),
            c: matrix([2, 5]),
        };
        let shared = [Scalar::from_u64(2)];
        let precommitted = [Scalar::from_u64(3)];
        let rest = [Scalar::from_u64(6)];
        let public = [Scalar::from_u64(3)];
        validate_split_witness_equations(&shape, &shared, &precommitted, &rest, &public)
            .expect("streamed split witness satisfies both rows");

        assert_eq!(
            validate_split_witness_equations(
                &shape,
                &shared,
                &precommitted,
                &[Scalar::from_u64(7)],
                &public,
            ),
            Err(Figure9SplitAdapterError::UnsatisfiedStep)
        );
        assert_eq!(
            validate_split_witness_equations(
                &shape,
                &shared,
                &precommitted,
                &rest,
                &[Scalar::from_u64(4)],
            ),
            Err(Figure9SplitAdapterError::UnsatisfiedStep)
        );
        assert_eq!(
            validate_split_witness_equations(&shape, &[], &precommitted, &rest, &public),
            Err(Figure9SplitAdapterError::InvalidShape)
        );
    }

    #[test]
    fn source_contract_keeps_the_adapter_dependency_free_and_non_authorizing() {
        let source = include_str!("split_adapter.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production split adapter");
        assert!(production.contains("repository commit `60b24f71eb`"));
        assert!(production.contains("Bellpepper 0.4.1"));
        assert!(production.contains("PINNED_STEP_REST_UNPADDED"));
        assert!(production.contains("replay_sha256_compression"));
        assert!(production.contains("sha256::compress(&mut actual, &block)"));
        assert!(production.contains("validate_core_assignment"));
        assert!(!production.contains("use bellpepper"));
        assert!(!production.contains("VegaMcZkSNARK"));
        assert!(!production.contains("encode_iroha_canonical"));
        assert!(!production.contains("fill_bytes"));
        assert!(
            !production.contains("#[derive(Clone)]\npub(super) struct Figure9SplitWitnessAdapter")
        );
        assert_eq!(PINNED_STEP_REST_UNPADDED, 34_462);
        assert_eq!(
            BELLPEPPER_SHA256_SOURCE_SHA256,
            "e02e54f9a3a4a81c2d241cb75a42199c388d98ee1b9eea796e8bc08c9e099df3"
        );
        assert_eq!(
            BELLPEPPER_UINT32_SOURCE_SHA256,
            "98dcc6388d44291f0fecb9adfef156e69ff1d386aa002001dcaf2bee0f876768"
        );
        assert_eq!(
            BELLPEPPER_MULTIEQ_SOURCE_SHA256,
            "f016e3e5d33c15e459f8c9e51a71fc5495f69ad33b83d32f04c4418b70c7260e"
        );
        assert_eq!(
            BELLPEPPER_CORE_BOOLEAN_SOURCE_SHA256,
            "8471d4b24b03662d96137c24ddf0adebc097e573e73f64b75796eb81230468e0"
        );
    }
}
