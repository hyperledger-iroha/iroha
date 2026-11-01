//! Bounded test-only four-row SHA3-384 experiment using the canonical stream owner.

#![allow(unsafe_code)]

use super::*;
use core::arch::x86_64::*;
use sha3::digest::common::hazmat::{SerializableState, SerializedState};
use std::cell::RefCell;

const RATE: usize = 104;
const MAX_UPDATE: usize = 64;
const NAMED_WORKER_SCRATCH_LIMIT: usize = 4096;
const RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];
const RHO: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];
const PI: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

thread_local! {
    static CLEARS: RefCell<Option<Vec<(&'static str, bool, bool)>>> = const { RefCell::new(None) };
}

/// All serialized crypto state is ephemeral, held in this clearing stack owner.
/// `lanes[word][row]` is a test-kernel layout, never a RustCrypto private cast.
struct Scratch {
    serialized: [[u8; 201]; 4],
    lanes: [[u64; 4]; 25],
    temporary: [[u64; 4]; 5],
}
impl Scratch {
    fn new() -> Self {
        Self {
            serialized: [[0; 201]; 4],
            lanes: [[0; 4]; 25],
            temporary: [[0; 4]; 5],
        }
    }
    fn is_clear(&self) -> bool {
        self.serialized.iter().flatten().all(|&x| x == 0)
            && self.lanes.iter().flatten().all(|&x| x == 0)
            && self.temporary.iter().flatten().all(|&x| x == 0)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let observed = CLEARS.with(|slot| slot.borrow().is_some());
        let was_clear = observed && self.is_clear();
        self.serialized.zeroize();
        self.lanes.zeroize();
        self.temporary.zeroize();
        if observed {
            CLEARS.with(|slot| {
                slot.borrow_mut()
                    .as_mut()
                    .unwrap()
                    .push(("scratch", was_clear, self.is_clear()))
            });
        }
    }
}

/// Staged typed states retain their original slots through installation.
struct Replacements([Sha3_384; 4]);
impl Replacements {
    fn is_clear(&self) -> bool {
        self.0.iter().all(|state| {
            let serialized = zeroize::Zeroizing::new(state.serialize());
            serialized.iter().all(|&byte| byte == 0)
        })
    }
}
impl Drop for Replacements {
    fn drop(&mut self) {
        let observed = CLEARS.with(|slot| slot.borrow().is_some());
        let was_clear = observed && self.is_clear();
        for state in &mut self.0 {
            *state = Sha3_384::default();
        }
        if observed {
            CLEARS.with(|slot| {
                slot.borrow_mut().as_mut().unwrap().push((
                    "replacements",
                    was_clear,
                    self.is_clear(),
                ))
            });
        }
    }
}

#[target_feature(enable = "avx2")]
#[inline]
fn load(words: &[u64; 4]) -> __m256i {
    // SAFETY: the array provides exactly one unaligned 256-bit load.
    unsafe { _mm256_loadu_si256(words.as_ptr().cast()) }
}
#[target_feature(enable = "avx2")]
#[inline]
fn store(words: &mut [u64; 4], value: __m256i) {
    // SAFETY: the exclusive array provides exactly one unaligned 256-bit store.
    unsafe { _mm256_storeu_si256(words.as_mut_ptr().cast(), value) }
}
#[target_feature(enable = "avx2")]
#[inline]
fn rotate(value: __m256i, bits: u32) -> __m256i {
    _mm256_or_si256(
        _mm256_sllv_epi64(value, _mm256_set1_epi64x(i64::from(bits))),
        _mm256_srlv_epi64(value, _mm256_set1_epi64x(i64::from(64 - bits))),
    )
}

/// Four independent FIPS-202 Keccak-f1600 permutations, with all 24 rounds.
#[target_feature(enable = "avx2")]
unsafe fn permute_four(scratch: &mut Scratch) {
    // SAFETY: this wrapper has the same AVX2 admission as the shared kernel.
    unsafe { permute_lanes(&mut scratch.lanes, &mut scratch.temporary) };
}

/// Shared test kernel; retained and converted owners use identical rounds.
#[target_feature(enable = "avx2")]
unsafe fn permute_lanes(lanes: &mut [[u64; 4]; 25], temporary: &mut [[u64; 4]; 5]) {
    for constant in RC {
        for x in 0..5 {
            let mut parity = _mm256_setzero_si256();
            for y in 0..5 {
                parity = _mm256_xor_si256(parity, load(&lanes[5 * y + x]));
            }
            store(&mut temporary[x], parity);
        }
        for x in 0..5 {
            let delta = _mm256_xor_si256(
                load(&temporary[(x + 4) % 5]),
                rotate(load(&temporary[(x + 1) % 5]), 1),
            );
            for y in 0..5 {
                let value = _mm256_xor_si256(load(&lanes[5 * y + x]), delta);
                store(&mut lanes[5 * y + x], value);
            }
        }
        let mut previous = load(&lanes[1]);
        for (target, bits) in PI.into_iter().zip(RHO) {
            let next = load(&lanes[target]);
            store(&mut lanes[target], rotate(previous, bits));
            previous = next;
        }
        for y in 0..5 {
            for x in 0..5 {
                temporary[x] = lanes[y * 5 + x];
            }
            for x in 0..5 {
                let value = _mm256_xor_si256(
                    load(&temporary[x]),
                    _mm256_andnot_si256(
                        load(&temporary[(x + 1) % 5]),
                        load(&temporary[(x + 2) % 5]),
                    ),
                );
                store(&mut lanes[y * 5 + x], value);
            }
        }
        let value = _mm256_xor_si256(load(&lanes[0]), _mm256_set1_epi64x(constant as i64));
        store(&mut lanes[0], value);
    }
}

fn scalar_update(
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    bytes: &[&[u8]],
) -> Result<usize, PrivacyOuterLastFieldStreamErrorV1> {
    for (stream, bytes) in streams.iter_mut().zip(bytes) {
        stream.update(bytes)?;
    }
    Ok(0)
}

fn update_group(
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    bytes: &[&[u8]],
    vector: bool,
) -> Result<usize, PrivacyOuterLastFieldStreamErrorV1> {
    update_group_with_checkpoint(streams, bytes, vector, || Ok(()))
}

/// Return the actual number of completed four-state permutations. All failure
/// checks precede original mutation; the checkpoint injects only test failures.
fn update_group_with_checkpoint(
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    bytes: &[&[u8]],
    vector: bool,
    checkpoint: impl FnOnce() -> Result<(), PrivacyOuterLastFieldStreamErrorV1>,
) -> Result<usize, PrivacyOuterLastFieldStreamErrorV1> {
    use PrivacyOuterLastFieldStreamErrorV1 as Error;
    if streams.is_empty() || streams.len() > 4 || streams.len() != bytes.len() {
        return Err(Error::FramingLimitExceeded);
    }
    for (stream, bytes) in streams.iter().zip(bytes) {
        if stream.remaining == usize::MAX {
            return Err(Error::FramingLimitExceeded);
        }
        if bytes.len() > stream.remaining {
            return Err(Error::InputOverrun {
                remaining: stream.remaining,
                supplied: bytes.len(),
            });
        }
    }
    if !vector
        || !std::is_x86_feature_detected!("avx2")
        || streams.len() != 4
        || bytes[0].is_empty()
        || bytes[0].len() > MAX_UPDATE
        || bytes.iter().any(|input| input.len() != bytes[0].len())
        || streams
            .iter()
            .any(|stream| stream.remaining != streams[0].remaining)
    {
        return scalar_update(streams, bytes);
    }
    let mut scratch = Scratch::new();
    for (target, stream) in scratch.serialized.iter_mut().zip(streams.iter()) {
        let serialized = stream.state.state_snapshot_v1();
        target.copy_from_slice(&serialized);
    }
    let mut cursor = usize::from(scratch.serialized[0][200]);
    assert!(
        cursor < RATE,
        "supported Sha3 serialization must have a valid cursor"
    );
    if scratch
        .serialized
        .iter()
        .any(|state| usize::from(state[200]) != cursor)
    {
        return scalar_update(streams, bytes);
    }
    let mut consumed = 0;
    let mut permutations = 0;
    while consumed < bytes[0].len() {
        let count = (RATE - cursor).min(bytes[0].len() - consumed);
        for (state, input) in scratch.serialized.iter_mut().zip(bytes) {
            for (target, value) in state[cursor..cursor + count]
                .iter_mut()
                .zip(&input[consumed..consumed + count])
            {
                *target ^= value;
            }
        }
        cursor += count;
        consumed += count;
        if cursor == RATE {
            for word in 0..25 {
                for row in 0..4 {
                    scratch.lanes[word][row] = u64::from_le_bytes(
                        scratch.serialized[row][word * 8..word * 8 + 8]
                            .try_into()
                            .unwrap(),
                    );
                }
            }
            // SAFETY: runtime admission above precedes the AVX2-only function;
            // all vector accesses use complete four-word arrays in this owner.
            unsafe { permute_four(&mut scratch) };
            permutations += 1;
            for word in 0..25 {
                for row in 0..4 {
                    scratch.serialized[row][word * 8..word * 8 + 8]
                        .copy_from_slice(&scratch.lanes[word][row].to_le_bytes());
                }
            }
            cursor = 0;
        }
    }
    for state in &mut scratch.serialized {
        state[200] = u8::try_from(cursor).unwrap();
    }
    // Deserialize every replacement before committing any original state. A
    // failed conversion or injected unwind drops all temporary zeroizing owners.
    let replacements = Replacements(core::array::from_fn(|row| {
        let mut serialized = zeroize::Zeroizing::new(SerializedState::<Sha3_384>::default());
        serialized.copy_from_slice(&scratch.serialized[row]);
        Sha3_384::deserialize(&serialized).expect("supported state and checked cursor round trip")
    }));
    checkpoint()?;
    for (stream, state) in streams.iter_mut().zip(replacements.0.iter()) {
        // Assignment drops and wipes the original Sha3 state in its original slot.
        stream.state.inner = state.clone();
        stream.remaining -= bytes[0].len();
    }
    Ok(permutations)
}

fn streams(count: usize, declared: usize) -> Vec<PrivacyOuterLastFieldStreamV1> {
    let catalog =
        crate::privacy_engines::transparent_stark::TransparentStarkDigestContextV1::catalog_v1();
    let context =
        crate::privacy_engines::transparent_stark::TransparentStarkDigestContextV1::x509_scoped_v1(
            [0x37; 32],
            PrivacyOuterProofFamilyV1::Main,
        );
    let domain = context
        .domain_v1(
            &catalog,
            b"iroha:privacy:zk-x509:stark:base-leaf:v1",
            b"vector-row-leaf",
            0,
            0,
            0,
        )
        .unwrap();
    let prefix = PrivacyOuterDomainPrefixV1::new(domain).unwrap();
    (0..count)
        .map(|row| {
            prefix
                .last_field_stream_at_with_counter(
                    row as u64,
                    u64::from(u16::MAX),
                    &[&u16::MAX.to_be_bytes(), &3213_u16.to_be_bytes()],
                    declared,
                )
                .unwrap()
        })
        .collect()
}

fn cloned(streams: &[PrivacyOuterLastFieldStreamV1]) -> Vec<PrivacyOuterLastFieldStreamV1> {
    streams
        .iter()
        .map(|stream| PrivacyOuterLastFieldStreamV1 {
            state: stream.state.clone(),
            remaining: stream.remaining,
        })
        .collect()
}

fn assert_same(left: &[PrivacyOuterLastFieldStreamV1], right: &[PrivacyOuterLastFieldStreamV1]) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.remaining, right.remaining);
        assert_eq!(
            &*left.state.state_snapshot_v1(),
            &*right.state.state_snapshot_v1()
        );
    }
}

#[test]
#[ignore = "mandatory native four-row SHA3 correctness control; no production dispatch"]
fn required_avx2_all_rate_positions_updates_and_scalar_tails_match_actual_owner() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let named = core::mem::size_of::<Scratch>()
        + 4 * core::mem::size_of::<Sha3_384>()
        + core::mem::size_of::<SerializedState<Sha3_384>>()
        + 4 * MAX_UPDATE;
    assert!(named <= NAMED_WORKER_SCRATCH_LIMIT);
    assert!(16 * NAMED_WORKER_SCRATCH_LIMIT <= 1 << 30);
    let mut permutations = 0;
    for cursor in 0..RATE {
        for length in 0..=MAX_UPDATE {
            let mut actual = streams(4, 512);
            let position = usize::from(actual[0].state.state_snapshot_v1()[200]);
            let advance = (cursor + RATE - position) % RATE;
            for (row, state) in actual.iter_mut().enumerate() {
                state.update(&vec![row as u8 + 1; advance]).unwrap();
            }
            let mut expected = cloned(&actual);
            let bytes: [[u8; MAX_UPDATE]; 4] = core::array::from_fn(|row| {
                core::array::from_fn(|offset| (row * 67 + offset * 19) as u8)
            });
            let input: [&[u8]; 4] = core::array::from_fn(|row| &bytes[row][..length]);
            let count = update_group(&mut actual, &input, true).unwrap();
            assert_eq!(count, usize::from(cursor + length >= RATE));
            permutations += count;
            scalar_update(&mut expected, &input).unwrap();
            assert_same(&actual, &expected);
            for (actual, expected) in actual.iter_mut().zip(&mut expected) {
                let tail = zeroize::Zeroizing::new(vec![0x81; actual.remaining]);
                actual.update(&tail).unwrap();
                expected.update(&tail).unwrap();
                assert_eq!(
                    actual.finalize_in_place_v1(),
                    expected.finalize_in_place_v1()
                );
                assert!(actual.update(&[]).is_err());
                assert!(
                    actual
                        .state
                        .state_snapshot_v1()
                        .iter()
                        .all(|&byte| byte == 0)
                );
            }
        }
    }
    for rows in 1..=4 {
        for length in [0, 1, 7, 8, 63, 64, 65, 103, 104, 105] {
            for vector in [false, true] {
                let mut actual = streams(rows, length);
                let mut expected = cloned(&actual);
                let data = zeroize::Zeroizing::new(vec![0x9d; length]);
                let bytes = vec![&data[..]; rows];
                let count = update_group(&mut actual, &bytes, vector).unwrap();
                if !vector || rows != 4 || length > MAX_UPDATE {
                    assert_eq!(count, 0);
                }
                scalar_update(&mut expected, &bytes).unwrap();
                assert_same(&actual, &expected);
                for (actual, expected) in actual.iter_mut().zip(&mut expected) {
                    assert_eq!(
                        actual.finalize_in_place_v1(),
                        expected.finalize_in_place_v1()
                    );
                }
            }
        }
    }
    assert!(permutations > 0);
    println!(
        "sha3_avx2_rate_control four_state_permutations={permutations} named_worker_scratch_bytes={named} parity=exact"
    );
}

#[test]
#[ignore = "mandatory native SHA3 refusal/clearing control; no production dispatch"]
fn required_avx2_refusals_fallback_and_clearing_preserve_actual_owner() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let payload = [0xa7; 64];
    let bytes: [&[u8]; 4] = [&payload; 4];
    for variant in 0..4 {
        let mut actual = streams(4, 512);
        match variant {
            0 => actual[1].update(&[7]).unwrap(), // unequal cursor and remaining
            1 => actual[1] = streams(1, 511).pop().unwrap(), // equal cursor, unequal remaining
            2 => actual[1].update(&[7; RATE]).unwrap(), // equal cursor after one whole rate
            _ => {
                actual[1] = streams(1, 513).pop().unwrap();
                actual[1].update(&[7]).unwrap();
            } // equal remaining, unequal cursor
        }
        let mut expected = cloned(&actual);
        assert_eq!(update_group(&mut actual, &bytes, true).unwrap(), 0);
        scalar_update(&mut expected, &bytes).unwrap();
        assert_same(&actual, &expected);
    }
    for index in 0..4 {
        let mut actual = streams(4, 64);
        actual[index] = streams(1, 63).pop().unwrap();
        let original = cloned(&actual);
        assert!(update_group(&mut actual, &bytes, true).is_err());
        assert_same(&actual, &original);
        assert!(actual[0].finalize_in_place_v1().is_err());
    }
    let mut sealed = streams(4, 0);
    for stream in &mut sealed {
        stream.finalize_in_place_v1().unwrap();
    }
    let original = cloned(&sealed);
    let empty: [&[u8]; 4] = [&[]; 4];
    assert!(update_group(&mut sealed, &empty, true).is_err());
    assert_same(&sealed, &original);
    for outcome in 0..3 {
        let mut actual = streams(4, 64);
        let original = cloned(&actual);
        CLEARS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            update_group_with_checkpoint(&mut actual, &bytes, true, || match outcome {
                0 => Ok(()),
                1 => Err(PrivacyOuterLastFieldStreamErrorV1::FramingLimitExceeded),
                _ => panic!("injected SHA3 test-only precommit unwind"),
            })
        }));
        let cleared = CLEARS.with(|slot| slot.borrow_mut().take().unwrap());
        assert_eq!(
            cleared,
            vec![("replacements", false, true), ("scratch", false, true)]
        );
        if outcome != 0 {
            assert_same(&actual, &original);
        }
        match outcome {
            0 => assert!(result.unwrap().is_ok()),
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
    }
    println!(
        "sha3_avx2_refusal_control precommit_refusals=exact scratch_clear=success,error,unwind scalar_fallback=executed"
    );
}

use crate::privacy_engines::{
    aggregate_stark as aggregate,
    transparent_stark::{GoldilocksFieldV1 as F, TransparentStarkDigestContextV1},
};
use rayon::prelude::*;

const BASE_LEAF: &[u8] = b"iroha:privacy:zk-x509:stark:base-leaf:v1";
const BASE_NODE: &[u8] = b"iroha:privacy:zk-x509:stark:base-node:v1";
const AUX_LEAF: &[u8] = b"iroha:privacy:zk-x509:stark:aux-leaf:v1";
const AUX_NODE: &[u8] = b"iroha:privacy:zk-x509:stark:aux-node:v1";

fn row_context() -> TransparentStarkDigestContextV1 {
    TransparentStarkDigestContextV1::x509_scoped_v1([0x37; 32], PrivacyOuterProofFamilyV1::Main)
}

fn row_streams(rows: usize, width: usize, leaf: &[u8]) -> Vec<PrivacyOuterLastFieldStreamV1> {
    assert!(rows > 0 && rows <= 1 << 22 && width > 0 && width <= usize::from(u16::MAX));
    let catalog = TransparentStarkDigestContextV1::catalog_v1();
    let domain = row_context()
        .domain_v1(&catalog, leaf, b"vector-row-leaf", 0, 0, 0)
        .unwrap();
    let prefix = PrivacyOuterDomainPrefixV1::new(domain).unwrap();
    let mut output = Vec::new();
    output.try_reserve_exact(rows).unwrap();
    assert_eq!(output.capacity(), rows);
    for row in 0..rows {
        output.push(
            prefix
                .last_field_stream_at_with_counter(
                    row as u64,
                    u64::from(u16::MAX),
                    &[
                        &u16::MAX.to_be_bytes(),
                        &u16::try_from(width).unwrap().to_be_bytes(),
                    ],
                    width.checked_mul(8).unwrap(),
                )
                .unwrap(),
        );
    }
    output
}

fn field_columns(rows: usize, first: usize, count: usize) -> zeroize::Zeroizing<Vec<Vec<F>>> {
    assert!(count > 0 && count <= 8 && rows <= 1 << 22);
    zeroize::Zeroizing::new(
        (first..first + count)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        if (row + column).is_multiple_of(17) {
                            F(
                                crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1
                                    - 1,
                            )
                        } else {
                            F((row as u64)
                                .wrapping_mul(31)
                                .wrapping_add(column as u64 * 17))
                        }
                    })
                    .collect()
            })
            .collect(),
    )
}

/// Candidate packing driver retains the current whole-batch canonicality
/// pre-scan and 1024-row task geometry. The additional stream preflight is timed.
fn absorb_fields(
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    columns: &[Vec<F>],
    vector: bool,
) -> Result<usize, aggregate::AggregateStarkErrorV1> {
    use aggregate::AggregateStarkErrorV1 as Error;
    if streams.is_empty()
        || columns.is_empty()
        || columns.len() > 8
        || columns.iter().any(|column| column.len() != streams.len())
    {
        return Err(Error::InvalidLayout);
    }
    if columns
        .iter()
        .flatten()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(Error::NonCanonicalField);
    }
    let bytes = columns.len() * 8;
    if streams
        .iter()
        .any(|stream| stream.remaining == usize::MAX || stream.remaining < bytes)
    {
        return Err(Error::InvalidLayout);
    }
    streams
        .par_chunks_mut(1024)
        .enumerate()
        .map(|(chunk_index, streams)| {
            let mut packed = zeroize::Zeroizing::new([[0u8; MAX_UPDATE]; 4]);
            let mut permutations = 0;
            for (group_index, group) in streams.chunks_mut(4).enumerate() {
                let count = group.len();
                for row in 0..count {
                    let index = chunk_index * 1024 + group_index * 4 + row;
                    for (column, values) in columns.iter().enumerate() {
                        packed[row][column * 8..column * 8 + 8]
                            .copy_from_slice(&values[index].0.to_be_bytes());
                    }
                }
                let input: [&[u8]; 4] = core::array::from_fn(|row| &packed[row][..bytes]);
                permutations += update_group(group, &input[..count], vector)
                    .map_err(|_| Error::InvalidLayout)?;
            }
            Ok(permutations)
        })
        .try_reduce(|| 0, |left, right| Ok(left + right))
}

fn finish_rows(
    streams: &mut [PrivacyOuterLastFieldStreamV1],
    node: &'static [u8],
    queries: &[usize],
) -> aggregate::StreamingMerkleCommitmentV1 {
    aggregate::streaming_merkle_commitment_v1(
        row_context(),
        node,
        streams.len(),
        queries,
        streams.iter_mut().map(|stream| {
            stream
                .finalize_in_place_v1()
                .map_err(|_| aggregate::AggregateStarkErrorV1::InvalidLayout)
        }),
    )
    .unwrap()
}

#[test]
#[ignore = "mandatory canonical row packing/root/frontier parity and refusal control"]
fn required_avx2_canonical_rows_match_streaming_builder_and_refuse_before_mutation() {
    assert!(std::is_x86_feature_detected!("avx2"));
    let mut permutations = 0;
    for (leaf, node) in [(BASE_LEAF, BASE_NODE), (AUX_LEAF, AUX_NODE)] {
        for rows in [1, 2, 4, 8, 64, 2048] {
            for width in [1, 2, 7, 8, 9, 13, 31] {
                let mut query = vec![0, rows / 2, rows - 1];
                query.sort_unstable();
                query.dedup();
                let mut expected = aggregate::StreamingRowCommitmentV1::new(
                    row_context(),
                    leaf,
                    node,
                    usize::from(u16::MAX),
                    rows,
                    width,
                    &query,
                )
                .unwrap();
                let mut actual = row_streams(rows, width, leaf);
                for first in (0..width).step_by(8) {
                    let columns = field_columns(rows, first, (width - first).min(8));
                    expected.absorb_columns_v1(&columns).unwrap();
                    permutations += absorb_fields(&mut actual, &columns, true).unwrap();
                }
                assert_eq!(
                    finish_rows(&mut actual, node, &query),
                    expected.finish().unwrap().commitment
                );
                for state in &actual {
                    assert_eq!(state.remaining, usize::MAX);
                    assert!(
                        state
                            .state
                            .state_snapshot_v1()
                            .iter()
                            .all(|&byte| byte == 0)
                    );
                }
            }
        }
    }
    for bad in 0..4 {
        let mut actual = row_streams(8, 8, BASE_LEAF);
        let before = cloned(&actual);
        let mut expected = aggregate::StreamingRowCommitmentV1::new(
            row_context(),
            BASE_LEAF,
            BASE_NODE,
            usize::from(u16::MAX),
            8,
            8,
            &[],
        )
        .unwrap();
        let mut columns = field_columns(8, 0, 8);
        match bad {
            0 => {
                columns[7][7] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1)
            }
            1 => {
                columns[7].pop();
            }
            2 => {
                for column in &mut *columns {
                    column.zeroize();
                }
                columns.clear();
            }
            _ => columns.push(vec![F::ONE; 8]),
        }
        assert!(expected.absorb_columns_v1(&columns).is_err());
        assert!(absorb_fields(&mut actual, &columns, true).is_err());
        assert_same(&actual, &before);
        let good = field_columns(8, 0, 8);
        expected.absorb_columns_v1(&good).unwrap();
        absorb_fields(&mut actual, &good, true).unwrap();
        assert_eq!(
            finish_rows(&mut actual, BASE_NODE, &[]),
            expected.finish().unwrap().commitment
        );
    }
    assert!(permutations > 0);
    println!(
        "sha3_avx2_row_control four_state_permutations={permutations} framing=canonical root_frontier=exact canonicality_refusal=before_mutation"
    );
}

#[test]
#[ignore = "paired exact-width SHA3 cost diagnostic; requires matched optimized crypto dependencies"]
fn avx2_exact_width_rows_report_paired_owner_cost() {
    // Absorption is the comparative metric. Finalization retains exact output
    // but uses serial candidate leaves versus the current parallel builder;
    // its timing and total are informational, not acceleration evidence.
    // Input generation and column clearing are outside the measured interval.
    use std::time::Instant;
    assert!(std::is_x86_feature_detected!("avx2"));
    assert!(!cfg!(debug_assertions), "optimized diagnostic required");
    let (groups, full_rows, stream_limit, replay_limit, runtime_limit) =
        crate::privacy_engines::zk_x509::stark::ZkX509MainRegistrationShapeV1::row_hash_diagnostic_geometry_v1().unwrap();
    let shapes = [
        (3213usize, BASE_LEAF, BASE_NODE),
        (2618usize, AUX_LEAF, AUX_NODE),
    ];
    let batches: [Vec<core::ops::Range<usize>>; 2] = core::array::from_fn(|kind| {
        let mut offset = 0;
        let mut batches = Vec::new();
        for &(_, base, aux) in &groups {
            let width = if kind == 0 { base } else { aux };
            for first in (0..width).step_by(8) {
                batches.push(offset + first..offset + (first + 8).min(width));
            }
            offset += width;
        }
        assert_eq!(offset, shapes[kind].0);
        batches
    });
    assert_eq!([batches[0].len(), batches[1].len()], [404, 329]);
    println!(
        "sha3_avx2_cost_header expected_records=48 workers=8,16 rows=4096,262144 rounds=3 canonical_framing=true conversion_clear_cost_timed=true input_generation_and_column_clear_timed=false primary_metric=absorb_ns candidate_leaf_finalize=serial current_leaf_finalize=parallel native_group_boundaries=true wholeproof_claim=false groups={groups:?} full_rows={full_rows} stream_limit={stream_limit} replay_limit={replay_limit} runtime_limit={runtime_limit}"
    );
    let mut records = 0;
    let mut total_permutations = 0;
    for workers in [8, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            for rows in [4096usize, 1 << 18] {
                let row_payload = rows * core::mem::size_of::<PrivacyOuterLastFieldStreamV1>();
                let column_payload = rows * 8 * core::mem::size_of::<F>();
                assert!(rows <= full_rows && row_payload <= stream_limit);
                assert!(column_payload + 8 * core::mem::size_of::<Vec<F>>() <= replay_limit);
                assert!(workers * NAMED_WORKER_SCRATCH_LIMIT <= runtime_limit);
                for (kind, (width, leaf, node)) in shapes.into_iter().enumerate() {
                    let mut expected_root = None;
                    for round in 0..3 {
                        for vector in if round % 2 == 0 { [false, true] } else { [true, false] } {
                            let initial = Instant::now();
                            let mut current = (!vector).then(|| aggregate::StreamingRowCommitmentV1::new(
                                row_context(), leaf, node, usize::from(u16::MAX), rows, width, &[]).unwrap());
                            let mut candidate = vector.then(|| row_streams(rows, width, leaf));
                            let init_ns = initial.elapsed().as_nanos();
                            let mut absorb_ns = 0;
                            let mut permutations = 0;
                            for batch in &batches[kind] {
                                let columns = field_columns(rows, batch.start, batch.len());
                                assert!(columns.capacity() <= 8);
                                assert!(columns.iter().all(|column| column.capacity() == rows));
                                let start = Instant::now();
                                if let Some(builder) = &mut current { builder.absorb_columns_v1(&columns).unwrap(); }
                                else { permutations += absorb_fields(candidate.as_mut().unwrap(), &columns, true).unwrap(); }
                                absorb_ns += start.elapsed().as_nanos();
                            }
                            let finalizing = Instant::now();
                            let root = if let Some(builder) = current { builder.finish().unwrap().commitment.root }
                                else { finish_rows(candidate.as_mut().unwrap(), node, &[]).root };
                            let finalize_ns = finalizing.elapsed().as_nanos();
                            let clearing = Instant::now(); drop(candidate);
                            let final_drop_ns = clearing.elapsed().as_nanos();
                            let total_ns = init_ns + absorb_ns + finalize_ns + final_drop_ns;
                            if let Some(expected) = expected_root { assert_eq!(root, expected); }
                            else { expected_root = Some(root); }
                            assert_eq!(permutations > 0, vector);
                            total_permutations += permutations; records += 1;
                            println!("sha3_avx2_cost workers={workers} rows={rows} width={width} round={round} vector={vector} four_state_permutations={permutations} init_ns={init_ns} absorb_ns={absorb_ns} finalize_ns={finalize_ns} final_drop_ns={final_drop_ns} total_ns={total_ns} row_payload_bytes={row_payload} column_payload_bytes={column_payload} parity=exact");
                        }
                    }
                }
            }
        });
    }
    assert_eq!(records, 48);
    assert!(total_permutations > 0);
    println!(
        "sha3_avx2_cost_complete records={records} four_state_permutations={total_permutations} parity=exact"
    );
}

#[path = "privacy_outer_hash_persistent_avx2_tests.rs"]
mod persistent;
