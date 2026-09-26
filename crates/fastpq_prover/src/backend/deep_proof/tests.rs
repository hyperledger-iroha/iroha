//! Actual canonical carrier size, bounded decode and malformed-input controls.

use std::ops::Range;

use super::*;
use crate::backend::GOLDILOCKS_MODULUS;

fn fp4(seed: usize) -> Fp4 {
    Fp4::new([seed as u64, 0, GOLDILOCKS_MODULUS - 1, 17]).unwrap()
}

fn maximal_queries() -> Vec<usize> {
    // Repeated six-bit windows place one query in each of the top 64 subtrees
    // at every committed depth. All per-tree maxima are attained together.
    let mut queries: Vec<_> = (0..QUERY_COUNT)
        .map(|index| (index | index << 6 | index << 12 | index << 18) & (LDE_ROWS - 1))
        .collect();
    queries.sort_unstable();
    queries
}

fn fixture(queries: &[usize]) -> DeepProof {
    let plans = OpeningPlans::new(queries).unwrap();
    let digest = Digest::new([1, 2, 3, 4, 5, GOLDILOCKS_MODULUS - 1]).unwrap();
    DeepProof {
        row_root: digest,
        quotient_root: digest,
        fri_roots: vec![digest; 6],
        ood: OodAnswers {
            current: (0..COMMITTED_COLUMN_COUNT).map(fp4).collect(),
            next: (0..COMMITTED_COLUMN_COUNT).map(|i| fp4(i + 1000)).collect(),
            quotient: vec![fp4(2000), fp4(2001)],
        },
        rows: queries
            .iter()
            .map(|&index| RowOpening {
                index: index as u32,
                values: RowValues::new(
                    (0..COMMITTED_COLUMN_COUNT)
                        .map(|column| {
                            if column % 2 == 0 {
                                index as u64
                            } else {
                                GOLDILOCKS_MODULUS - 1
                            }
                        })
                        .collect(),
                )
                .unwrap(),
            })
            .collect(),
        quotients: queries
            .iter()
            .map(|&index| QuotientOpening {
                index: index as u32,
                low: fp4(index),
                high: fp4(index + 1),
            })
            .collect(),
        row_siblings: vec![digest; plans.initial.work().siblings],
        quotient_siblings: vec![digest; plans.initial.work().siblings],
        rounds: plans
            .rounds
            .iter()
            .enumerate()
            .map(|(round, plan)| FriRound {
                groups: plans.round_indices[round]
                    .iter()
                    .map(|&index| FriGroup {
                        index: index as u32,
                        values: FriValues::new(
                            (0..ARITIES[round]).map(|slot| fp4(index + slot)).collect(),
                        )
                        .unwrap(),
                    })
                    .collect(),
                siblings: vec![digest; plan.work().siblings],
            })
            .collect(),
        terminal: vec![fp4(9000); TERMINAL_VALUES],
    }
}

fn bare(proof: &DeepProof) -> Vec<u8> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (bytes, flags) = norito::codec::encode_with_header_flags(proof);
    assert_eq!(flags, norito::core::default_encode_flags());
    bytes
}

fn frame(payload: &[u8]) -> Vec<u8> {
    norito::core::frame_bare_with_header_flags::<DeepProof>(
        payload,
        norito::core::default_encode_flags(),
    )
    .unwrap()
}

fn field(bytes: &[u8], index: usize) -> Range<usize> {
    let mut offset = 0;
    for position in 0..=index {
        let (length, prefix) = norito::core::read_len_from_slice_with_flags(
            &bytes[offset..],
            norito::core::default_encode_flags(),
        )
        .unwrap();
        let start = offset + prefix;
        let end = start + length;
        assert!(end <= bytes.len());
        if position == index {
            return start..end;
        }
        offset = end;
    }
    unreachable!()
}

fn nested_field(bytes: &[u8], outer: Range<usize>, index: usize) -> Range<usize> {
    let inner = field(&bytes[outer.clone()], index);
    outer.start + inner.start..outer.start + inner.end
}

fn element(bytes: &[u8], vector: Range<usize>, index: usize) -> Range<usize> {
    let (count, prefix) = norito::core::inspect_seq_len_slice(&bytes[vector.clone()]).unwrap();
    assert!(index < count);
    let inner = field(&bytes[vector.start + prefix..vector.end], index);
    vector.start + prefix + inner.start..vector.start + prefix + inner.end
}

fn reject(proof: &DeepProof) {
    assert!(preflight(proof, &maximal_queries()).is_err());
    let bytes = norito::encode_canonical(proof).unwrap();
    assert!(decode(&bytes, PROOF_BYTE_TARGET).is_err());
}

#[test]
fn exact_linked_upper_frontier_encodes_below_512k_and_roundtrips() {
    let queries = maximal_queries();
    let proof = fixture(&queries);
    let plans = preflight(&proof, &queries).unwrap();
    assert_eq!(plans.initial.work().siblings, 1088);
    assert_eq!(
        plans.rounds.each_ref().map(|plan| plan.work().siblings),
        [832, 576, 384, 192, 64]
    );
    assert_eq!(plans.terminal.work().siblings, 0);
    assert_eq!(plans.terminal.work().parent_hashes, 1);
    assert!(
        plans
            .round_indices
            .iter()
            .all(|indices| indices.len() == QUERY_COUNT)
    );
    let bytes = norito::encode_canonical(&proof).unwrap();
    assert_eq!(bytes.len(), 500_783);
    assert_eq!(bytes.len(), MAX_FRAME_BYTES);
    assert_eq!(bytes.len(), maximum_frame_bytes());
    assert_eq!(
        norito::core::encoded_frame_len(&proof).unwrap(),
        bytes.len()
    );
    assert!(bytes.len() < PROOF_BYTE_TARGET);
    assert_eq!(PROOF_BYTE_TARGET - bytes.len(), 23_505);
    for offset in [0, 1, 7] {
        let mut storage = vec![0; offset];
        storage.extend(&bytes);
        assert_eq!(
            decode(&storage[offset..], PROOF_BYTE_TARGET).unwrap(),
            proof
        );
    }
    for queries in [
        (0..QUERY_COUNT).collect::<Vec<_>>(),
        (0..QUERY_COUNT)
            .map(|index| index * (LDE_ROWS / QUERY_COUNT))
            .collect(),
    ] {
        let proof = fixture(&queries);
        let bytes = norito::encode_canonical(&proof).unwrap();
        preflight(&proof, &queries).unwrap();
        assert!(bytes.len() <= MAX_FRAME_BYTES);
        assert_eq!(decode(&bytes, PROOF_BYTE_TARGET).unwrap(), proof);
    }
    eprintln!(
        "deep_maximal_linked_frame_bytes={}; single_proof_headroom={}",
        bytes.len(),
        PROOF_BYTE_TARGET - bytes.len()
    );
}

#[test]
fn retained_row_is_exact_little_endian_and_rejects_wrong_spans_and_scalars() {
    let row = RowValues::new(
        (0..COMMITTED_COLUMN_COUNT)
            .map(|column| [0, 1, 0x0102_0304_0506_0708, GOLDILOCKS_MODULUS - 1][column % 4])
            .collect(),
    )
    .unwrap();
    let expected: Vec<_> = row.iter().flat_map(|value| value.to_le_bytes()).collect();
    assert_eq!(expected.len(), 2408);
    for flags in
        (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
    {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::codec::encode_with_header_flags(&row).0, expected);
        let (decoded, consumed) =
            norito::core::decode_field_canonical::<RowValues>(&expected).unwrap();
        assert_eq!(decoded, row);
        assert_eq!(consumed, RowValues::BYTES);
    }
    for width in [0, 300, 302, 342] {
        assert!(RowValues::new(vec![0; width]).is_err());
    }
    for length in [0, 1, RowValues::BYTES - 1, RowValues::BYTES + 1] {
        let mut bytes = expected.clone();
        bytes.resize(length, 0);
        assert!(norito::core::decode_field_canonical::<RowValues>(&bytes).is_err());
    }
    for column in 0..COMMITTED_COLUMN_COUNT {
        let mut values = vec![0; COMMITTED_COLUMN_COUNT];
        values[column] = GOLDILOCKS_MODULUS;
        assert!(RowValues::new(values).is_err());
    }
    for column in [0, 150, 300] {
        for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
            let mut bytes = expected.clone();
            bytes[column * 8..column * 8 + 8].copy_from_slice(&invalid.to_le_bytes());
            assert!(norito::core::decode_field_canonical::<RowValues>(&bytes).is_err());
        }
    }
}

#[test]
fn byte_element_allocation_and_outer_budgets_are_enforced() {
    assert!(matches!(
        decode(&[0xff], 0),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            actual: 1,
            max: 0
        })
    ));
    assert!(matches!(
        decode(&vec![0; MAX_FRAME_BYTES + 1], usize::MAX),
        Err(Error::VerifierLimitExceeded {
            max: MAX_FRAME_BYTES,
            ..
        })
    ));
    let proof = fixture(&maximal_queries());
    let bytes = norito::encode_canonical(&proof).unwrap();
    assert!(decode(&bytes, bytes.len() - 1).is_err());
    let limits = decode_limits(bytes.len());
    let (decoded, usage) =
        norito::core::with_decode_limits_measured(limits, || decode(&bytes, bytes.len()));
    assert_eq!(decoded.unwrap(), proof);
    assert_eq!(usage.total_elements(), MAX_TOTAL_ELEMENTS);
    assert!(usage.total_allocated_bytes() >= QUERY_COUNT * size_of::<RowOpening>());
    assert!(usage.total_allocated_bytes() <= MAX_ALLOCATION_CHARGES);
    let allocation = usage.total_allocated_bytes();
    for (elements, allocated) in [
        (MAX_TOTAL_ELEMENTS - 1, allocation),
        (MAX_TOTAL_ELEMENTS, allocation - 1),
    ] {
        let outer = DecodeLimits::new(
            MAX_SEQUENCE_ELEMENTS,
            bytes.len(),
            elements,
            allocated,
            MAX_DECODE_DEPTH,
        );
        assert!(
            norito::core::with_decode_limits_scope(outer, || decode(&bytes, bytes.len())).is_err()
        );
    }
    let exact = DecodeLimits::new(
        MAX_SEQUENCE_ELEMENTS,
        bytes.len(),
        MAX_TOTAL_ELEMENTS,
        allocation,
        MAX_DECODE_DEPTH,
    );
    assert_eq!(
        norito::core::with_decode_limits_scope(exact, || decode(&bytes, bytes.len())).unwrap(),
        proof
    );
    eprintln!(
        "deep_decode_elements={}; allocation_charges={allocation}",
        usage.total_elements()
    );
}

#[test]
fn every_nested_sequence_count_bomb_is_rejected_with_valid_framing() {
    let original = bare(&fixture(&maximal_queries()));
    let mut sequences: Vec<_> = [2, 4, 5, 6, 7, 8, 9]
        .into_iter()
        .map(|index| field(&original, index))
        .collect();
    let ood = field(&original, 3);
    sequences.extend((0..3).map(|index| nested_field(&original, ood.clone(), index)));
    let round = element(&original, field(&original, 8), 0);
    let groups = nested_field(&original, round.clone(), 0);
    sequences.push(groups.clone());
    sequences.push(nested_field(&original, round, 1));
    assert_eq!(sequences.len(), 12);
    for sequence in sequences {
        for count in [u64::MAX, MAX_SEQUENCE_ELEMENTS as u64 + 1] {
            let mut changed = original.clone();
            changed[sequence.start..sequence.start + 8].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&frame(&changed), PROOF_BYTE_TARGET).is_err());
        }
    }
}

#[test]
fn fixed_dimensions_sorted_unique_positions_and_minimal_frontiers_are_mandatory() {
    let queries = maximal_queries();
    let proof = fixture(&queries);
    for part in 0..10 {
        let mut changed = proof.clone();
        match part {
            0 => {
                changed.fri_roots.pop();
            }
            1 => {
                changed.ood.current.pop();
            }
            2 => {
                changed.ood.next.push(Fp4::ZERO);
            }
            3 => {
                changed.ood.quotient.pop();
            }
            4 => {
                changed.rows.pop();
            }
            5 => {
                changed.quotients.pop();
            }
            6 => {
                changed.row_siblings.pop();
            }
            7 => {
                changed.quotient_siblings.pop();
            }
            8 => {
                changed.rounds.pop();
            }
            _ => {
                changed.terminal.pop();
            }
        }
        reject(&changed);
    }
    for change in 0..5 {
        let mut changed = proof.clone();
        match change {
            0 => changed.rows.swap(0, 1),
            1 => changed.rows[1].index = changed.rows[0].index,
            2 => changed.rows[0].index = LDE_ROWS as u32,
            3 => changed.quotients.swap(0, 1),
            _ => changed.quotients[0].index ^= 1,
        }
        reject(&changed);
    }
    for round in 0..ARITIES.len() {
        for change in 0..6 {
            let mut changed = proof.clone();
            match change {
                0 => {
                    changed.rounds[round].groups.pop();
                }
                1 => changed.rounds[round].groups.swap(0, 1),
                2 => changed.rounds[round].groups[0].index = GROUP_LEAVES[round] as u32,
                3 => {
                    let wrong_arity = if ARITIES[round] == 4 { 8 } else { 4 };
                    changed.rounds[round].groups[0].values =
                        FriValues::new(vec![Fp4::ZERO; wrong_arity]).unwrap();
                }
                4 => {
                    changed.rounds[round].siblings.pop();
                }
                _ => changed.rounds[round].siblings.push(Digest::default()),
            }
            reject(&changed);
        }
    }
    let mut other_queries = queries.clone();
    other_queries[0] = 1;
    assert!(preflight(&proof, &other_queries).is_err());
    assert!(OpeningPlans::new(&queries[..63]).is_err());
    assert!(OpeningPlans::new(&vec![0; 64]).is_err());
    assert!(OpeningPlans::new(&vec![LDE_ROWS; 64]).is_err());
}

#[test]
fn each_scalar_family_and_digest_lane_rejects_noncanonical_wire_values() {
    let original = bare(&fixture(&maximal_queries()));
    let mut scalars = vec![field(&original, 0), field(&original, 1)];
    scalars.push(element(&original, field(&original, 2), 0));
    let ood = field(&original, 3);
    for part in 0..3 {
        scalars.push(element(
            &original,
            nested_field(&original, ood.clone(), part),
            0,
        ));
    }
    let row = element(&original, field(&original, 4), 0);
    scalars.push(nested_field(&original, row, 1));
    let quotient = element(&original, field(&original, 5), 0);
    scalars.push(nested_field(&original, quotient.clone(), 1));
    scalars.push(nested_field(&original, quotient, 2));
    scalars.push(element(&original, field(&original, 6), 0));
    scalars.push(element(&original, field(&original, 7), 0));
    let round = element(&original, field(&original, 8), 0);
    let group = element(&original, nested_field(&original, round.clone(), 0), 0);
    let mut fiber = nested_field(&original, group, 1);
    fiber.start += 1; // The fixed fiber's one-byte arity tag is not a field limb.
    scalars.push(fiber);
    scalars.push(element(&original, nested_field(&original, round, 1), 0));
    scalars.push(element(&original, field(&original, 9), 0));
    for span in scalars {
        let words = span.len() / 8;
        for lane in 0..words.min(6) {
            let mut changed = original.clone();
            let start = span.start + lane * 8;
            changed[start..start + 8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
            assert!(decode(&frame(&changed), PROOF_BYTE_TARGET).is_err());
        }
    }
}

#[test]
fn canonical_frame_schema_flags_and_complete_bytes_are_required() {
    #[derive(NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "fastpq:test:unrelated-deep-frame",
        frame = "fastpq:test:unrelated-deep-frame"
    )]
    struct OtherFrame {
        value: u8,
    }
    let proof = fixture(&maximal_queries());
    let payload = bare(&proof);
    let wrong_schema = norito::core::frame_bare_with_header_flags::<OtherFrame>(
        &payload,
        norito::core::default_encode_flags(),
    )
    .unwrap();
    assert!(decode(&wrong_schema, PROOF_BYTE_TARGET).is_err());
    let bytes = norito::encode_canonical(&proof).unwrap();
    for length in [0, norito::core::Header::SIZE - 1, bytes.len() - 1] {
        assert!(decode(&bytes[..length], PROOF_BYTE_TARGET).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing, PROOF_BYTE_TARGET).is_err());
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(decode(&corrupt, PROOF_BYTE_TARGET).is_err());
    for flags in
        (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
    {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&proof).unwrap(), bytes);
        assert_eq!(decode(&bytes, PROOF_BYTE_TARGET).unwrap(), proof);
    }
}
