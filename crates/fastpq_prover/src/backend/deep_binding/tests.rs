//! Fixed transcript, canonicality and shared-framing regression tests.

use super::*;

fn tape(round: Round, values: impl IntoIterator<Item = u64>) -> Vec<u8> {
    let mut bytes = vec![0; round.tape_bytes()];
    for (value, target) in values.into_iter().zip(bytes.chunks_exact_mut(8)) {
        target.copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn context() -> Context {
    Context::new(b"complete immutable public statement").unwrap()
}

/// Narrow one fixed test dimension or coordinate to its `u32` field.
fn test_u32(value: usize) -> u32 {
    u32::try_from(value).expect("fixed test dimension fits u32")
}

#[test]
fn doubled_degree_geometry_changes_the_bound_context_and_root_commitment() {
    let current = context();
    let old_descriptor = StatementContext {
        layout: LAYOUT_ID.to_owned(),
        relation: FIXTURE_RELATION_IDENTITY.to_owned(),
        trace_rows: test_u32(TRACE_ROWS),
        lde_rows: test_u32(LDE_ROWS),
        columns: test_u32(COMMITTED_COLUMN_COUNT),
        constraints: test_u32(CONSTRAINTS),
        modulus: MODULUS,
        extension_nonresidue: 7,
        lde_root: LDE_ROOT,
        coset_offset: COSET_OFFSET,
        fri_arities: FRI_ARITIES.map(test_u32),
        fri_lengths: FRI_LENGTHS.map(test_u32),
        fri_degrees: [65_536, 4_096, 256, 32, 4, 1],
        query_count: test_u32(QUERY_COUNT),
        query_candidates: test_u32(QUERY_CANDIDATES),
        statement: b"complete immutable public statement".to_vec(),
    };
    let old = Context {
        framing: FramingContext::new(&norito::encode_canonical(&old_descriptor).unwrap()).unwrap(),
    };
    let row = vec![0; COMMITTED_COLUMN_COUNT * 8];
    assert_ne!(
        current.hash_leaf(Oracle::Row, 0, &row).unwrap(),
        old.hash_leaf(Oracle::Row, 0, &row).unwrap()
    );
}

fn raw_challenge(transcript: &mut Transcript, round: Round) -> Result<Message> {
    transcript.challenge_with(|_, actual, _, output| {
        assert_eq!(actual, round);
        output.copy_from_slice(&tape(round, 0..(round.tape_bytes() / 8) as u64));
        Ok(())
    })
}

#[test]
fn ten_atomic_raw_messages_and_exact_commitment_order() {
    assert_eq!(
        (1..=10).map(|i| Round(i).tape_bytes()).sum::<usize>(),
        30_920
    );
    assert_eq!(Round(2).tape_bytes(), 29_584);
    assert_eq!(Round(10).tape_bytes(), 744);
    for round in [0, 11, u8::MAX] {
        assert!(Round::new(round).is_err());
    }
    let mut transcript = Transcript::new(context());
    let zero_row = [F::ZERO; COMMITTED_COLUMN_COUNT];
    for ordinal in 1..=10 {
        let message = raw_challenge(&mut transcript, Round(ordinal)).unwrap();
        match ordinal {
            1 => assert_eq!(message, Message::Dummy),
            2 => assert!(matches!(message,Message::Fields(v) if v.len()==923)),
            3..=9 => assert!(matches!(message,Message::Fields(v) if v.len()==1)),
            10 => assert_eq!(message, Message::Queries((0..77).collect())),
            _ => unreachable!(),
        }
        if ordinal == 10 {
            break;
        }
        if ordinal == 3 {
            transcript
                .commit_ood(&zero_row, &zero_row, &[F::ZERO; 2])
                .unwrap();
        } else {
            let oracle = match ordinal {
                1 => Oracle::Row,
                2 => Oracle::QuotientAndMask,
                4..=8 => Oracle::Fri(ordinal - 4),
                9 => Oracle::Terminal,
                _ => unreachable!(),
            };
            transcript
                .commit_root(oracle, Digest::from_bytes([ordinal; 32]))
                .unwrap();
        }
    }
    assert!(matches!(transcript.phase, Phase::Complete));
    assert!(transcript.challenge().is_err());
}
#[test]
fn decoded_coordinates_are_canonical_and_raw_rejections_are_not_reductions() {
    for ordinal in 1..=10 {
        let round = Round(ordinal);
        let original = tape(round, 0..(round.tape_bytes() / 8) as u64);
        if let Message::Fields(values) = decode(round, &original).unwrap() {
            for (i, value) in values.into_iter().enumerate() {
                assert_eq!(
                    value.coefficients(),
                    core::array::from_fn(|lane| (4 * i + lane) as u64)
                );
            }
        }
        assert!(matches!(
            decode(round, &original[..original.len() - 1]),
            Err(BindingError::Tape)
        ));
    }
    let mut words = (0..10).collect::<Vec<u64>>();
    words[0] = MODULUS;
    words[1] = u64::MAX;
    assert_eq!(
        decode(Round(4), &tape(Round(4), words)).unwrap(),
        Message::Fields(vec![F::new([2, 3, 4, 5]).unwrap()])
    );
    assert_eq!(decode(Round(1), &[0xff; 32]).unwrap(), Message::Dummy);
}

#[test]
fn every_unused_raw_coordinate_is_bound_before_the_next_message() {
    let context = context();
    let root = Digest::from_bytes([19; 32]);
    for ordinal in 1..=9 {
        let round = Round(ordinal);
        let raw = tape(round, 0..(round.tape_bytes() / 8) as u64);
        let used = match ordinal {
            1 => 0,
            2 => CONSTRAINTS * 4,
            _ => 4,
        };
        let expected = context.chain(round, &raw, root).unwrap();
        for index in used..raw.len() / 8 {
            let mut changed = raw.clone();
            changed[index * 8..(index + 1) * 8].copy_from_slice(&123_456_u64.to_le_bytes());
            assert_eq!(
                decode(round, &raw).unwrap(),
                decode(round, &changed).unwrap()
            );
            assert_ne!(context.chain(round, &changed, root).unwrap(), expected);
        }
    }
}

#[test]
fn query_candidate_87_is_used_and_raw_suffix_cannot_extend_the_candidate_budget() {
    let round = Round(10);
    let mut words = vec![0; 93];
    for (i, v) in words.iter_mut().enumerate().take(76) {
        *v = i as u64;
    }
    words[86] = 76;
    words[87..].fill(MODULUS - 1);
    assert_eq!(
        decode(round, &tape(round, words.clone())).unwrap(),
        Message::Queries((0..77).collect())
    );
    words[86] = MODULUS - 1;
    words[87] = 76;
    assert!(matches!(
        decode(round, &tape(round, words)),
        Err(BindingError::Exhausted)
    ));
    assert_eq!(
        decode(round, &tape(round, (0..93).rev())).unwrap(),
        Message::Queries((16..93).collect())
    );
}
#[test]
fn sampler_fill_ood_and_schedule_failures_are_permanent_abort() {
    for ordinal in [2, 3, 4, 9, 10] {
        let mut transcript = Transcript::new(context());
        transcript.phase = Phase::Ready(Round(ordinal));
        assert!(
            transcript
                .challenge_with(|_, _, _, output| {
                    output.fill(0xff);
                    Ok(())
                })
                .is_err()
        );
        assert!(matches!(transcript.phase, Phase::Aborted));
        assert!(transcript.challenge().is_err());
        assert!(
            transcript
                .commit_root(Oracle::Row, Digest::default())
                .is_err()
        );
    }
    let mut transcript = Transcript::new(context());
    transcript.phase = Phase::Ready(Round(3));
    assert!(
        transcript
            .challenge_with(|_, _, _, output| {
                output.fill(0);
                Ok(())
            })
            .is_err()
    );
    assert!(matches!(transcript.phase, Phase::Aborted));
    for lane in 1..4 {
        let mut words = [0; 10];
        words[lane] = 1;
        assert!(decode(Round(3), &tape(Round(3), words)).is_ok());
    }
    for kind in 0..4 {
        let mut t = Transcript::new(context());
        if kind != 0 {
            raw_challenge(&mut t, Round(1)).unwrap();
        }
        let result = match kind {
            0 => t.commit_root(Oracle::Row, Digest::default()),
            1 => t.challenge().map(|_| ()),
            2 => t.commit_root(Oracle::Fri(255), Digest::default()),
            _ => t.commit_ood(&[], &[], &[]),
        };
        assert!(result.is_err());
        assert!(matches!(t.phase, Phase::Aborted));
        assert!(t.challenge().is_err());
    }
    let mut t = Transcript::new(context());
    assert!(
        t.challenge_with(|_, _, _, _| Err(BindingError::Shape))
            .is_err()
    );
    assert!(matches!(t.phase, Phase::Aborted));
}

#[test]
fn complete_ood_coordinates_have_fixed_order_and_reject_every_noncanonical_lane() {
    let values: Vec<_> = (0..OOD_VALUES)
        .map(|i| F::new(core::array::from_fn(|lane| (4 * i + lane + 1) as u64)).unwrap())
        .collect();
    let current = &values[..COMMITTED_COLUMN_COUNT];
    let next = &values[COMMITTED_COLUMN_COUNT..2 * COMMITTED_COLUMN_COUNT];
    let quotient = &values[2 * COMMITTED_COLUMN_COUNT..];
    let bytes = ood_bytes(current, next, quotient).unwrap();
    assert_eq!(bytes.len(), 604 * 32);
    for (index, word) in bytes.chunks_exact(8).enumerate() {
        assert_eq!(
            u64::from_le_bytes(word.try_into().unwrap()),
            index as u64 + 1
        );
    }
    for coordinate in 0..OOD_VALUES * 4 {
        let mut malformed = values.clone();
        let mut words = malformed[coordinate / 4].coefficients();
        words[coordinate % 4] = MODULUS;
        malformed[coordinate / 4] = F::from_coefficients_unchecked_for_test(words);
        assert!(ood_bytes(&malformed[..301], &malformed[301..602], &malformed[602..]).is_err());
    }
    let context = context();
    let digest = context.hash_ood(current, next, quotient).unwrap();
    assert_ne!(digest, context.hash_ood(next, current, quotient).unwrap());
    assert_ne!(
        digest,
        context
            .hash_ood(current, next, &[quotient[1], quotient[0]])
            .unwrap()
    );
    for width in [0, 300, 302] {
        assert!(ood_bytes(&vec![F::ZERO; width], next, quotient).is_err());
    }
    let mut transcript = Transcript::new(context);
    transcript.phase = Phase::Pending {
        round: Round(3),
        raw: RawTapeV1::from_bytes(RawTapeRoundV1::new(3).unwrap(), &tape(Round(3), 0..10))
            .unwrap(),
    };
    assert!(transcript.commit_ood(&[], next, quotient).is_err());
    assert!(matches!(transcript.phase, Phase::Aborted));
}

#[test]
fn all_fixed_oracle_shapes_and_full_terminal_are_checked() {
    let context = context();
    // The old two-value leaf is not another accepted candidate encoding.
    assert!(
        context
            .hash_leaf(Oracle::QuotientAndMask, 0, &[0; 64])
            .is_err()
    );
    let mut payload = [0; 96];
    let original = context
        .hash_leaf(Oracle::QuotientAndMask, 0, &payload)
        .unwrap();
    payload[64] = 1;
    assert_ne!(
        original,
        context
            .hash_leaf(Oracle::QuotientAndMask, 0, &payload)
            .unwrap()
    );
    for oracle in [
        Oracle::Row,
        Oracle::QuotientAndMask,
        Oracle::Fri(0),
        Oracle::Fri(1),
        Oracle::Fri(2),
        Oracle::Fri(3),
        Oracle::Fri(4),
        Oracle::Terminal,
    ] {
        let (_, _, leaves, bytes) = oracle.shape().unwrap();
        let payload = vec![0; bytes];
        let leaf = context
            .hash_leaf(oracle, test_u32(leaves - 1), &payload)
            .unwrap();
        assert!(
            context
                .hash_leaf(oracle, test_u32(leaves), &payload)
                .is_err()
        );
        assert!(context.hash_leaf(oracle, 0, &payload[..bytes - 8]).is_err());
        let mut bad = payload;
        bad[bytes - 8..].copy_from_slice(&MODULUS.to_le_bytes());
        assert!(context.hash_leaf(oracle, 0, &bad).is_err());
        let depth = leaves.ilog2().max(1);
        context.hash_parent(oracle, depth, 0, leaf, leaf).unwrap();
        assert!(context.hash_parent(oracle, 0, 0, leaf, leaf).is_err());
        assert!(
            context
                .hash_parent(oracle, depth + 1, 0, leaf, leaf)
                .is_err()
        );
        assert!(context.hash_parent(oracle, depth, 1, leaf, leaf).is_err());
    }
    assert_eq!(Oracle::Row.shape().unwrap().2, 1 << 23);
    assert_eq!(Oracle::Terminal.shape().unwrap().3, 4096);
    assert!(context.hash_leaf(Oracle::Fri(5), 0, &[0; 128]).is_err());
    assert!(
        context
            .hash_parent(
                Oracle::Terminal,
                1,
                0,
                Digest::default(),
                Digest::from_bytes([1; 32])
            )
            .is_err()
    );
}

#[test]
fn shared_framing_binds_statement_profile_oracle_and_fri_round() {
    assert!(Context::new(b"").is_err());
    assert!(Context::new(&vec![0; MAX_STATEMENT_BYTES + 1]).is_err());
    let context = context();
    let changed = Context::new(b"different full public statement").unwrap();
    let zero = Digest::default();
    let expected = context.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap();
    assert_ne!(
        expected,
        changed.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap()
    );
    assert_ne!(
        expected,
        context
            .hash_parent(Oracle::QuotientAndMask, 1, 0, zero, zero)
            .unwrap()
    );
    let raw = Context {
        framing: FramingContext::new(b"complete immutable public statement").unwrap(),
    };
    assert_ne!(
        expected,
        raw.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap()
    );
    assert_ne!(
        context.hash_leaf(Oracle::Fri(0), 0, &[0; 512]).unwrap(),
        context.hash_leaf(Oracle::Fri(1), 0, &[0; 512]).unwrap()
    );
    assert_ne!(
        context.hash_leaf(Oracle::Fri(2), 0, &[0; 256]).unwrap(),
        context.hash_leaf(Oracle::Fri(3), 0, &[0; 256]).unwrap()
    );
    assert_ne!(
        context
            .hash_leaf(Oracle::QuotientAndMask, 0, &[0; 96])
            .unwrap(),
        context
            .hash_leaf(Oracle::QuotientAndMask, 1, &[0; 96])
            .unwrap()
    );
}

#[test]
fn real_challenge_uses_exact_whole_raw_extent_and_preserves_full_raw_tape() {
    let mut transcript = Transcript::new(context());
    assert_eq!(transcript.challenge().unwrap(), Message::Dummy);
    transcript
        .commit_root(Oracle::Row, Digest::default())
        .unwrap();
    let Message::Fields(alpha) = transcript.challenge().unwrap() else {
        panic!("alpha");
    };
    assert_eq!(alpha.len(), CONSTRAINTS);
    let Phase::Pending { round, raw } = &transcript.phase else {
        panic!("pending");
    };
    assert_eq!(*round, Round(2));
    assert_eq!(raw.as_bytes().len(), 29_584);
    assert_ne!(&raw.as_bytes()[..136], &raw.as_bytes()[136..272]);
    assert_ne!(&raw.as_bytes()[..136], &raw.as_bytes()[29_448..]);
    assert_eq!(
        decode(*round, raw.as_bytes()).unwrap(),
        Message::Fields(alpha)
    );
}

#[test]
fn prepared_relation_identity_is_explicit_bounded_and_separate_from_raw_fixtures() {
    use crate::{
        backend::compact_transfer_air::CompactTransferAir,
        gadgets::compact_smt_air::{PublicStatement, PublicUpdate},
    };
    let mut marker = [0; 8];
    marker[7] = 1 << 24;
    let statement = PublicStatement {
        updates: [PublicUpdate {
            old_leaf: marker,
            new_leaf: marker,
            path: 0,
        }; 2],
        old_root: marker,
        new_root: marker,
    };
    let relation =
        CompactTransferAir::new(&statement, Some(b"complete prepared statement")).unwrap();
    let actual = Context::for_relation(&relation).unwrap();
    let exact =
        Context::with_identity(relation.schema().identity, relation.statement_bytes()).unwrap();
    let fixture = Context::new(relation.statement_bytes()).unwrap();
    let zero = Digest::default();
    let root = actual.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap();
    assert_eq!(
        root,
        exact.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap()
    );
    assert_ne!(
        root,
        fixture.hash_parent(Oracle::Row, 1, 0, zero, zero).unwrap()
    );
    let different = Context::with_identity(
        "another prepared relation identity",
        relation.statement_bytes(),
    )
    .unwrap();
    assert_ne!(
        root,
        different
            .hash_parent(Oracle::Row, 1, 0, zero, zero)
            .unwrap()
    );
    assert!(Context::with_identity("", relation.statement_bytes()).is_err());
    assert!(
        Context::with_identity(
            &"x".repeat(MAX_RELATION_IDENTITY_BYTES + 1),
            relation.statement_bytes()
        )
        .is_err()
    );
    assert!(
        Context::with_identity(
            &"x".repeat(MAX_RELATION_IDENTITY_BYTES),
            relation.statement_bytes()
        )
        .is_ok()
    );
    assert!(core::ptr::eq(&raw const relation, relation.deep_relation()));
}

#[test]
fn exact_keccak_ledger_covers_every_oracle_and_complete_raw_tapes() {
    let context = Context::new(b"source-bound Keccak work ledger").unwrap();
    let mut total = 0;
    for oracle in [
        Oracle::Row,
        Oracle::QuotientAndMask,
        Oracle::Fri(0),
        Oracle::Fri(1),
        Oracle::Fri(2),
        Oracle::Fri(3),
        Oracle::Fri(4),
        Oracle::Terminal,
    ] {
        let (leaf, parent) = context.tree_frame_lengths(oracle).unwrap();
        let (lp, pp) = context.tree_permutations(oracle).unwrap();
        assert_eq!(lp, context.framing.body_permutations(leaf).unwrap());
        assert_eq!(pp, context.framing.body_permutations(parent).unwrap());
        assert!(lp >= 1 && pp >= 1);
        total += lp + pp;
    }
    assert!(context.transcript_permutations().unwrap() > 29584 / 136);
    assert!(total > 8);
}
