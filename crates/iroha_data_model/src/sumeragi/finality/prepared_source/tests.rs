//! Canonical parity, immutable source identity and unchanged prepared backing.

use super::*;
use crate::block::{BlockHeader, builder::BlockBuilder};
use std::num::NonZeroU64;

fn limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: 65_536,
        journal_bytes: 131_072,
        block_count: 4,
        allocated_bytes: 1024 * 1024,
    }
}
fn journal() -> NativeFinalityJournal {
    // This unsigned body is a codec fixture, never an authenticated finality clock.
    let block = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        0,
    ))
    .build(std::collections::BTreeSet::default());
    let artifact = NativeFinalityArtifact::from_block(&block, limits()).unwrap();
    NativeFinalityJournal {
        blocks: vec![artifact.clone(), artifact],
    }
}

#[test]
fn prepared_native_source_matches_canonical_owned_journal_and_borrows_original_frame() {
    let journal = journal();
    let bytes = norito::encode_canonical(&journal).unwrap();
    let pool = AllocationBudget::new(2 * 1024 * 1024);
    let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
    assert!(prepared.belongs_to(&pool));
    assert!(!prepared.belongs_to(&AllocationBudget::new(pool.limit_bytes())));
    let credits = pool.reserved_bytes();
    assert!(!prepared.is_decoded());
    assert!(matches!(
        prepared.view(&bytes),
        Err(PreparedNativeFinalityError::NotDecoded)
    ));
    prepared.decode(&bytes).unwrap();
    let view = prepared.view(&bytes).unwrap();
    assert_eq!(view.len(), 2);
    assert!(!view.is_empty());
    view.validate(limits()).unwrap();
    let mut wires = view.blocks();
    assert_eq!(wires.len(), 2);
    assert_eq!(wires.size_hint(), (2, Some(2)));
    for original in &journal.blocks {
        let wire = wires.next().unwrap();
        assert_eq!(wire, original.block_wire);
        assert_ne!(wire.as_ptr(), original.block_wire.as_ptr());
        let start = wire.as_ptr().addr() - bytes.as_ptr().addr();
        assert!(start + wire.len() <= bytes.len());
        assert_eq!(wire.as_ptr(), bytes[start..].as_ptr());
    }
    assert_eq!(wires.size_hint(), (0, Some(0)));
    assert!(wires.next().is_none());
    prepared.decode(&bytes).unwrap();
    assert_eq!(pool.reserved_bytes(), credits);
    drop(prepared);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_native_source_rejects_rebound_or_changed_bytes_until_original_consumption() {
    let journal = journal();
    let mut bytes = norito::encode_canonical(&journal).unwrap();
    let copy = bytes.clone();
    let pool = AllocationBudget::new(2 * 1024 * 1024);
    let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
    let spans = prepared.spans.as_slice().as_ptr();
    let blocks = prepared.blocks.as_slice().as_ptr();
    let credits = pool.reserved_bytes();
    prepared.decode(&bytes).unwrap();
    assert!(matches!(
        prepared.decode(&copy),
        Err(PreparedNativeFinalityError::SourceChanged)
    ));
    assert!(matches!(
        prepared.view(&copy),
        Err(PreparedNativeFinalityError::SourceChanged)
    ));
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(matches!(
        prepared.decode(&bytes),
        Err(PreparedNativeFinalityError::SourceChanged)
    ));
    bytes[last] ^= 1;
    prepared.decode(&bytes).unwrap();
    prepared.clear_consumed();
    assert!(!prepared.is_decoded());
    prepared.decode(&copy).unwrap();
    assert_eq!(prepared.spans.as_slice().as_ptr(), spans);
    assert_eq!(prepared.blocks.as_slice().as_ptr(), blocks);
    assert_eq!(pool.reserved_bytes(), credits);
    drop(prepared);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_native_source_preserves_enclosing_refusal_and_canonical_invalidity() {
    let journal = journal();
    let bytes = norito::encode_canonical(&journal).unwrap();
    let pool = AllocationBudget::new(2 * 1024 * 1024);
    let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
    let credits = pool.reserved_bytes();
    let error = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || prepared.decode(&bytes),
    )
    .unwrap_err();
    let PreparedNativeFinalityError::Decode(PreparedDecodeError::Codec(error)) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        error.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    assert!(!prepared.is_decoded());
    assert_eq!(pool.reserved_bytes(), credits);
    prepared.decode(&bytes).unwrap();
    prepared.clear_consumed();
    let truncated = &bytes[..bytes.len() - 1];
    assert!(NativeFinalityJournal::decode(truncated, limits()).is_err());
    let error = prepared.decode(truncated).unwrap_err();
    assert!(
        matches!(error, PreparedNativeFinalityError::Decode(PreparedDecodeError::Codec(ref original)) if original.kind() == norito::core::DecodeAttemptErrorKind::Invalid)
    );
    assert!(!prepared.is_decoded());
    assert_eq!(pool.reserved_bytes(), credits);
}

#[test]
fn prepared_native_source_keeps_exact_count_empty_and_aggregate_bounds() {
    let original = journal();
    let cases = [
        NativeFinalityJournal { blocks: Vec::new() },
        NativeFinalityJournal {
            blocks: vec![NativeFinalityArtifact {
                block_wire: Vec::new(),
            }],
        },
        NativeFinalityJournal {
            blocks: vec![original.blocks[0].clone(); 5],
        },
    ];
    for journal in cases {
        let bytes = norito::encode_canonical(&journal).unwrap();
        let pool = AllocationBudget::new(2 * 1024 * 1024);
        let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
        let credits = pool.reserved_bytes();
        assert!(NativeFinalityJournal::decode(&bytes, limits()).is_err());
        assert!(prepared.decode(&bytes).is_err());
        assert!(!prepared.is_decoded());
        assert_eq!(pool.reserved_bytes(), credits);
    }
    let source = NativeFinalitySource::from(&original);
    assert!(
        source
            .validate(NativeFinalityLimits {
                block_count: 1,
                ..limits()
            })
            .is_err()
    );
    assert!(
        source
            .validate(NativeFinalityLimits {
                block_bytes: 1,
                ..limits()
            })
            .is_err()
    );
    let length = original.blocks[0].block_wire.len();
    assert!(
        source
            .validate(NativeFinalityLimits {
                block_bytes: length,
                journal_bytes: length,
                ..limits()
            })
            .is_err()
    );
}

#[test]
fn prepared_native_source_checks_full_nominal_frame_and_keeps_original_backing_on_invalidity() {
    let journal = journal();
    let original = norito::encode_canonical(&journal).unwrap();
    let wrong_nominal = norito::encode_canonical(&journal.blocks[0]).unwrap();
    let mut bad_checksum = original.clone();
    let last = bad_checksum.len() - 1;
    bad_checksum[last] ^= 1;
    let mut trailing = original.clone();
    trailing.push(0);
    for bytes in [&wrong_nominal, &bad_checksum, &trailing] {
        let pool = AllocationBudget::new(2 * 1024 * 1024);
        let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
        let credits = pool.reserved_bytes();
        let spans = prepared.spans.as_slice().as_ptr();
        let blocks = prepared.blocks.as_slice().as_ptr();
        assert!(NativeFinalityJournal::decode(bytes, limits()).is_err());
        let error = prepared.decode(bytes).unwrap_err();
        assert!(
            matches!(error, PreparedNativeFinalityError::Decode(PreparedDecodeError::Codec(ref cause)) if cause.kind() == norito::core::DecodeAttemptErrorKind::Invalid),
            "{error:?}"
        );
        assert!(!prepared.is_decoded());
        assert_eq!(prepared.spans.as_slice().as_ptr(), spans);
        assert_eq!(prepared.blocks.as_slice().as_ptr(), blocks);
        assert_eq!(pool.reserved_bytes(), credits);
        prepared.clear_consumed();
        prepared.decode(&original).unwrap();
        assert_eq!(
            prepared.view(&original).unwrap().len(),
            journal.blocks.len()
        );
        drop(prepared);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn prepared_native_source_constructor_funds_both_range_arrays_before_source_consumption() {
    let range_bytes = core::mem::size_of::<SequenceSpan>() * limits().block_count;
    let control_bytes = PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum::<usize>();
    let demand = 2 * range_bytes + control_bytes;
    let pool = AllocationBudget::new(demand);
    for allowed in [0, range_bytes, demand - 1] {
        let blocker = pool.try_reserve_bytes(demand - allowed).unwrap();
        let before = pool.reserved_bytes();
        let error = match PreparedNativeFinalityJournal::new(limits(), &pool) {
            Ok(_) => panic!("an unfunded range or control must refuse construction"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            PreparedNativeFinalityError::Storage(ChargedBufferError::Admission(_))
        ));
        assert_eq!(pool.reserved_bytes(), before);
        drop(blocker);
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
    assert_eq!(pool.reserved_bytes(), demand);
    assert!(!prepared.is_decoded());
    drop(prepared);
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(matches!(
        PreparedNativeFinalityJournal::new(
            NativeFinalityLimits {
                block_count: 0,
                ..limits()
            },
            &pool
        ),
        Err(PreparedNativeFinalityError::Invalid(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
}
