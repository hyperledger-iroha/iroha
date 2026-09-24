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

#[test]
fn doubled_degree_geometry_changes_the_bound_context_and_root_commitment() {
    let current = context();
    let old_descriptor = StatementContext {
        layout: LAYOUT_ID.to_owned(),
        trace_rows: TRACE_ROWS as u32,
        lde_rows: LDE_ROWS as u32,
        columns: COMMITTED_COLUMN_COUNT as u32,
        constraints: CONSTRAINTS as u32,
        modulus: MODULUS,
        extension_nonresidue: 7,
        lde_root: LDE_ROOT,
        coset_offset: COSET_OFFSET,
        fri_arities: FRI_ARITIES.map(|v| v as u32),
        fri_lengths: FRI_LENGTHS.map(|v| v as u32),
        fri_degrees: [65_536, 4_096, 256, 32, 4, 1],
        query_count: QUERY_COUNT as u32,
        query_candidates: QUERY_CANDIDATES as u32,
        statement: b"complete immutable public statement".to_vec(),
    };
    let old = Context {
        framing: FramingContext::new_deep(&norito::encode_canonical(&old_descriptor).unwrap())
            .unwrap(),
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
fn ten_messages_637_blocks_and_exact_commitment_order() {
    assert_eq!(
        (1..=10).map(|i| Round(i).tape_bytes() / 48).sum::<usize>(),
        637
    );
    assert_eq!(Round(2).tape_bytes(), 29_568);
    assert_eq!(Round(10).tape_bytes(), 624);
    for round in [0, 11, u8::MAX] {
        assert!(Round::new(round).is_err());
    }
    let mut transcript = Transcript::new(context());
    let zero_row = [F::ZERO; COMMITTED_COLUMN_COUNT];
    assert!(
        transcript
            .commit_root(Oracle::Row, Digest::default())
            .is_err()
    );
    for ordinal in 1..=10 {
        let message = raw_challenge(&mut transcript, Round(ordinal)).unwrap();
        match ordinal {
            1 => assert_eq!(message, Message::Dummy),
            2 => assert!(matches!(message, Message::Fields(values) if values.len() == 923)),
            3..=9 => assert!(matches!(message, Message::Fields(values) if values.len() == 1)),
            10 => assert_eq!(message, Message::Queries((0..64).collect())),
            _ => unreachable!(),
        }
        assert!(transcript.challenge().is_err());
        if ordinal == 10 {
            break;
        }
        let root = Digest::new([u64::from(ordinal); 6]).unwrap();
        if ordinal == 3 {
            assert!(transcript.commit_root(Oracle::Row, root).is_err());
            transcript
                .commit_ood(&zero_row, &zero_row, &[F::ZERO; 2])
                .unwrap();
        } else {
            assert!(
                transcript
                    .commit_ood(&zero_row, &zero_row, &[F::ZERO; 2])
                    .is_err()
            );
            let expected = match ordinal {
                1 => Oracle::Row,
                2 => Oracle::QuotientPair,
                4..=8 => Oracle::Fri(ordinal - 4),
                9 => Oracle::Terminal,
                _ => unreachable!(),
            };
            assert!(transcript.commit_root(Oracle::Fri(255), root).is_err());
            transcript.commit_root(expected, root).unwrap();
        }
    }
    assert_eq!(transcript.phase, Phase::Complete);
    assert!(
        transcript
            .commit_root(Oracle::Terminal, Digest::default())
            .is_err()
    );
}

#[test]
fn all_tape_coordinates_are_canonical_and_alpha_coordinates_are_independent() {
    for ordinal in 1..=10 {
        let round = Round(ordinal);
        let original = tape(round, 0..(round.tape_bytes() / 8) as u64);
        let decoded = decode(round, &original).unwrap();
        if let Message::Fields(values) = decoded {
            for (i, value) in values.into_iter().enumerate() {
                assert_eq!(
                    value.coefficients(),
                    core::array::from_fn(|lane| (4 * i + lane) as u64)
                );
            }
        }
        for index in 0..original.len() / 8 {
            let mut malformed = original.clone();
            malformed[index * 8..(index + 1) * 8].copy_from_slice(&MODULUS.to_le_bytes());
            assert!(matches!(decode(round, &malformed), Err(BindingError::Tape)));
        }
        assert!(matches!(
            decode(round, &original[..original.len() - 8]),
            Err(BindingError::Tape)
        ));
    }
}

#[test]
fn every_unused_canonical_coordinate_is_bound_before_the_next_message() {
    let context = context();
    let root = Digest::new([19; 6]).unwrap();
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
            changed[index * 8..(index + 1) * 8].copy_from_slice(&123456_u64.to_le_bytes());
            assert_eq!(
                decode(round, &raw).unwrap(),
                decode(round, &changed).unwrap()
            );
            assert_ne!(context.chain(round, &changed, root).unwrap(), expected);
        }
    }
}

#[test]
fn query_candidate_74_is_used_and_four_padding_words_are_only_padding() {
    let round = Round(10);
    let mut words = vec![0; 78];
    for (i, value) in words.iter_mut().enumerate().take(63) {
        *value = i as u64;
    }
    words[73] = 63;
    words[74..].fill(MODULUS - 1);
    assert_eq!(
        decode(round, &tape(round, words.clone())).unwrap(),
        Message::Queries((0..64).collect())
    );
    words[73] = MODULUS - 1;
    words[74] = 63;
    assert!(matches!(
        decode(round, &tape(round, words)),
        Err(BindingError::Exhausted)
    ));
    let reversed = tape(round, (0..78).rev());
    assert_eq!(
        decode(round, &reversed).unwrap(),
        Message::Queries((14..78).collect())
    );
}

#[test]
fn base_ood_sampler_encoding_and_fill_failures_are_permanent_abort() {
    for ordinal in [1, 2, 3, 4, 9, 10] {
        let mut transcript = Transcript::new(context());
        transcript.phase = Phase::Ready(Round(ordinal));
        let result = transcript.challenge_with(|_, _, _, output| {
            output.fill(0);
            if ordinal != 3 && ordinal != 10 {
                output[..8].copy_from_slice(&MODULUS.to_le_bytes());
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(transcript.phase, Phase::Aborted);
        assert!(transcript.challenge().is_err());
        assert!(
            transcript
                .commit_root(Oracle::Row, Digest::default())
                .is_err()
        );
    }
    for lane in 1..4 {
        let mut words = [0; 6];
        words[lane] = 1;
        assert!(decode(Round(3), &tape(Round(3), words)).is_ok());
    }
    let mut transcript = Transcript::new(context());
    assert!(
        transcript
            .challenge_with(|_, _, _, _| Err(BindingError::Shape))
            .is_err()
    );
    assert_eq!(transcript.phase, Phase::Aborted);
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
        raw: tape(Round(3), 0..6),
    };
    assert!(transcript.commit_ood(&[], next, quotient).is_err());
    assert_eq!(transcript.phase, Phase::Aborted);
}

#[test]
fn all_fixed_oracle_shapes_and_full_terminal_are_checked() {
    let context = context();
    for oracle in [
        Oracle::Row,
        Oracle::QuotientPair,
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
            .hash_leaf(oracle, (leaves - 1) as u32, &payload)
            .unwrap();
        assert!(context.hash_leaf(oracle, leaves as u32, &payload).is_err());
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
                Digest::new([1; 6]).unwrap()
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
            .hash_parent(Oracle::QuotientPair, 1, 0, zero, zero)
            .unwrap()
    );
    let old = FramingContext::new(b"complete immutable public statement").unwrap();
    assert_ne!(
        expected,
        old.hash_parent(super::super::compact_v1::Oracle::Row, 1, 0, zero, zero)
            .unwrap()
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
            .hash_leaf(Oracle::QuotientPair, 0, &[0; 64])
            .unwrap(),
        context
            .hash_leaf(Oracle::QuotientPair, 1, &[0; 64])
            .unwrap()
    );
}

#[test]
fn real_challenge_uses_every_block_and_preserves_full_raw_tape() {
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
    assert_eq!(raw.len(), 616 * 48);
    assert_ne!(&raw[..48], &raw[48..96]);
    assert_ne!(&raw[..48], &raw[615 * 48..]);
    assert_eq!(decode(*round, raw).unwrap(), Message::Fields(alpha));
}
