//! Immutable typed-domain prefix reuse for the canonical six-lane digest.
//!
//! This isolated helper caches the existing framing and permutation through
//! domain tag 7. It reads the canonical lane IVs and round constants through
//! public accessors; the source-committed one-shot and stream implementations
//! remain independent and unchanged. No digest coordinate, lane separation,
//! field delimiter, or integer byte encoding is changed.

use std::sync::{Arc, OnceLock};

use crate::{
    poseidon::{FIELD_MODULUS, MDS, RATE, STATE_WIDTH},
    poseidon_digest384::{
        GOLDILOCKS_DIGEST384_LANES_V1, GOLDILOCKS_DIGEST384_ROUNDS_V1,
        GoldilocksDigest384LastFieldStreamErrorV1, GoldilocksDigest384LastFieldStreamV1,
        GoldilocksDigest384V1, GoldilocksDigestDomainV1,
        goldilocks_digest384_lane_initial_state_v1, goldilocks_digest384_lane_round_constants_v1,
    },
};

// Exact canonical framing constants. Oracle tests below cover every field,
// empty/partial byte chunks, full-width integers and all independent lanes.
const MAX_FRAMED_FIELD_BYTES_V1: usize = u32::MAX as usize;
const MESSAGE_FRAME_DOMAIN_V1: &[u8] = b"iroha:goldilocks-digest384:message-frame:v1";
const FULL_ROUNDS_HALF_V1: usize = 4;
const PARTIAL_ROUNDS_V1: usize = 57;
type LaneRoundConstants = [[u64; STATE_WIDTH]; GOLDILOCKS_DIGEST384_ROUNDS_V1];

// Fixed-size public parameter copies avoid an accessor/OnceLock lookup in
// every permutation round. This is not a mutable domain or witness cache.
static ROUND_CONSTANTS: OnceLock<[LaneRoundConstants; GOLDILOCKS_DIGEST384_LANES_V1]> =
    OnceLock::new();

fn round_constants() -> &'static [LaneRoundConstants; GOLDILOCKS_DIGEST384_LANES_V1] {
    ROUND_CONSTANTS.get_or_init(|| {
        core::array::from_fn(|lane| {
            core::array::from_fn(|round| {
                goldilocks_digest384_lane_round_constants_v1(lane, round)
                    .expect("canonical lane and round indices")
            })
        })
    })
}

/// Immutable six-lane snapshot through the typed domain's tree-level field.
///
/// Construction absorbs canonical tags 1 through 7 once. Every [`Self::hash_at`]
/// resumes from a copy, binding tag 8 to the selected index, tag 9 to the original
/// counter, and tag 10 to each independent lane. Domain byte fields are borrowed
/// without allocation so [`Self::last_field_stream_at`] can preserve the exact
/// canonical streaming API. The cache is immutable and shareable between workers;
/// its borrowed domain fields must remain alive while the cache is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoldilocksDigest384DomainPrefixV1<'a> {
    lane_states: [[u64; STATE_WIDTH]; GOLDILOCKS_DIGEST384_LANES_V1],
    pending: [u64; RATE],
    pending_len: usize,
    domain: GoldilocksDigestDomainV1<'a>,
}

impl<'a> GoldilocksDigest384DomainPrefixV1<'a> {
    /// Cache the exact typed prefix through tag 7, borrowing its complete domain.
    ///
    /// Returns `None` if a domain byte field exceeds the canonical 32-bit
    /// framing ceiling. Full-width integers are framed as eight little-endian
    /// bytes, so every `u64` index, level, and counter is representable.
    #[must_use]
    pub fn new(domain: GoldilocksDigestDomainV1<'a>) -> Option<Self> {
        if [
            domain.catalog,
            domain.protocol,
            domain.profile,
            domain.role,
            domain.phase,
        ]
        .iter()
        .any(|field| field.len() > MAX_FRAMED_FIELD_BYTES_V1)
        {
            return None;
        }
        let level = domain.level.to_le_bytes();
        let lanes: [CachedLane; GOLDILOCKS_DIGEST384_LANES_V1] = core::array::from_fn(|lane| {
            let mut sponge = CachedLane::new(lane);
            for (tag, bytes) in [
                (1, MESSAGE_FRAME_DOMAIN_V1),
                (2, domain.catalog),
                (3, domain.protocol),
                (4, domain.profile),
                (5, domain.role),
                (6, domain.phase),
                (7, level.as_slice()),
            ] {
                sponge.absorb_byte_field(tag, bytes);
            }
            sponge
        });
        let pending = lanes[0].pending;
        let pending_len = lanes[0].pending_len;
        debug_assert!(
            lanes
                .iter()
                .all(|lane| lane.pending == pending && lane.pending_len == pending_len)
        );
        Some(Self {
            lane_states: core::array::from_fn(|lane| lanes[lane].state),
            pending,
            pending_len,
            domain,
        })
    }

    /// Hash exact ordered byte fields at the domain's original index.
    ///
    /// Returns `None` only when the count or a field length exceeds the canonical
    /// framing ceiling, exactly as [`crate::hash_bytes_384_v1`].
    #[must_use]
    pub fn hash(&self, fields: &[&[u8]]) -> Option<GoldilocksDigest384V1> {
        self.hash_at(self.domain.index, fields)
    }

    /// Hash exact ordered byte fields while changing only domain tag 8 (index).
    ///
    /// Catalog, protocol, profile, role, phase, level, counter, and six independent
    /// lane identities retain their original meanings. Zero fields are distinct
    /// from one empty field. The immutable cached state is never advanced.
    #[must_use]
    pub fn hash_at(&self, index: u64, fields: &[&[u8]]) -> Option<GoldilocksDigest384V1> {
        if fields.len() > MAX_FRAMED_FIELD_BYTES_V1
            || fields
                .iter()
                .any(|field| field.len() > MAX_FRAMED_FIELD_BYTES_V1)
        {
            return None;
        }
        let words = self.lanes_at(index).map(|mut lane| {
            lane.absorb(11);
            lane.absorb(u64::try_from(fields.len()).expect("bounded field count fits u64"));
            for (position, field) in fields.iter().enumerate() {
                lane.absorb_byte_field(
                    12 + u64::try_from(position).expect("bounded field position fits u64"),
                    field,
                );
            }
            lane.finish()
        });
        GoldilocksDigest384V1::new(words)
    }

    /// Construct the unchanged canonical final-field stream at the selected index.
    ///
    /// This less frequent API replays the borrowed domain through the canonical
    /// constructor; it does not resume the cached private permutation state.
    /// It retains the exact CPU/GPU-compatible return type, lane snapshots,
    /// chunking, finalization and atomic errors. The hot [`Self::hash_at`] path
    /// uses the cached state directly and does not call this method.
    ///
    /// # Errors
    ///
    /// Returns [`GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded`]
    /// if a field count or byte length exceeds the canonical 32-bit ceiling.
    pub fn last_field_stream_at(
        &self,
        index: u64,
        prefix_fields: &[&[u8]],
        final_field_len: usize,
    ) -> Result<GoldilocksDigest384LastFieldStreamV1, GoldilocksDigest384LastFieldStreamErrorV1>
    {
        framed_field_count(prefix_fields.len(), final_field_len)?;
        GoldilocksDigest384LastFieldStreamV1::new(
            GoldilocksDigestDomainV1 {
                index,
                ..self.domain
            },
            prefix_fields,
            final_field_len,
        )
    }

    fn lanes_at(&self, index: u64) -> [CachedLane; GOLDILOCKS_DIGEST384_LANES_V1] {
        core::array::from_fn(|lane| {
            let mut sponge = CachedLane {
                state: self.lane_states[lane],
                pending: self.pending,
                pending_len: self.pending_len,
                constants: &round_constants()[lane],
            };
            for (tag, bytes) in [
                (8, index.to_le_bytes()),
                (9, self.domain.counter.to_le_bytes()),
                (
                    10,
                    u64::try_from(lane)
                        .expect("six-lane index fits u64")
                        .to_le_bytes(),
                ),
            ] {
                sponge.absorb_byte_field(tag, &bytes);
            }
            sponge
        })
    }
}

/// Owned immutable byte fields and complete numeric suffix of a canonical digest domain.
///
/// Arc-backed fields permit independent prefixes to share a complete public context
/// without replacing it by a digest or making a copy for every role or tree level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldilocksDigest384OwnedDomainV1 {
    /// Exact final catalog identity.
    pub catalog: Arc<[u8]>,
    /// Exact canonical native-STARK protocol identity.
    pub protocol: Arc<[u8]>,
    /// Complete canonical profile descriptor, including any caller-bound context.
    pub profile: Arc<[u8]>,
    /// Tree, oracle or transcript role.
    pub role: Arc<[u8]>,
    /// Protocol phase within the role.
    pub phase: Arc<[u8]>,
    /// Exact tree level, or zero for non-tree roles.
    pub level: u64,
    /// Default index used by the prefix's non-overriding hash method.
    pub index: u64,
    /// Exact transcript or challenge counter.
    pub counter: u64,
}

impl GoldilocksDigest384OwnedDomainV1 {
    /// Borrow the exact canonical domain without allocation or metadata substitution.
    #[must_use]
    pub fn as_borrowed(&self) -> GoldilocksDigestDomainV1<'_> {
        GoldilocksDigestDomainV1 {
            catalog: &self.catalog,
            protocol: &self.protocol,
            profile: &self.profile,
            role: &self.role,
            phase: &self.phase,
            level: self.level,
            index: self.index,
            counter: self.counter,
        }
    }
}

/// Owned reusable snapshot of the one canonical six-lane domain-prefix implementation.
///
/// Construction delegates to [`GoldilocksDigest384DomainPrefixV1::new`]. Every
/// operation resumes through that same prefix implementation; no independent
/// permutation, framing algorithm, lane parameter owner or mutable cache is added.
/// Clones share the immutable snapshot and all complete domain byte fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldilocksDigest384OwnedDomainPrefixV1 {
    inner: Arc<OwnedDomainPrefix>,
}

#[derive(Debug, PartialEq, Eq)]
struct OwnedDomainPrefix {
    lane_states: [[u64; STATE_WIDTH]; GOLDILOCKS_DIGEST384_LANES_V1],
    pending: [u64; RATE],
    pending_len: usize,
    domain: GoldilocksDigest384OwnedDomainV1,
}

impl GoldilocksDigest384OwnedDomainPrefixV1 {
    /// Absorb the complete owned domain through tag 7 using the canonical owner.
    ///
    /// Returns `None` for the same oversized domain fields as the borrowed owner.
    #[must_use]
    pub fn new(domain: GoldilocksDigest384OwnedDomainV1) -> Option<Self> {
        let prefix = GoldilocksDigest384DomainPrefixV1::new(domain.as_borrowed())?;
        let lane_states = prefix.lane_states;
        let pending = prefix.pending;
        let pending_len = prefix.pending_len;
        Some(Self {
            inner: Arc::new(OwnedDomainPrefix {
                lane_states,
                pending,
                pending_len,
                domain,
            }),
        })
    }

    /// Retain access to the complete immutable context and every original domain field.
    #[must_use]
    pub fn domain(&self) -> GoldilocksDigestDomainV1<'_> {
        self.inner.domain.as_borrowed()
    }

    /// Hash exact ordered fields at the original index with canonical framing bounds.
    #[must_use]
    pub fn hash(&self, fields: &[&[u8]]) -> Option<GoldilocksDigest384V1> {
        self.borrowed_prefix().hash(fields)
    }

    /// Override only the full-width index while reusing the complete immutable prefix.
    #[must_use]
    pub fn hash_at(&self, index: u64, fields: &[&[u8]]) -> Option<GoldilocksDigest384V1> {
        self.borrowed_prefix().hash_at(index, fields)
    }

    /// Construct the unchanged canonical stream using the complete owned domain.
    ///
    /// This delegates to the existing stream handoff, including its framing limits
    /// and atomic-error behavior; it does not introduce another stream permutation.
    ///
    /// # Errors
    ///
    /// Returns the same framing-limit error as the borrowed prefix's stream handoff.
    pub fn last_field_stream_at(
        &self,
        index: u64,
        prefix_fields: &[&[u8]],
        final_field_len: usize,
    ) -> Result<GoldilocksDigest384LastFieldStreamV1, GoldilocksDigest384LastFieldStreamErrorV1>
    {
        self.borrowed_prefix()
            .last_field_stream_at(index, prefix_fields, final_field_len)
    }

    fn borrowed_prefix(&self) -> GoldilocksDigest384DomainPrefixV1<'_> {
        GoldilocksDigest384DomainPrefixV1 {
            lane_states: self.inner.lane_states,
            pending: self.inner.pending,
            pending_len: self.inner.pending_len,
            domain: self.domain(),
        }
    }
}

/// Private execution state for the exact public canonical lane parameters.
struct CachedLane {
    state: [u64; STATE_WIDTH],
    pending: [u64; RATE],
    pending_len: usize,
    constants: &'static LaneRoundConstants,
}

impl CachedLane {
    fn new(lane: usize) -> Self {
        Self {
            state: goldilocks_digest384_lane_initial_state_v1(lane)
                .expect("canonical six-lane index"),
            pending: [0; RATE],
            pending_len: 0,
            constants: &round_constants()[lane],
        }
    }

    fn absorb(&mut self, value: u64) {
        debug_assert!(value < FIELD_MODULUS);
        self.pending[self.pending_len] = value;
        self.pending_len += 1;
        if self.pending_len == RATE {
            self.flush();
        }
    }

    fn flush(&mut self) {
        for (state, value) in self.state.iter_mut().zip(self.pending) {
            *state = add(*state, value);
        }
        permute(&mut self.state, self.constants);
        self.pending = [0; RATE];
        self.pending_len = 0;
    }

    fn finish(mut self) -> u64 {
        self.absorb(1);
        if self.pending_len != 0 {
            self.flush();
        }
        self.state[0]
    }

    fn absorb_byte_field(&mut self, tag: u64, bytes: &[u8]) {
        debug_assert!(bytes.len() <= MAX_FRAMED_FIELD_BYTES_V1);
        self.absorb(tag);
        self.absorb(u64::try_from(bytes.len()).expect("bounded field length fits u64"));
        let mut chunks = bytes.chunks_exact(7);
        for chunk in &mut chunks {
            let mut word = [0; 8];
            word[..7].copy_from_slice(chunk);
            self.absorb(u64::from_le_bytes(word));
        }
        let remainder = chunks.remainder();
        let mut terminal = [0; 8];
        terminal[..remainder.len()].copy_from_slice(remainder);
        terminal[remainder.len()] = 1;
        self.absorb(u64::from_le_bytes(terminal));
    }
}

#[inline]
fn add(left: u64, right: u64) -> u64 {
    let sum = left.wrapping_add(right);
    let reduced = if sum < left {
        sum.wrapping_sub(FIELD_MODULUS)
    } else {
        sum
    };
    if reduced >= FIELD_MODULUS {
        reduced - FIELD_MODULUS
    } else {
        reduced
    }
}

#[inline]
fn multiply(left: u64, right: u64) -> u64 {
    let product = u128::from(left) * u128::from(right);
    let low = u64::try_from(product & u128::from(u64::MAX)).expect("masked product word");
    let high = u64::try_from(product >> 64).expect("high product word");
    let high_low = i128::from(high & 0xffff_ffff);
    let high_high = i128::from(high >> 32);
    let mut reduced = i128::from(low) + (high_low << 32) - high_low - high_high;
    let modulus = i128::from(FIELD_MODULUS);
    // Exact Goldilocks reduction: -(2^32-1) <= reduced <= 2p-2, so one
    // correction in each direction suffices, as in the canonical implementation.
    if reduced < 0 {
        reduced += modulus;
    }
    if reduced >= modulus {
        reduced -= modulus;
    }
    u64::try_from(reduced).expect("canonical Goldilocks product")
}

#[inline]
fn pow7(value: u64) -> u64 {
    let square = multiply(value, value);
    let fourth = multiply(square, square);
    multiply(multiply(fourth, square), value)
}

fn permute(state: &mut [u64; STATE_WIDTH], constants: &LaneRoundConstants) {
    for (round, row) in constants.iter().enumerate() {
        for (word, constant) in state.iter_mut().zip(row) {
            *word = add(*word, *constant);
        }
        if (FULL_ROUNDS_HALF_V1..FULL_ROUNDS_HALF_V1 + PARTIAL_ROUNDS_V1).contains(&round) {
            state[0] = pow7(state[0]);
        } else {
            for word in state.iter_mut() {
                *word = pow7(*word);
            }
        }
        let prior = *state;
        for (result, row) in state.iter_mut().zip(MDS) {
            *result = row.iter().zip(prior).fold(0, |sum, (coefficient, value)| {
                add(sum, multiply(*coefficient, value))
            });
        }
    }
}

fn framed_field_count(
    prefix_count: usize,
    final_field_len: usize,
) -> Result<usize, GoldilocksDigest384LastFieldStreamErrorV1> {
    let count = prefix_count
        .checked_add(1)
        .ok_or(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded)?;
    if count > MAX_FRAMED_FIELD_BYTES_V1 || final_field_len > MAX_FRAMED_FIELD_BYTES_V1 {
        return Err(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded);
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{poseidon::FIELD_MODULUS, poseidon_digest384::hash_bytes_384_v1};

    fn domain() -> GoldilocksDigestDomainV1<'static> {
        GoldilocksDigestDomainV1 {
            catalog: crate::FASTPQ_CATALOG_V1.as_bytes(),
            protocol: crate::FASTPQ_FINAL_V1_ID.as_bytes(),
            profile: crate::FASTPQ_FINAL_V1_ID.as_bytes(),
            role: b"air-trace-commitment",
            phase: b"node",
            level: 19,
            index: 7,
            counter: 23,
        }
    }

    fn bytes(length: usize, salt: u64) -> Vec<u8> {
        let mut state = salt;
        (0..length)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 56) as u8
            })
            .collect()
    }

    fn assert_stream_eq(
        actual: &GoldilocksDigest384LastFieldStreamV1,
        expected: &GoldilocksDigest384LastFieldStreamV1,
    ) {
        assert_eq!(actual.expected_len(), expected.expected_len());
        assert_eq!(actual.received_len(), expected.received_len());
        assert_eq!(actual.remaining_len(), expected.remaining_len());
        for lane in 0..GOLDILOCKS_DIGEST384_LANES_V1 {
            assert_eq!(actual.lane_prefix_v1(lane), expected.lane_prefix_v1(lane));
        }
        assert_eq!(actual.lane_prefix_v1(GOLDILOCKS_DIGEST384_LANES_V1), None);
        assert_eq!(actual.lane_prefix_v1(usize::MAX), None);
    }

    fn owned_domain(domain: GoldilocksDigestDomainV1<'_>) -> GoldilocksDigest384OwnedDomainV1 {
        GoldilocksDigest384OwnedDomainV1 {
            catalog: Arc::from(domain.catalog),
            protocol: Arc::from(domain.protocol),
            profile: Arc::from(domain.profile),
            role: Arc::from(domain.role),
            phase: Arc::from(domain.phase),
            level: domain.level,
            index: domain.index,
            counter: domain.counter,
        }
    }

    #[test]
    fn owned_prefix_matches_independent_oracle_at_both_rate_positions_and_full_integers() {
        fn assert_worker_safe<T: Clone + Send + Sync>() {}
        assert_worker_safe::<GoldilocksDigest384OwnedDomainPrefixV1>();
        let mut positions = [false; RATE];
        for length in 0..=15 {
            let profile = bytes(length, 813);
            // Vary all three integer fields across field and machine boundaries.
            for (level, counter) in [
                (0, u64::MAX),
                (FIELD_MODULUS - 1, FIELD_MODULUS),
                (FIELD_MODULUS, FIELD_MODULUS - 1),
                (u64::MAX, 0),
            ] {
                let selected = GoldilocksDigestDomainV1 {
                    profile: &profile,
                    level,
                    counter,
                    ..domain()
                };
                let borrowed = GoldilocksDigest384DomainPrefixV1::new(selected).unwrap();
                let cache =
                    GoldilocksDigest384OwnedDomainPrefixV1::new(owned_domain(selected)).unwrap();
                positions[cache.inner.pending_len] = true;
                assert_eq!(cache.domain(), selected);
                assert_eq!(cache.borrowed_prefix(), borrowed);
                let original = cache.clone();
                for index in [0, 1, FIELD_MODULUS - 1, FIELD_MODULUS, u64::MAX] {
                    let indexed = GoldilocksDigestDomainV1 { index, ..selected };
                    for fields in [
                        Vec::<&[u8]>::new(),
                        vec![b"".as_slice()],
                        vec![b"".as_slice(), b"".as_slice()],
                        vec![
                            b"1234567".as_slice(),
                            b"".as_slice(),
                            b"abcdefgh".as_slice(),
                        ],
                    ] {
                        let expected = hash_bytes_384_v1(indexed, &fields);
                        assert_eq!(cache.hash_at(index, &fields), expected);
                        assert_eq!(borrowed.hash_at(index, &fields), expected);
                    }
                }
                assert_eq!(
                    cache.hash(&[b"default index"]),
                    hash_bytes_384_v1(selected, &[b"default index"])
                );
                assert_ne!(cache.hash(&[]), cache.hash(&[b""]));
                assert_eq!(cache, original);
                assert!(Arc::ptr_eq(&cache.inner, &original.inner));
            }
        }
        assert!(positions.into_iter().all(|seen| seen));
    }

    #[test]
    fn owned_prefix_streams_match_independent_snapshots_chunking_and_zero_fields() {
        let mut positions = [false; RATE];
        for length in 0..=15 {
            let profile = bytes(length, 59);
            let selected = GoldilocksDigestDomainV1 {
                profile: &profile,
                index: u64::MAX,
                ..domain()
            };
            let cache =
                GoldilocksDigest384OwnedDomainPrefixV1::new(owned_domain(selected)).unwrap();
            positions[cache.inner.pending_len] = true;
            for prefixes in [
                Vec::<&[u8]>::new(),
                vec![b"".as_slice()],
                vec![b"1234567".as_slice(), b"abcdefgh".as_slice()],
            ] {
                for final_len in [0, 1, 6, 7, 8, 14, 15, 48] {
                    let payload = bytes(final_len, 201);
                    let fields: Vec<&[u8]> = prefixes
                        .iter()
                        .copied()
                        .chain([payload.as_slice()])
                        .collect();
                    let expected = hash_bytes_384_v1(selected, &fields).unwrap();
                    for chunk_size in [1, 7, 8, 13] {
                        let mut stream = cache
                            .last_field_stream_at(selected.index, &prefixes, final_len)
                            .unwrap();
                        let mut independent = GoldilocksDigest384LastFieldStreamV1::new(
                            selected, &prefixes, final_len,
                        )
                        .unwrap();
                        assert_stream_eq(&stream, &independent);
                        for chunk in payload.chunks(chunk_size) {
                            stream.update(chunk).unwrap();
                            independent.update(chunk).unwrap();
                            assert_stream_eq(&stream, &independent);
                        }
                        assert_eq!(stream.finalize().unwrap(), expected);
                        assert_eq!(independent.finalize().unwrap(), expected);
                        assert_eq!(cache.hash(&fields), Some(expected));
                    }
                }
            }
        }
        assert!(positions.into_iter().all(|seen| seen));
    }

    #[test]
    fn owned_prefix_retains_long_context_once_and_survives_concurrent_reuse() {
        let profile: Arc<[u8]> = Arc::from(bytes(256 * 1024, 103));
        let mut descriptor = owned_domain(domain());
        descriptor.profile = profile.clone();
        let cache = GoldilocksDigest384OwnedDomainPrefixV1::new(descriptor).unwrap();
        assert!(Arc::ptr_eq(&profile, &cache.inner.domain.profile));
        assert_eq!(Arc::strong_count(&profile), 2);
        // Two one-shot long-context oracles suffice; each worker reuses both.
        let cases = [0, u64::MAX].map(|index| {
            (
                index,
                hash_bytes_384_v1(
                    GoldilocksDigestDomainV1 {
                        index,
                        ..cache.domain()
                    },
                    &[b"", b"complete body"],
                )
                .unwrap(),
            )
        });
        let saved = cache.clone();
        drop(profile);
        assert_eq!(Arc::strong_count(&cache.inner.domain.profile), 1);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let worker = cache.clone();
                let cases = &cases;
                scope.spawn(move || {
                    for _ in 0..4 {
                        for &(index, expected) in cases {
                            assert_eq!(
                                worker.hash_at(index, &[b"", b"complete body"]),
                                Some(expected)
                            );
                        }
                    }
                });
            }
        });
        assert!(Arc::ptr_eq(&cache.inner, &saved.inner));
        assert_eq!(cache, saved);
        let mut changed = cache.inner.domain.clone();
        let mut changed_bytes = changed.profile.to_vec();
        *changed_bytes.last_mut().unwrap() ^= 1;
        changed.profile = Arc::from(changed_bytes);
        let different = GoldilocksDigest384OwnedDomainPrefixV1::new(changed).unwrap();
        assert_ne!(
            cache.hash(&[b"complete body"]),
            different.hash(&[b"complete body"])
        );
    }

    #[test]
    fn owned_prefix_stream_errors_preserve_snapshot_and_preallocation_bounds() {
        let cache = GoldilocksDigest384OwnedDomainPrefixV1::new(owned_domain(domain())).unwrap();
        let mut stream = cache.last_field_stream_at(u64::MAX, &[b""], 8).unwrap();
        let pristine = stream;
        assert_eq!(
            stream.update(&[0; 9]),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun {
                expected: 8,
                received: 0,
                additional: 9
            })
        );
        assert_stream_eq(&stream, &pristine);
        stream.update(b"123456").unwrap();
        let partial = stream;
        assert_eq!(
            stream.update(b"789"),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun {
                expected: 8,
                received: 6,
                additional: 3
            })
        );
        assert_stream_eq(&stream, &partial);
        assert_eq!(
            stream.finalize(),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputUnderrun {
                expected: 8,
                received: 6
            })
        );
        stream.update(b"78").unwrap();
        assert_eq!(
            Some(stream.finalize().unwrap()),
            cache.hash_at(u64::MAX, &[b"", b"12345678"])
        );
        if let Some(too_long) = MAX_FRAMED_FIELD_BYTES_V1.checked_add(1) {
            assert_eq!(
                cache.last_field_stream_at(0, &[], too_long).unwrap_err(),
                GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded
            );
        }
        let boundary = cache
            .last_field_stream_at(0, &[], MAX_FRAMED_FIELD_BYTES_V1)
            .unwrap();
        assert_eq!(boundary.remaining_len(), MAX_FRAMED_FIELD_BYTES_V1);
        assert_eq!(
            cache.hash(&[b"after errors"]),
            hash_bytes_384_v1(domain(), &[b"after errors"])
        );
    }

    #[test]
    fn isolated_permutation_uses_public_parameters_and_exact_field_arithmetic() {
        let modulus = u128::from(FIELD_MODULUS);
        let values = [
            0,
            1,
            2,
            u64::from(u32::MAX),
            1 << 32,
            FIELD_MODULUS - 2,
            FIELD_MODULUS - 1,
        ];
        for left in values {
            for right in values {
                assert_eq!(
                    u128::from(add(left, right)),
                    (u128::from(left) + u128::from(right)) % modulus
                );
                assert_eq!(
                    u128::from(multiply(left, right)),
                    (u128::from(left) * u128::from(right)) % modulus
                );
            }
            let expected = (0..7).fold(1_u128, |product, _| product * u128::from(left) % modulus);
            assert_eq!(u128::from(pow7(left)), expected);
        }
        assert_eq!(
            GOLDILOCKS_DIGEST384_ROUNDS_V1,
            2 * FULL_ROUNDS_HALF_V1 + PARTIAL_ROUNDS_V1
        );
        for (lane, constants) in round_constants().iter().enumerate() {
            for (round, row) in constants.iter().enumerate() {
                assert_eq!(
                    Some(*row),
                    goldilocks_digest384_lane_round_constants_v1(lane, round)
                );
            }
            let initial = goldilocks_digest384_lane_initial_state_v1(lane).unwrap();
            assert_eq!(CachedLane::new(lane).state, initial);
            for mut actual in [initial, [0; STATE_WIDTH], [FIELD_MODULUS - 1; STATE_WIDTH]] {
                let mut expected = actual.map(u128::from);
                for (round, constants) in constants.iter().enumerate() {
                    for (word, constant) in expected.iter_mut().zip(constants) {
                        *word = (*word + u128::from(*constant)) % modulus;
                    }
                    for (index, word) in expected.iter_mut().enumerate() {
                        if index == 0 || !(4..61).contains(&round) {
                            *word = (0..7).fold(1, |product, _| product * *word % modulus);
                        }
                    }
                    let prior = expected;
                    for (word, coefficients) in expected.iter_mut().zip(MDS) {
                        *word =
                            coefficients
                                .iter()
                                .zip(prior)
                                .fold(0, |sum, (coefficient, value)| {
                                    (sum + u128::from(*coefficient) * value) % modulus
                                });
                    }
                }
                permute(&mut actual, constants);
                assert_eq!(actual.map(u128::from), expected, "lane {lane}");
            }
        }
    }

    #[test]
    fn cached_hash_matches_all_independent_python_reference_vectors() {
        fn decode_hex(encoded: &str) -> Vec<u8> {
            assert!(encoded.len().is_multiple_of(2));
            encoded
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        let mut count = 0;
        for line in include_str!("assets/digest384_reference_v1.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let columns: Vec<_> = line.split('\t').collect();
            assert_eq!(columns.len(), 11);
            let domain_bytes: Vec<_> = columns[1..=5]
                .iter()
                .map(|bytes| decode_hex(bytes))
                .collect();
            let selected = GoldilocksDigestDomainV1 {
                catalog: &domain_bytes[0],
                protocol: &domain_bytes[1],
                profile: &domain_bytes[2],
                role: &domain_bytes[3],
                phase: &domain_bytes[4],
                level: columns[6].parse().unwrap(),
                index: columns[7].parse().unwrap(),
                counter: columns[8].parse().unwrap(),
            };
            let fields = if columns[9] == "-" {
                Vec::new()
            } else {
                columns[9].split(',').map(decode_hex).collect::<Vec<_>>()
            };
            let borrowed: Vec<_> = fields.iter().map(Vec::as_slice).collect();
            let expected =
                GoldilocksDigest384V1::from_le_bytes(decode_hex(columns[10]).try_into().unwrap())
                    .unwrap();
            let cache = GoldilocksDigest384DomainPrefixV1::new(selected).unwrap();
            assert_eq!(
                cache.hash(&borrowed),
                Some(expected),
                "fixture {}",
                columns[0]
            );
            assert_eq!(
                cache.hash(&borrowed),
                hash_bytes_384_v1(selected, &borrowed)
            );
            if let Some((last, prefix)) = borrowed.split_last() {
                let mut stream = cache
                    .last_field_stream_at(selected.index, prefix, last.len())
                    .unwrap();
                for chunk in last.chunks(7) {
                    stream.update(chunk).unwrap();
                }
                assert_eq!(
                    stream.finalize().unwrap(),
                    expected,
                    "stream fixture {}",
                    columns[0]
                );
            }
            count += 1;
        }
        assert_eq!(count, 31);
    }

    #[test]
    fn cached_hash_matches_independent_oracle_for_every_domain_coordinate() {
        let original = domain();
        let fields: [&[u8]; 3] = [b"first", b"", b"payload\0"];
        let baseline = hash_bytes_384_v1(original, &fields).unwrap();
        for coordinate in 0..8 {
            let mut changed = original;
            match coordinate {
                0 => changed.catalog = b"changed catalog",
                1 => changed.protocol = b"changed protocol",
                2 => changed.profile = b"changed profile",
                3 => changed.role = b"changed role",
                4 => changed.phase = b"changed phase",
                5 => changed.level = u64::MAX,
                6 => changed.index = u64::MAX,
                7 => changed.counter = u64::MAX,
                _ => unreachable!(),
            }
            let prefix = GoldilocksDigest384DomainPrefixV1::new(changed).unwrap();
            let actual = prefix.hash(&fields).unwrap();
            assert_eq!(Some(actual), hash_bytes_384_v1(changed, &fields));
            assert_ne!(actual, baseline, "domain coordinate {coordinate}");
            let old_stream =
                GoldilocksDigest384LastFieldStreamV1::new(changed, &fields[..2], fields[2].len())
                    .unwrap();
            let stream = prefix
                .last_field_stream_at(changed.index, &fields[..2], fields[2].len())
                .unwrap();
            assert_stream_eq(&stream, &old_stream);
        }
        let empty = GoldilocksDigestDomainV1 {
            catalog: b"",
            protocol: b"",
            profile: b"",
            role: b"",
            phase: b"",
            level: 0,
            index: 0,
            counter: 0,
        };
        let prefix = GoldilocksDigest384DomainPrefixV1::new(empty).unwrap();
        assert_eq!(prefix.hash(&fields), hash_bytes_384_v1(empty, &fields));
        assert_eq!(prefix.hash(&[]), hash_bytes_384_v1(empty, &[]));
    }

    #[test]
    fn index_override_preserves_original_counter_and_all_full_width_integers() {
        for counter in [0, FIELD_MODULUS, u64::MAX] {
            let original = GoldilocksDigestDomainV1 {
                level: u64::MAX,
                counter,
                ..domain()
            };
            let prefix = GoldilocksDigest384DomainPrefixV1::new(original).unwrap();
            let unchanged = prefix;
            for index in [
                0,
                1,
                FIELD_MODULUS - 1,
                FIELD_MODULUS,
                FIELD_MODULUS + 1,
                u64::MAX,
            ] {
                let selected = GoldilocksDigestDomainV1 { index, ..original };
                for fields in [
                    Vec::<&[u8]>::new(),
                    vec![b"".as_slice()],
                    vec![b"index payload".as_slice()],
                ] {
                    assert_eq!(
                        prefix.hash_at(index, &fields),
                        hash_bytes_384_v1(selected, &fields)
                    );
                }
                let stream = prefix.last_field_stream_at(index, &[b"prefix"], 8).unwrap();
                let old =
                    GoldilocksDigest384LastFieldStreamV1::new(selected, &[b"prefix"], 8).unwrap();
                assert_stream_eq(&stream, &old);
            }
            assert_eq!(
                prefix.hash(&[b"original"]),
                hash_bytes_384_v1(original, &[b"original"])
            );
            assert_eq!(prefix, unchanged);
        }
    }

    #[test]
    fn empty_fields_and_seven_byte_boundaries_remain_distinct() {
        let prefix = GoldilocksDigest384DomainPrefixV1::new(domain()).unwrap();
        let fields: [&[&[u8]]; 5] = [&[], &[b""], &[b"", b""], &[b"\0"], &[b"\0", b""]];
        let digests: Vec<_> = fields
            .iter()
            .map(|fields| {
                let actual = prefix.hash(fields).unwrap();
                assert_eq!(Some(actual), hash_bytes_384_v1(domain(), fields));
                actual
            })
            .collect();
        for (index, digest) in digests.iter().enumerate() {
            assert!(!digests[..index].contains(digest));
        }
        for length in [0, 1, 6, 7, 8, 13, 14, 15, 47, 48, 49, 95, 96, 97] {
            let payload = bytes(length, 42);
            let with_zero: Vec<u8> = payload.iter().copied().chain([0]).collect();
            let actual = prefix.hash(&[&payload]).unwrap();
            assert_eq!(Some(actual), hash_bytes_384_v1(domain(), &[&payload]));
            assert_ne!(Some(actual), prefix.hash(&[&with_zero]));
            for split in [0, length / 2, length] {
                let fields = [&payload[..split], &payload[split..]];
                assert_eq!(prefix.hash(&fields), hash_bytes_384_v1(domain(), &fields));
                assert_ne!(Some(actual), prefix.hash(&fields));
            }
        }
    }

    #[test]
    fn domain_chunk_alignment_and_stream_handoffs_match_in_all_six_lanes() {
        for length in 0..=15 {
            let role = bytes(length, 91);
            let selected = GoldilocksDigestDomainV1 {
                role: &role,
                ..domain()
            };
            let cache = GoldilocksDigest384DomainPrefixV1::new(selected).unwrap();
            for prefix_fields in [
                Vec::<&[u8]>::new(),
                vec![b"".as_slice()],
                vec![b"1234567".as_slice(), b"abcdefgh".as_slice()],
            ] {
                let mut stream = cache
                    .last_field_stream_at(selected.index, &prefix_fields, 48)
                    .unwrap();
                let mut old =
                    GoldilocksDigest384LastFieldStreamV1::new(selected, &prefix_fields, 48)
                        .unwrap();
                assert_stream_eq(&stream, &old);
                let final_field = bytes(48, length as u64);
                for chunk in final_field.chunks(7) {
                    stream.update(chunk).unwrap();
                    old.update(chunk).unwrap();
                    assert_stream_eq(&stream, &old);
                }
                assert_eq!(stream.finalize(), old.finalize());
            }
        }
    }

    #[test]
    fn actual_merkle_children_and_chunked_final_fields_match_one_shot() {
        let left = GoldilocksDigest384V1::new([0, 1, FIELD_MODULUS - 1, 42, 99, 2])
            .unwrap()
            .to_le_bytes();
        let right = GoldilocksDigest384V1::new([FIELD_MODULUS - 1, 2, 3, 4, 5, 6])
            .unwrap()
            .to_le_bytes();
        let cache = GoldilocksDigest384DomainPrefixV1::new(domain()).unwrap();
        for index in [0, 1, 8191, u64::MAX] {
            let selected = GoldilocksDigestDomainV1 { index, ..domain() };
            let expected = hash_bytes_384_v1(selected, &[&left, &right]).unwrap();
            assert_eq!(cache.hash_at(index, &[&left, &right]), Some(expected));
            for chunk_size in [1, 6, 7, 8, 48, 49] {
                let mut stream = cache
                    .last_field_stream_at(index, &[&left], right.len())
                    .unwrap();
                for chunk in right.chunks(chunk_size) {
                    stream.update(chunk).unwrap();
                }
                assert_eq!(stream.finalize().unwrap(), expected);
            }
            assert_ne!(cache.hash_at(index, &[&right, &left]), Some(expected));
        }
        for length in [0, 1, 6, 7, 8, 13, 14, 15, 96, 272] {
            let final_field = bytes(length, 777);
            let expected = hash_bytes_384_v1(domain(), &[&left, &final_field]).unwrap();
            for chunk_size in [1, 7, 13, 64] {
                let mut stream = cache
                    .last_field_stream_at(domain().index, &[&left], length)
                    .unwrap();
                for chunk in final_field.chunks(chunk_size) {
                    stream.update(chunk).unwrap();
                }
                assert_eq!(stream.finalize().unwrap(), expected);
            }
        }
    }

    #[test]
    fn stream_errors_are_atomic_and_cache_borrows_immutable_shareable_domain() {
        fn assert_worker_safe<T: Copy + Send + Sync>() {}
        assert_worker_safe::<GoldilocksDigest384DomainPrefixV1<'_>>();
        let role = bytes(15, 123);
        let cache = GoldilocksDigest384DomainPrefixV1::new(GoldilocksDigestDomainV1 {
            role: &role,
            ..domain()
        })
        .unwrap();
        // The immutable cache retains borrowed field pointers, not owned copies.
        assert_eq!(cache.domain.role.as_ptr(), role.as_ptr());
        assert_eq!(cache.domain.catalog.as_ptr(), domain().catalog.as_ptr());
        assert_eq!(cache.domain.protocol.as_ptr(), domain().protocol.as_ptr());
        assert_eq!(cache.domain.profile.as_ptr(), domain().profile.as_ptr());
        assert_eq!(cache.domain.phase.as_ptr(), domain().phase.as_ptr());
        let original = cache;
        let mut stream = cache.last_field_stream_at(99, &[b"prefix"], 8).unwrap();
        let pristine = stream;
        assert_eq!(
            stream.update(&[0; 9]),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun {
                expected: 8,
                received: 0,
                additional: 9,
            })
        );
        assert_stream_eq(&stream, &pristine);
        stream.update(b"123456").unwrap();
        let partial = stream;
        assert_eq!(
            stream.update(b"789"),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputOverrun {
                expected: 8,
                received: 6,
                additional: 3,
            })
        );
        assert_stream_eq(&stream, &partial);
        assert_eq!(
            stream.finalize(),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::InputUnderrun {
                expected: 8,
                received: 6,
            })
        );
        assert_stream_eq(&stream, &partial);
        stream.update(b"78").unwrap();
        let mut recovered = partial;
        recovered.update(b"78").unwrap();
        // Public snapshots do not expose a partial seven-byte chunk; matching
        // the recovered final digest also checks that hidden buffered content.
        assert_eq!(stream.finalize(), recovered.finalize());
        assert_eq!(
            Some(stream.finalize().unwrap()),
            cache.hash_at(99, &[b"prefix", b"12345678"])
        );
        let mut again = cache.last_field_stream_at(99, &[b"prefix"], 8).unwrap();
        assert_stream_eq(&again, &pristine);
        again.update(b"abcdefgh").unwrap();
        assert_eq!(
            Some(again.finalize().unwrap()),
            cache.hash_at(99, &[b"prefix", b"abcdefgh"])
        );
        assert_eq!(cache, original);
    }

    #[test]
    fn framing_limits_are_checked_without_allocating_the_declared_payload() {
        assert_eq!(framed_field_count(0, 0).unwrap(), 1);
        assert_eq!(
            framed_field_count(MAX_FRAMED_FIELD_BYTES_V1 - 1, MAX_FRAMED_FIELD_BYTES_V1).unwrap(),
            MAX_FRAMED_FIELD_BYTES_V1
        );
        assert_eq!(
            framed_field_count(MAX_FRAMED_FIELD_BYTES_V1, 0),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded)
        );
        assert_eq!(
            framed_field_count(usize::MAX, 0),
            Err(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded)
        );
        let cache = GoldilocksDigest384DomainPrefixV1::new(domain()).unwrap();
        let stream = cache
            .last_field_stream_at(u64::MAX, &[], MAX_FRAMED_FIELD_BYTES_V1)
            .unwrap();
        let oracle = GoldilocksDigest384LastFieldStreamV1::new(
            GoldilocksDigestDomainV1 {
                index: u64::MAX,
                ..domain()
            },
            &[],
            MAX_FRAMED_FIELD_BYTES_V1,
        )
        .unwrap();
        assert_stream_eq(&stream, &oracle);
        assert_eq!(stream.remaining_len(), MAX_FRAMED_FIELD_BYTES_V1);
        if let Some(too_long) = MAX_FRAMED_FIELD_BYTES_V1.checked_add(1) {
            assert_eq!(
                cache.last_field_stream_at(0, &[], too_long).unwrap_err(),
                GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded
            );
            assert_eq!(
                framed_field_count(0, too_long),
                Err(GoldilocksDigest384LastFieldStreamErrorV1::FramingLimitExceeded)
            );
        }
        assert!(core::mem::size_of_val(&cache) <= 512);
        assert!(core::mem::size_of_val(&stream) <= 256);
    }

    #[test]
    #[ignore = "native scalar timing diagnostic; not a production performance qualification"]
    fn cached_merkle_pair_timing_diagnostic() {
        let left = GoldilocksDigest384V1::new([0, 1, 2, 3, 4, 5])
            .unwrap()
            .to_le_bytes();
        let right = GoldilocksDigest384V1::new([6, 7, 8, 9, 10, 11])
            .unwrap()
            .to_le_bytes();
        let cache = GoldilocksDigest384DomainPrefixV1::new(domain()).unwrap();
        let started = std::time::Instant::now();
        let expected: Vec<_> = (0..1000)
            .map(|index| {
                hash_bytes_384_v1(
                    GoldilocksDigestDomainV1 { index, ..domain() },
                    &[&left, &right],
                )
                .unwrap()
            })
            .collect();
        let one_shot = started.elapsed();
        let started = std::time::Instant::now();
        for (index, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                cache.hash_at(index as u64, &[&left, &right]).unwrap(),
                expected
            );
        }
        eprintln!(
            "digest384_merkle_pairs=1000 one_shot={one_shot:?} prefix_cache={:?}",
            started.elapsed()
        );
    }
}
