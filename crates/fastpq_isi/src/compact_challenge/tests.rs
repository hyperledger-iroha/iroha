//! Exact finite decoders and whole-tape custody for the fixed q77 candidate.

use super::*;
use crate::keccak256::Sha3_256V1;

fn round(n: u8) -> RawTapeRoundV1 {
    RawTapeRoundV1::new(n).unwrap()
}
fn words(n: u8, values: impl IntoIterator<Item = u64>) -> RawTapeV1 {
    let raw: Vec<_> = values.into_iter().flat_map(u64::to_le_bytes).collect();
    RawTapeV1::from_bytes(round(n), &raw).unwrap()
}

#[test]
fn exact_round_geometry_preserves_query_and_hiding_parameters() {
    assert!(RawTapeRoundV1::new(0).is_none());
    assert!(RawTapeRoundV1::new(11).is_none());
    assert_eq!(
        (1..=10).map(|n| round(n).tape_bytes()).collect::<Vec<_>>(),
        [32, 29_584, 80, 80, 80, 80, 80, 80, 80, 744]
    );
    assert_eq!(
        (QUERY_COUNT, QUERY_CANDIDATES, QUERY_RAW_WORDS),
        (77, 87, 93)
    );
    assert_eq!(
        (TRACE_MASK_COEFFICIENTS, QUOTIENT_MASK_COEFFICIENTS),
        (162, 78)
    );
    assert_eq!(COMPOSITION_MASK_COEFFICIENTS, 131_072);
    for n in 1..=10 {
        assert_eq!(round(n).ordinal(), n);
        for length in [round(n).tape_bytes() - 1, round(n).tape_bytes() + 1] {
            assert!(matches!(
                RawTapeV1::from_bytes(round(n), &vec![0; length]),
                Err(RawTapeErrorV1::Length)
            ));
        }
    }
}

#[test]
fn scalar_decoding_rejects_raw_words_without_modular_reduction_or_extra_squeeze() {
    let raw = [
        FIELD_MODULUS,
        u64::MAX,
        1,
        2,
        3,
        4,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    ];
    for n in 3..=9 {
        assert_eq!(
            words(n, raw).decode().unwrap(),
            RawTapeMessageV1::Fields(vec![[1, 2, 3, 4]])
        );
    }
    let exhausted = [
        1,
        2,
        3,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    ];
    for n in 3..=9 {
        assert_eq!(words(n, exhausted).decode(), Err(RawTapeErrorV1::Exhausted));
    }
    assert_eq!(
        words(3, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]).decode(),
        Err(RawTapeErrorV1::BaseOod)
    );
    assert_eq!(
        words(4, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0]).decode().unwrap(),
        RawTapeMessageV1::Fields(vec![[1, 0, 0, 0]])
    );
    assert_eq!(
        words(1, [u64::MAX; 4]).decode().unwrap(),
        RawTapeMessageV1::Dummy
    );
}

#[test]
fn alpha_rejection_is_ordered_and_complete_with_exact_exhaustion_boundary() {
    let accepted: Vec<_> = (1..=CONSTRAINTS * 4).map(|i| i as u64).collect();
    let raw = std::iter::repeat_n(u64::MAX, 6).chain(accepted.iter().copied());
    let RawTapeMessageV1::Fields(fields) = words(2, raw).decode().unwrap() else {
        panic!("alpha fields");
    };
    assert_eq!(fields.len(), CONSTRAINTS);
    for (index, field) in fields.iter().enumerate() {
        assert_eq!(
            *field,
            core::array::from_fn(|limb| accepted[4 * index + limb])
        );
    }
    let raw =
        std::iter::repeat_n(u64::MAX, 7).chain(accepted[..accepted.len() - 1].iter().copied());
    assert_eq!(words(2, raw).decode(), Err(RawTapeErrorV1::Exhausted));
}

#[test]
fn query_decoding_checks_raw_acceptance_unbiased_range_occupancy_and_sorted_census() {
    let limit = FIELD_MODULUS - FIELD_MODULUS % LDE_ROWS as u64;
    let mut raw = vec![u64::MAX; 6];
    raw.extend([limit, limit]);
    raw.extend((0..QUERY_COUNT as u64).rev());
    raw.extend([0; 8]);
    assert_eq!(raw.len(), QUERY_RAW_WORDS);
    assert_eq!(
        words(10, raw).decode().unwrap(),
        RawTapeMessageV1::Queries((0..u32::try_from(QUERY_COUNT).unwrap()).collect())
    );
    // There are already 77 distinct positions, but fewer than 87 accepted field
    // words: accepting here would change the defined whole-message decoder.
    let raw =
        (0..QUERY_COUNT as u64).chain(std::iter::repeat_n(u64::MAX, QUERY_RAW_WORDS - QUERY_COUNT));
    assert_eq!(words(10, raw).decode(), Err(RawTapeErrorV1::Exhausted));
    assert_eq!(
        words(10, [0; QUERY_RAW_WORDS]).decode(),
        Err(RawTapeErrorV1::Exhausted)
    );
}

#[test]
fn query_decoding_preserves_positions_at_the_upper_domain_boundary() {
    let limit = FIELD_MODULUS - FIELD_MODULUS % LDE_ROWS as u64;
    let raw = (1..=QUERY_CANDIDATES as u64)
        .map(|distance| limit - distance)
        .chain(std::iter::repeat_n(u64::MAX, 6));
    let end = u32::try_from(LDE_ROWS).expect("fixed query domain fits u32");
    let count = u32::try_from(QUERY_COUNT).unwrap();
    assert_eq!(
        words(10, raw).decode().unwrap(),
        RawTapeMessageV1::Queries((end - count..end).collect())
    );
}

#[test]
fn raw_unused_suffix_is_retained_even_when_decoded_messages_are_equal() {
    let mut raw = vec![0; round(2).tape_bytes()];
    let left = RawTapeV1::from_bytes(round(2), &raw).unwrap();
    raw[CONSTRAINTS * 32..].fill(0xff);
    let right = RawTapeV1::from_bytes(round(2), &raw).unwrap();
    assert_eq!(left.decode().unwrap(), right.decode().unwrap());
    assert_ne!(left.as_bytes(), right.as_bytes());
    let hash = |tape: &RawTapeV1| {
        let mut h = Sha3_256V1::new();
        h.update(tape.as_bytes());
        h.finalize()
    };
    assert_ne!(hash(&left), hash(&right));
    assert_eq!(left.round(), round(2));
    assert!(left.allocation_bytes() >= MAX_RAW_TAPE_BYTES);
    assert_eq!(
        format!("{left:?}"),
        "RawTapeV1 { round: RawTapeRoundV1(2), bytes: 29584, .. }"
    );
}

#[test]
fn xof_materializes_the_same_complete_tape_across_cached_prefix_splits() {
    let input: Vec<_> = (0..512)
        .map(|i| u8::try_from((i * 37 + 11) & 255).unwrap())
        .collect();
    for n in 1..=10 {
        let empty = Shake256V1::new();
        let full = RawTapeV1::derive(round(n), &empty, &input).unwrap();
        for split in [0, 1, 135, 136, 137, 512] {
            let mut prefix = Shake256V1::new();
            prefix.update(&input[..split]);
            let tape = RawTapeV1::derive(round(n), &prefix, &input[split..]).unwrap();
            assert_eq!(tape.as_bytes(), full.as_bytes());
            assert_eq!(tape.decode().unwrap(), full.decode().unwrap());
        }
    }
}

struct Observation;
impl Observation {
    fn start() -> Self {
        ERASURE.with(|counts| assert_eq!(counts.replace(Some((0, 0))), None));
        Self
    }
    fn counts() -> (usize, usize) {
        ERASURE.with(|counts| counts.get().unwrap())
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        ERASURE.with(|counts| counts.set(None));
    }
}
#[test]
fn entire_owned_raw_tapes_clear_on_return_decode_error_and_unwind() {
    let observation = Observation::start();
    drop(RawTapeV1::from_bytes(round(1), &[0xff; 32]).unwrap());
    assert_eq!(Observation::counts(), (32, 0));
    let error = (|| {
        let tape = RawTapeV1::from_bytes(round(4), &[0xff; 80])?;
        tape.decode()
    })();
    assert_eq!(error, Err(RawTapeErrorV1::Exhausted));
    assert_eq!(Observation::counts(), (112, 0));
    assert!(
        std::panic::catch_unwind(|| {
            let _tape = RawTapeV1::from_bytes(round(10), &[0xff; 744]).unwrap();
            panic!("whole-tape ownership unwind fixture");
        })
        .is_err()
    );
    assert_eq!(Observation::counts(), (856, 0));
    drop(observation);
}
