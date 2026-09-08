//! FASTPQ backend arithmetic, commitments, query sampling, and retained-layer contracts.

use super::*;
use crate::{OperationKind, PublicInputs, StateTransition};
use std::collections::BTreeSet;

fn fp4(value: u64) -> GoldilocksFp4V1 {
    GoldilocksFp4V1::from_base(value).expect("canonical Goldilocks test value")
}

fn fp4_values(values: &[u64]) -> Vec<GoldilocksFp4V1> {
    values.iter().copied().map(fp4).collect()
}

fn lde_merkle_root(leaves: &[GoldilocksDigest384V1]) -> GoldilocksDigest384V1 {
    merkle_root_with_mode(leaves, MerkleTreeRoleV1::Lde, ExecutionMode::Cpu)
        .expect("typed LDE Merkle root")
}
fn sample_batch(rows: usize) -> TransitionBatch {
    let mut batch =
        TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
    for idx in 0..rows {
        let key = format!("asset/xor/account-{idx:04}").into_bytes();
        let idx_u64 = u64::try_from(idx).expect("sample batch index fits u64");
        let pre = idx_u64.to_le_bytes().to_vec();
        let op = OperationKind::MetaSet;
        let post_value = idx_u64.wrapping_add(1);
        let post = post_value.to_le_bytes().to_vec();
        batch.push(StateTransition::new(key, pre, post, op));
    }
    batch.sort();
    batch
}
#[test]
fn transcript_challenges_are_deterministic() {
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        "fastpq-state-transition-stark-v1",
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    transcript.append_message("tag", b"payload");
    let a = transcript.challenge_field("gamma");
    let b = transcript.challenge_field("gamma");
    assert_ne!(a, 0);
    assert_ne!(b, 0);
    assert_ne!(a, b);
}
#[test]
fn transcript_encoding_ignores_and_restores_ambient_norito_layout() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let public_io = PublicIO {
        slot: 0x1234_5678_90ab_cdef,
        ..PublicIO::default()
    };
    let baseline = Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
    let trace_root = GoldilocksDigest384V1::new([1, 2, 3, 4, 5, 6]).unwrap();
    let air_root = GoldilocksDigest384V1::new([7, 8, 9, 10, 11, 12]).unwrap();
    let mut expected = baseline.clone();
    expected
        .append_trace_oracles(trace_root, air_root, 64, 17)
        .unwrap();
    let expected_challenge = expected.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX);
    let probe = ("layout restoration", vec![1_u64, 2, 3]);
    let canonical_probe = norito::core::to_bytes(&probe).unwrap();
    for flags in [
        0,
        norito::core::header_flags::PACKED_SEQ,
        norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
    ] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::core::to_bytes(&probe).unwrap();
        let actual =
            Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
        assert_eq!(actual.state, baseline.state);
        assert_eq!(actual.counter, baseline.counter);
        let mut appended = baseline.clone();
        appended
            .append_trace_oracles(trace_root, air_root, 64, 17)
            .unwrap();
        assert_eq!(
            appended.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX),
            expected_challenge
        );
        assert_eq!(
            norito::core::to_bytes(&probe).unwrap(),
            before,
            "transcript helpers must restore the caller's layout flags"
        );
        if flags == 0 {
            assert_ne!(
                before, canonical_probe,
                "the control must exercise an alternate layout"
            );
        }
    }
}

#[test]
fn aggregation_challenges_bind_both_trace_roots_and_geometry() {
    let challenge = |trace, air_trace, domain_size, columns| {
        let mut transcript = Transcript::initialise(
            &PublicIO::default(),
            fastpq_isi::FASTPQ_FINAL_V1.name,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .unwrap();
        transcript
            .append_trace_oracles(trace, air_trace, domain_size, columns)
            .unwrap();
        transcript.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX)
    };
    let digest = |word| GoldilocksDigest384V1::new([word; 6]).unwrap();
    let baseline = challenge(digest(1), digest(2), 64, 10);
    assert_eq!(baseline, challenge(digest(1), digest(2), 64, 10));
    assert_ne!(baseline, challenge(digest(3), digest(2), 64, 10));
    assert_ne!(baseline, challenge(digest(1), digest(3), 64, 10));
    assert_ne!(baseline, challenge(digest(1), digest(2), 128, 10));
    assert_ne!(baseline, challenge(digest(1), digest(2), 64, 11));
    assert!(baseline.coefficients()[1..].iter().any(|value| *value != 0));
}

#[test]
fn mixed_trace_uses_all_extension_coefficients_and_checks_column_shape() {
    let columns = vec![vec![2, 3], vec![5, 7]];
    let coefficients = [
        GoldilocksFp4V1::new([11, 13, 17, 19]).unwrap(),
        GoldilocksFp4V1::new([23, 29, 31, 37]).unwrap(),
    ];
    let actual = combine_lde_columns_v1(&columns, &coefficients).unwrap();
    for index in 0..2 {
        let expected = core::array::from_fn(|lane| {
            add_mod(
                mul_mod(columns[0][index], coefficients[0].coefficients()[lane]),
                mul_mod(columns[1][index], coefficients[1].coefficients()[lane]),
            )
        });
        assert_eq!(actual[index].coefficients(), expected);
    }
    assert!(combine_lde_columns_v1(&[], &[]).is_err());
    assert!(combine_lde_columns_v1(&columns, &coefficients[..1]).is_err());
    assert!(combine_lde_columns_v1(&[vec![1], vec![2, 3]], &coefficients).is_err());
}

#[test]
fn extension_oracle_hashes_bind_every_coefficient_and_reject_noncanonical_values() {
    let value = GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap();
    let leaf = hash_lde_chunk_fp4(0, &[value]).unwrap();
    assert_eq!(hash_lde_leaves_fp4(&[value], 2).unwrap(), vec![leaf]);
    assert_ne!(leaf, hash_lde_chunk_fp4(1, &[value]).unwrap());
    assert_ne!(leaf, hash_air_composition_leaf(0, value).unwrap());
    for lane in 0..4 {
        let mut coefficients = value.coefficients();
        coefficients[lane] += 1;
        assert_ne!(
            leaf,
            hash_lde_chunk_fp4(0, &[GoldilocksFp4V1::new(coefficients).unwrap()]).unwrap()
        );
        coefficients[lane] = GOLDILOCKS_MODULUS;
        let malformed = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
        assert!(matches!(
            hash_lde_chunk_fp4(0, &[malformed]),
            Err(Error::NonCanonicalGoldilocksElement { .. })
        ));
        assert!(hash_air_composition_leaf(0, malformed).is_err());
    }
    assert!(hash_lde_leaves_fp4(&[value], 8).is_err());
}

#[test]
fn air_row_hash_batches_match_scalar_rows_indices_and_domains() {
    for row_count in [1, 31, 32, 33, 65] {
        let columns: Vec<Vec<u64>> = (0..5)
            .map(|column| {
                (0..row_count)
                    .map(|row| {
                        if column == 0 && row % 7 == 0 {
                            GOLDILOCKS_MODULUS - 1
                        } else {
                            (column * 100 + row) as u64
                        }
                    })
                    .collect()
            })
            .collect();
        let expected: Vec<_> = (0..row_count)
            .map(|index| {
                let row: Vec<_> = columns.iter().map(|column| column[index]).collect();
                let digest = hash_air_trace_row(index, &row).unwrap();
                assert_ne!(digest, hash_air_trace_row(index + 1, &row).unwrap());
                assert_ne!(digest, hash_lde_chunk(index, &row).unwrap());
                digest
            })
            .collect();
        for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
            assert_eq!(
                hash_air_trace_rows_with_mode(&columns, mode).unwrap(),
                expected,
                "row count {row_count}, requested mode {mode:?}"
            );
        }
    }
}

#[test]
fn air_row_hash_batches_check_shapes_before_hashing() {
    for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
        assert!(hash_air_trace_rows_with_mode(&[], mode).unwrap().is_empty());
        assert!(
            hash_air_trace_rows_with_mode(&[vec![], vec![]], mode)
                .unwrap()
                .is_empty()
        );
        for columns in [
            vec![vec![GOLDILOCKS_MODULUS], vec![]],
            vec![vec![], vec![GOLDILOCKS_MODULUS]],
            vec![vec![0; 64], vec![GOLDILOCKS_MODULUS; 63]],
        ] {
            assert!(matches!(
                hash_air_trace_rows_with_mode(&columns, mode),
                Err(Error::AirOpeningMismatch { index: 0 })
            ));
        }
    }
}

#[test]
fn air_row_hash_batches_return_lowest_error_row_across_worker_schedules() {
    let mut columns = vec![vec![7; 65], vec![11; 65], vec![13; 65]];
    columns[2][17] = GOLDILOCKS_MODULUS;
    columns[0][31] = u64::MAX;
    columns[1][64] = GOLDILOCKS_MODULUS;
    let sequential: Vec<_> = columns.iter().map(|column| column[..31].to_vec()).collect();
    assert!(matches!(
        hash_air_trace_rows_with_mode(&sequential, ExecutionMode::Cpu),
        Err(Error::NonCanonicalGoldilocksElement {
            context: "native_stark_digest_input",
            indices,
        }) if indices == vec![17]
    ));
    for workers in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .expect("bounded hash regression worker pool");
        for iteration in 0..4 {
            let result =
                pool.install(|| hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu));
            assert!(
                matches!(result, Err(Error::NonCanonicalGoldilocksElement {
                    context: "native_stark_digest_input",
                    indices,
                }) if indices == vec![17]),
                "lowest row must win with {workers} workers on iteration {iteration}"
            );
        }
    }
}

#[test]
fn air_row_prefix_batches_preserve_wide_rows_across_worker_counts() {
    let pools = [1, 2, 4].map(|workers| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .expect("bounded AIR row prefix worker pool")
    });
    for width in [1, 7, 342] {
        for row_count in [1, 15, 16, 31, 32, 33, 65] {
            let columns: Vec<Vec<u64>> = (0..width)
                .map(|column| {
                    (0..row_count)
                        .map(|row| match (column + row) % 7 {
                            0 => 0,
                            1 => GOLDILOCKS_MODULUS - 1,
                            2 => GOLDILOCKS_MODULUS - 2,
                            _ => (column * 71 + row * 37) as u64,
                        })
                        .collect()
                })
                .collect();
            let expected: Vec<_> = (0..row_count)
                .map(|index| {
                    let row: Vec<_> = columns.iter().map(|column| column[index]).collect();
                    hash_air_trace_row(index, &row).unwrap()
                })
                .collect();
            for pool in &pools {
                for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
                    assert_eq!(
                        pool.install(|| hash_air_trace_rows_with_mode(&columns, mode))
                            .unwrap(),
                        expected,
                        "width {width}, rows {row_count}, workers {}, requested mode {mode:?}",
                        pool.current_num_threads()
                    );
                }
            }
        }
    }
}

#[test]
fn air_row_prefix_batches_reject_every_noncanonical_wide_column_like_scalar() {
    let mut columns = vec![vec![7]; 342];
    let mut row = vec![7; 342];
    for column in 0..columns.len() {
        for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
            columns[column][0] = invalid;
            row[column] = invalid;
            for result in [
                hash_air_trace_row(0, &row),
                hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu).map(|leaves| leaves[0]),
            ] {
                assert!(matches!(
                    result,
                    Err(Error::NonCanonicalGoldilocksElement {
                        context: "native_stark_digest_input",
                        indices,
                    }) if indices == vec![0]
                ));
            }
        }
        columns[column][0] = 7;
        row[column] = 7;
    }
    assert_eq!(
        hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu).unwrap(),
        vec![hash_air_trace_row(0, &row).unwrap()]
    );
}

#[test]
#[ignore = "bounded CPU timing diagnostic; no production speed assertion"]
fn air_row_prefix_microdiagnostic() {
    const ROWS: usize = 512;
    const WIDTH: usize = 342;
    let columns: Vec<Vec<u64>> = (0..WIDTH)
        .map(|column| {
            (0..ROWS)
                .map(|row| (column * 71 + row * 37) as u64)
                .collect()
        })
        .collect();
    let mut row = vec![0; WIDTH];
    let started = std::time::Instant::now();
    let expected: Vec<_> = (0..ROWS)
        .map(|index| {
            for (value, column) in row.iter_mut().zip(&columns) {
                *value = column[index];
            }
            hash_air_trace_row(index, &row).unwrap()
        })
        .collect();
    let canonical_elapsed = started.elapsed();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .expect("bounded AIR row prefix diagnostic pool");
        let started = std::time::Instant::now();
        let actual = pool
            .install(|| hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu))
            .unwrap();
        let prefix_elapsed = started.elapsed();
        assert_eq!(actual, expected);
        eprintln!(
            "air_row_prefix_rows={ROWS}; width={WIDTH}; workers={workers}; canonical_scalar={canonical_elapsed:?}; prefix_batch={prefix_elapsed:?}; canonical_leaf_parity=true"
        );
    }
}

#[test]
fn open_queries_rejects_out_of_range() {
    let err = open_queries(&[10u64, 11u64], &[2]).expect_err("out-of-range query");
    assert!(matches!(
        err,
        Error::QueryIndexOutOfRange { index: 2, len: 2 }
    ));
}
#[test]
fn merkle_paths_rejects_out_of_range_indices() {
    let evaluations = vec![1u64, 2, 3, 4];
    let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
    let err =
        merkle_paths_for_queries(&leaves, &[4], 2, evaluations.len()).expect_err("out of range");
    assert!(matches!(
        err,
        Error::QueryIndexOutOfRange { index: 4, len: 4 }
    ));
}
#[test]
fn merkle_paths_verify_against_lde_root_for_single_leaf() {
    let evaluations = vec![42u64];
    let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
    let root = lde_merkle_root(&leaves);
    let paths = merkle_paths_for_queries(&leaves, &[0], 2, evaluations.len()).expect("query path");
    let chunks = open_query_chunks(&evaluations, &[0], 2).expect("query chunk");
    let leaf = hash_lde_chunk(0, &chunks[0]).expect("leaf hash");
    assert!(verify_merkle_path(root, leaf, 0, &paths[0]).expect("path verifies"));
    assert!(!verify_merkle_path(leaf, leaf, 0, &[]).expect("empty path is noncanonical"));
}
#[test]
fn merkle_paths_verify_against_lde_root_for_odd_leaf_count() {
    let chunk_size = lde_chunk_size(2).expect("binary FRI chunk size");
    let evaluations = (0..(chunk_size * 3 - 1))
        .map(|idx| u64::try_from(idx).expect("index fits u64"))
        .collect::<Vec<_>>();
    let query_index = evaluations.len() - 1;
    let leaf_index = query_index / chunk_size;
    let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
    let root = lde_merkle_root(&leaves);
    let paths =
        merkle_paths_for_queries(&leaves, &[query_index], 2, evaluations.len()).expect("path");
    let chunks = open_query_chunks(&evaluations, &[query_index], 2).expect("chunk");
    let leaf = hash_lde_chunk(leaf_index, &chunks[0]).expect("leaf hash");
    assert!(verify_merkle_path(root, leaf, leaf_index, &paths[0]).expect("path verifies"));
}
#[test]
fn merkle_path_rejects_indices_with_bits_above_the_tree_depth() {
    let evaluations = (0u64..256).collect::<Vec<_>>();
    let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
    let root = lde_merkle_root(&leaves);
    let paths = merkle_paths_for_queries(&leaves, &[0], 2, evaluations.len()).expect("query path");
    let chunks = open_query_chunks(&evaluations, &[0], 2).expect("query chunk");
    let leaf = hash_lde_chunk(0, &chunks[0]).expect("leaf hash");
    let aliased_index = 1usize
        .checked_shl(u32::try_from(paths[0].len()).expect("path depth fits u32"))
        .expect("test path depth fits usize");

    assert!(verify_merkle_path(root, leaf, 0, &paths[0]).expect("path verifies"));
    assert!(
        !verify_merkle_path(root, leaf, aliased_index, &paths[0])
            .expect("high-bit alias is rejected")
    );
}

#[test]
fn lde_helpers_reject_non_binary_fri_arity() {
    let evaluations = [1u64, 2, 3, 4];
    let err = hash_lde_leaves(&evaluations, 8).expect_err("legacy arity must fail");
    assert!(matches!(err, Error::FriArity(8)));
    let err = open_query_chunks(&evaluations, &[0], 8).expect_err("legacy arity must fail");
    assert!(matches!(err, Error::FriArity(8)));
    let err = merkle_paths_for_queries(&[], &[], 8, evaluations.len())
        .expect_err("legacy arity must fail before empty-query handling");
    assert!(matches!(err, Error::FriArity(8)));
}
#[test]
fn air_constrains_every_metadata_commitment_limb_to_be_stable() {
    let mut batch = sample_batch(2);
    batch
        .metadata
        .insert("axt_metadata_binding".to_owned(), vec![0x5a; 32]);
    let trace = build_trace(&batch).expect("trace");
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let current = trace
        .columns
        .iter()
        .map(|column| column.values[0])
        .collect::<Vec<_>>();
    let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
        .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
        .collect::<Vec<_>>();
    assert_eq!(
        air_composition_value_for_rows(&column_names, &current, &current, &alphas)
            .expect("valid row composition"),
        0
    );
    for limb in 0..crate::trace::METADATA_COMMITMENT_LIMBS {
        let name = format!("metadata_hash_limb_{limb}");
        let index = column_names
            .iter()
            .position(|column| column == &name)
            .expect("metadata commitment limb column");
        let mut next = current.clone();
        next[index] = next[index].wrapping_add(1);
        assert_ne!(
            air_composition_value_for_rows(&column_names, &current, &next, &alphas)
                .expect("mutated row composition"),
            0,
            "AIR must reject instability in {name}"
        );
    }
}
#[test]
fn integer_air_binding_rejects_lengths_auxiliary_and_trailing_limb_mutations() {
    let mut trace = build_trace(&sample_batch(2)).unwrap();
    let witness = transfer_integer_air::TransferIntegerWitness::from_balances(1, 2);
    for (name, value) in transfer_integer_air::auxiliary_column_names()
        .into_iter()
        .zip(witness.auxiliary_values())
    {
        trace.columns.push(crate::TraceColumn {
            name,
            values: vec![value, 0],
        });
    }
    trace.columns.push(crate::TraceColumn {
        name: "value_old_limb_2".into(),
        values: vec![0, 0],
    });
    for (name, value) in [
        ("s_transfer", 1),
        ("s_meta_set", 0),
        ("value_old_limb_0", 1),
        ("value_new_limb_0", 2),
        ("delta", 1),
    ] {
        trace
            .columns
            .iter_mut()
            .find(|column| column.name == name)
            .unwrap()
            .values[0] = value;
    }
    ensure_base_trace_constraints(&trace).expect("exact integer trace relation");
    let names: Vec<_> = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect();
    assert_eq!(
        air_composition_alpha_count(&names),
        AIR_COMPOSITION_ALPHA_COUNT + 1
    );
    for (name, bad_value) in [
        ("value_old_len", 7),
        ("value_new_len", 9),
        ("value_old_limb_2", 1),
        ("transfer_old_bit_0", 2),
        ("transfer_carry_32", 1),
        ("transfer_is_debit", 2),
    ] {
        let mut invalid = trace.clone();
        invalid
            .columns
            .iter_mut()
            .find(|column| column.name == name)
            .unwrap()
            .values[0] = bad_value;
        assert!(
            matches!(
                ensure_base_trace_constraints(&invalid),
                Err(Error::AirConstraintMismatch { index: 0 })
            ),
            "{name}"
        );
    }
    trace
        .columns
        .retain(|column| column.name != "transfer_old_bit_0");
    assert!(
        matches!(ensure_base_trace_constraints(&trace), Err(Error::MissingColumn(name)) if name == "transfer_old_bit_0")
    );
}

#[test]
fn extension_quotient_combination_matches_each_base_coefficient() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let trace = build_trace(&sample_batch(3)).unwrap();
    let names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let mut columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
    // Activate multiple otherwise valid residues at one coset point, so this
    // comparison cannot pass merely because every tested quotient is zero.
    columns[0][3] = add_mod(columns[0][3], 7);
    let alphas = (0..air_composition_alpha_count(&names))
        .map(|index| {
            GoldilocksFp4V1::new(core::array::from_fn(|lane| 1 + (index * 4 + lane) as u64))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let combined = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
    let domain = AirQuotientDomain::new(&params, columns[0].len()).unwrap();
    for lane in 0..4 {
        let base_alphas = alphas
            .iter()
            .map(|alpha| alpha.coefficients()[lane])
            .collect::<Vec<_>>();
        let base_values = air_quotient_values(&params, &names, &columns, &base_alphas).unwrap();
        assert!(base_values.iter().any(|value| *value != 0));
        for (index, &value) in combined.iter().enumerate() {
            assert_eq!(value.coefficients()[lane], base_values[index]);
            let next = (index + params.fri.blowup_factor as usize) % combined.len();
            assert_eq!(
                value,
                air_quotient_value_for_rows(
                    &names,
                    &air_row_at(&columns, index).unwrap(),
                    &air_row_at(&columns, next).unwrap(),
                    &alphas,
                    domain.weights_at(index).unwrap()
                )
                .unwrap()
            );
        }
    }
}

#[test]
fn quotient_composition_matches_sampled_rows_and_excludes_only_the_final_transition() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let trace = build_trace(&sample_batch(3)).expect("padded trace");
    assert_eq!(trace.padded_len, 4);
    let names: Vec<_> = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect();
    let columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
    let mut alphas = vec![0; AIR_COMPOSITION_ALPHA_COUNT];
    alphas[AIR_BOOLEAN_RESIDUE_COUNT + 2] = 1;
    let domain = AirQuotientDomain::new(&params, columns[0].len()).expect("disjoint coset");
    let values = air_quotient_values(&params, &names, &columns, &alphas).expect("quotients");
    let next_step = params.fri.blowup_factor as usize;
    for (index, value) in values.iter().enumerate() {
        let next = (index + next_step) % values.len();
        assert_eq!(
            *value,
            air_quotient_value_for_rows(
                &names,
                &air_row_at(&columns, index).unwrap(),
                &air_row_at(&columns, next).unwrap(),
                &alphas,
                domain.weights_at(index).unwrap(),
            )
            .unwrap()
        );
    }
    let fri_domain = FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        values.len(),
        params.omega_coset,
    )
    .unwrap();
    let embedded: Vec<_> = values
        .into_iter()
        .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
        .collect();
    assert!(
        fri_domain
            .evaluations_have_degree_below(&embedded, trace.padded_len)
            .unwrap()
    );

    // The padded final row is excluded, but an inactive interior row followed
    // by an active row must still produce a quotient above the allowed degree.
    let mut invalid = trace.clone();
    invalid
        .columns
        .iter_mut()
        .find(|column| column.name == "s_active")
        .unwrap()
        .values[1] = 0;
    let columns = derive_polynomial_data(&invalid, &Planner::new(&params)).into_lde_columns();
    let values = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
    let embedded: Vec<_> = values
        .into_iter()
        .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
        .collect();
    assert!(
        !fri_domain
            .evaluations_have_degree_below(&embedded, 2 * trace.padded_len)
            .unwrap()
    );
}

#[test]
fn all_row_quotient_rejects_non_boolean_selector_degree() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let mut trace = build_trace(&sample_batch(3)).unwrap();
    let names: Vec<_> = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect();
    let mut alphas = vec![0; AIR_COMPOSITION_ALPHA_COUNT];
    alphas[0] = 1;
    for valid in [true, false] {
        if !valid {
            trace
                .columns
                .iter_mut()
                .find(|column| column.name == "s_active")
                .unwrap()
                .values[0] = 2;
        }
        let columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
        let values = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            values.len(),
            params.omega_coset,
        )
        .unwrap();
        let values: Vec<_> = values
            .into_iter()
            .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
            .collect();
        assert_eq!(
            domain
                .evaluations_have_degree_below(&values, 2 * trace.padded_len)
                .unwrap(),
            valid
        );
    }
}

#[test]
fn air_composition_columnar_pass_matches_row_helper() {
    let trace = build_trace(&sample_batch(5)).expect("trace");
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let mut columns = trace
        .columns
        .iter()
        .map(|column| column.values.clone())
        .collect::<Vec<_>>();
    let active = column_names
        .iter()
        .position(|column| column == "s_active")
        .expect("active selector column");
    let metadata = column_names
        .iter()
        .position(|column| column == "metadata_hash_limb_3")
        .expect("metadata commitment column");
    columns[active][0] = 2;
    columns[metadata][1] = add_mod(columns[metadata][1], FIELD_ONE);
    let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
        .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
        .collect::<Vec<_>>();
    let next_step = 1;
    let expected = (0..trace.padded_len)
        .map(|row_index| {
            let current = air_row_at(&columns, row_index)?;
            let next = air_row_at(&columns, (row_index + next_step) % trace.padded_len)?;
            air_composition_value_for_rows(&column_names, &current, &next, &alphas)
        })
        .collect::<Result<Vec<_>>>()
        .expect("row-wise AIR composition");

    assert_eq!(
        air_composition_values(&column_names, &columns, &alphas, next_step)
            .expect("columnar AIR composition"),
        expected
    );
    assert!(expected.iter().any(|value| *value != 0));

    let large_step = usize::MAX;
    assert_eq!(
        air_composition_values(&column_names, &columns, &alphas, large_step)
            .expect("large next-step offset"),
        air_composition_values(
            &column_names,
            &columns,
            &alphas,
            large_step % trace.padded_len,
        )
        .expect("reduced next-step offset")
    );
}
#[test]
fn air_column_layout_preserves_schema_error_order() {
    let trace = build_trace(&sample_batch(2)).expect("trace");
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let columns = trace
        .columns
        .iter()
        .map(|column| column.values.clone())
        .collect::<Vec<_>>();
    let alphas = vec![FIELD_ONE; AIR_COMPOSITION_ALPHA_COUNT];
    let missing_index = column_names
        .iter()
        .position(|column| column == "s_transfer")
        .expect("transfer selector column");
    let mut missing_names = column_names.clone();
    missing_names.remove(missing_index);
    let mut missing_columns = columns.clone();
    missing_columns.remove(missing_index);

    let err = air_composition_values(&missing_names, &missing_columns, &alphas, 1)
        .expect_err("missing selector must reject the schema");
    assert!(matches!(err, Error::MissingColumn(name) if name == "s_transfer"));

    let err = air_composition_values(&missing_names, &missing_columns, &[], 1)
        .expect_err("challenge validation precedes schema validation");
    assert!(matches!(
        err,
        Error::AirChallengeCountMismatch {
            expected: AIR_COMPOSITION_ALPHA_COUNT,
            actual: 0
        }
    ));

    let mut short_names = column_names.clone();
    short_names.pop();
    let err = air_composition_values(&short_names, &columns, &alphas, 1)
        .expect_err("row width validation precedes schema validation");
    assert!(matches!(
        err,
        Error::AirOpeningMismatch { index } if index == columns.len()
    ));

    let mut missing_trace = trace;
    missing_trace.columns.remove(missing_index);
    let err = ensure_base_trace_constraints(&missing_trace)
        .expect_err("base trace pass must validate its layout");
    assert!(matches!(err, Error::MissingColumn(name) if name == "s_transfer"));

    assert!(
        air_composition_values(&[], &[Vec::new()], &[], 0)
            .expect("empty domains retain their validation behavior")
            .is_empty()
    );
}
#[test]
fn base_trace_constraint_check_rejects_non_boolean_selector() {
    let mut trace = build_trace(&sample_batch(2)).expect("trace");
    let selector = trace
        .columns
        .iter_mut()
        .find(|column| column.name == "s_active")
        .expect("active selector column");
    selector.values[0] = 2;
    let err = ensure_base_trace_constraints(&trace).unwrap_err();
    assert!(matches!(err, Error::AirConstraintMismatch { index: 0 }));
}
#[test]
fn base_trace_delta_constraint_reconstructs_every_packed_value_limb() {
    let mut batch =
        TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
    let before = (1u64 << 56) - 2;
    let after = (1u64 << 56) + 3;
    batch.push(StateTransition::new(
        b"asset/xor/account-multi-limb".to_vec(),
        before.to_le_bytes().to_vec(),
        after.to_le_bytes().to_vec(),
        OperationKind::MetaSet,
    ));
    let mut trace = build_trace(&batch).expect("trace");
    trace
        .columns
        .iter_mut()
        .find(|column| column.name == "s_meta_set")
        .expect("metadata selector")
        .values[0] = 0;
    trace
        .columns
        .iter_mut()
        .find(|column| column.name == "s_transfer")
        .expect("transfer selector")
        .values[0] = 1;
    trace
        .columns
        .iter_mut()
        .find(|column| column.name == "delta")
        .expect("delta column")
        .values[0] = sub_mod(after, before);
    let witness = transfer_integer_air::TransferIntegerWitness::from_balances(before, after);
    for (name, value) in transfer_integer_air::auxiliary_column_names()
        .into_iter()
        .zip(witness.auxiliary_values())
    {
        trace.columns.push(crate::TraceColumn {
            name,
            values: vec![value],
        });
    }
    ensure_base_trace_constraints(&trace).expect("multi-limb delta must satisfy the AIR");
}
#[test]
fn air_independent_challenges_prevent_same_parity_residue_cancellation() {
    let trace = build_trace(&sample_batch(2)).expect("trace");
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let current = trace
        .columns
        .iter()
        .map(|column| column.values[0])
        .collect::<Vec<_>>();
    let first = column_names
        .iter()
        .position(|column| column == "metadata_hash_limb_0")
        .expect("first metadata commitment limb");
    let third = column_names
        .iter()
        .position(|column| column == "metadata_hash_limb_2")
        .expect("third metadata commitment limb");
    let mut next = current.clone();
    next[first] = sub_mod(current[first], FIELD_ONE);
    next[third] = add_mod(current[third], FIELD_ONE);

    let legacy_same_parity_sum = add_mod(
        mul_mod(3, FIELD_ONE),
        mul_mod(3, GOLDILOCKS_MODULUS - FIELD_ONE),
    );
    assert_eq!(legacy_same_parity_sum, 0);
    let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
        .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
        .collect::<Vec<_>>();
    assert_ne!(
        air_composition_value_for_rows(&column_names, &current, &next, &alphas)
            .expect("AIR composition"),
        0,
        "distinct coefficients must expose equal-and-opposite residues at old reuse offsets"
    );
}
#[test]
fn native_stark_lde_digests_are_byte_identical_across_execution_modes() {
    let evaluations = (0_u64..257).collect::<Vec<_>>();
    let scalar = hash_lde_leaves_with_mode(&evaluations, 2, ExecutionMode::Cpu)
        .expect("scalar native-STARK LDE digests");
    let accelerated = hash_lde_leaves_with_mode(&evaluations, 2, ExecutionMode::Gpu)
        .expect("accelerated native-STARK LDE digests");
    assert_eq!(accelerated, scalar);
}
#[cfg(feature = "fastpq-gpu")]
#[test]
fn native_stark_fri_digests_are_byte_identical_for_mixed_layer_shapes() {
    for length in [2_usize, 4, 16, 256] {
        let values = (0..length)
            .map(|value| {
                GoldilocksFp4V1::new([u64::try_from(value).expect("test value fits u64"), 1, 2, 3])
                    .expect("canonical Fp4 test value")
            })
            .collect::<Vec<_>>();
        let scalar = hash_fri_leaves_with_mode(7, &values, 2, ExecutionMode::Cpu)
            .expect("scalar native-STARK FRI digests");
        let accelerated = hash_fri_leaves_with_mode(7, &values, 2, ExecutionMode::Gpu)
            .expect("accelerated native-STARK FRI digests");
        assert_eq!(accelerated, scalar, "FRI layer length {length}");
    }
}
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "requires actual Metal execution; no device skip is accepted"]
fn native_merkle_metal_levels_roots_and_chunk_boundaries_match_cpu() {
    use crate::digest_executor::{
        DigestExecutionV1, execute_bounded_digest384_frames_v1, execute_digest384_frames_v1,
    };
    let device = DigestExecutionV1::Device(crate::digest384_gpu::Digest384GpuBackendV1::Metal);
    for role in [
        MerkleTreeRoleV1::Trace,
        MerkleTreeRoleV1::Lde,
        MerkleTreeRoleV1::AirTrace,
        MerkleTreeRoleV1::AirComposition,
        MerkleTreeRoleV1::Fri(0),
        MerkleTreeRoleV1::Fri(7),
    ] {
        for len in [0, 1, 3, 5, 17] {
            let leaves: Vec<_> = (0..len)
                .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
                .collect();
            let cpu = build_merkle_levels_with_execution_v1(&leaves, role, DigestExecutionV1::Cpu)
                .unwrap();
            let mut dispatch_sizes = Vec::new();
            let metal = build_merkle_levels_with_executor_v1(&leaves, role, &mut |frames| {
                execute_bounded_digest384_frames_v1(
                    frames,
                    2,
                    frames[0].word_count() * 2,
                    &mut |chunk| {
                        dispatch_sizes.push(chunk.len());
                        execute_digest384_frames_v1(chunk, device)
                    },
                )
            })
            .expect("actual bounded Metal node dispatch");
            assert_eq!(metal, cpu, "role {role:?}, leaves {len}");
            if len == 17 {
                assert!(dispatch_sizes.len() > cpu.len());
                assert!(dispatch_sizes.contains(&1));
            }
            assert_eq!(
                merkle_root_with_execution_v1(&leaves, role, device).unwrap(),
                merkle_root_with_execution_v1(&leaves, role, DigestExecutionV1::Cpu).unwrap()
            );
        }
    }
    let leaves =
        hash_lde_leaves_with_mode(&(0_u64..513).collect::<Vec<_>>(), 2, ExecutionMode::Cpu)
            .unwrap();
    assert_eq!(
        merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Lde, device).unwrap(),
        merkle_root_with_mode(&leaves, MerkleTreeRoleV1::Lde, ExecutionMode::Cpu).unwrap()
    );
    assert_ne!(
        merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Fri(0), device).unwrap(),
        merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Fri(7), device).unwrap()
    );
    assert!(!preflight_native_v1_gpu_backend());
}

#[test]
fn native_merkle_device_failure_aborts_tree_without_cpu_substitution() {
    use crate::digest_executor::{
        DigestExecutionV1, execute_bounded_digest384_frames_v1, execute_digest384_frames_v1,
    };
    let leaves: Vec<_> = (0..17)
        .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
        .collect();
    let mut calls = 0;
    let result =
        build_merkle_levels_with_executor_v1(&leaves, MerkleTreeRoleV1::Fri(7), &mut |frames| {
            execute_bounded_digest384_frames_v1(frames, 2, 1000, &mut |chunk| {
                calls += 1;
                if calls == 2 {
                    Err(Error::NativeDigestExecution {
                        details: "injected Merkle device failure".into(),
                    })
                } else {
                    execute_digest384_frames_v1(chunk, DigestExecutionV1::Cpu)
                }
            })
        });
    assert!(
        matches!(result, Err(Error::NativeDigestExecution { details }) if details == "injected Merkle device failure")
    );
    assert_eq!(calls, 2);
}
#[test]
fn indexed_prefix_hashes_preserve_all_domain_coordinates_and_payload_framing() {
    for role in [
        MerkleTreeRoleV1::Trace,
        MerkleTreeRoleV1::Lde,
        MerkleTreeRoleV1::AirTrace,
        MerkleTreeRoleV1::AirComposition,
        MerkleTreeRoleV1::Fri(17),
    ] {
        for phase in [MERKLE_LEAF_PHASE_V1, MERKLE_NODE_PHASE_V1] {
            for level in [0, 1, 19] {
                let prefix =
                    digest_domain_prefix_v1(role.role(), phase, level, role.counter()).unwrap();
                for index in [0, 1, 7, 1 << 20, usize::MAX] {
                    for fields in [vec![], vec![&[][..]], vec![&[11; 48][..], &[29; 48][..]]] {
                        assert_eq!(
                            hash_at_prefix_v1(&prefix, index, &fields).unwrap(),
                            hash_bytes_v1(
                                role.role(),
                                phase,
                                level,
                                index,
                                role.counter(),
                                &fields
                            )
                            .unwrap(),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn parallel_fri_prefix_leaves_match_scalar_strided_groups() {
    let pools = [1, 4].map(|workers| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
    });
    for round in [0, 1, 17] {
        for count in [0, 1, 2, 62, 64, 128, 256] {
            let values: Vec<_> = (0..count)
                .map(|index| {
                    GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                        (index * 43 + lane * 17) as u64
                    }))
                    .unwrap()
                })
                .collect();
            let expected = if count == 0 {
                Vec::new()
            } else {
                let arity = 2.min(count);
                let output = count / arity;
                (0..output)
                    .map(|index| {
                        let group: Vec<_> = (0..arity)
                            .map(|position| values[index + position * output])
                            .collect();
                        hash_fri_chunk(round, index, &group).unwrap()
                    })
                    .collect()
            };
            for pool in &pools {
                for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
                    assert_eq!(
                        pool.install(|| hash_fri_leaves_with_mode(round, &values, 2, mode))
                            .unwrap(),
                        expected
                    );
                }
            }
        }
    }
    assert!(matches!(
        hash_fri_leaves_with_mode(0, &[GoldilocksFp4V1::ZERO; 3], 2, ExecutionMode::Cpu),
        Err(Error::FriDomainSize {
            length: 3,
            arity: 2
        })
    ));
    assert!(matches!(
        hash_fri_leaves_with_mode(0, &[], 4, ExecutionMode::Cpu),
        Err(Error::FriArity(4))
    ));
}

#[test]
fn parallel_single_fp4_leaves_match_canonical_hashes_and_first_error() {
    let pools = [1, 4].map(|workers| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
    });
    for role in [LDE_COMMITMENT_ROLE_V1, AIR_COMPOSITION_COMMITMENT_ROLE_V1] {
        for count in [0, 1, 31, 32, 65, 129] {
            let values: Vec<_> = (0..count)
                .map(|index| {
                    GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                        (index * 73 + lane * 11) as u64
                    }))
                    .unwrap()
                })
                .collect();
            let expected: Vec<_> = values
                .iter()
                .enumerate()
                .map(|(index, value)| hash_fp4_values_v1(role, 0, index, &[*value]).unwrap())
                .collect();
            for pool in &pools {
                assert_eq!(
                    pool.install(|| hash_fp4_single_leaves_with_role(role, &values))
                        .unwrap(),
                    expected
                );
            }
        }
    }
    for lane in 0..4 {
        let mut coefficients = [0; 4];
        coefficients[lane] = GOLDILOCKS_MODULUS;
        let malformed = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
        let mut values = vec![GoldilocksFp4V1::ZERO; 65];
        values[3] = malformed;
        values[47] = malformed;
        for pool in &pools {
            assert!(matches!(
                pool.install(|| hash_fp4_single_leaves_with_role(LDE_COMMITMENT_ROLE_V1, &values)),
                Err(Error::NonCanonicalGoldilocksElement { context: "native_stark_fp4_digest_input", indices })
                    if indices == vec![3, 0, lane]
            ));
        }
    }
}

#[test]
fn parallel_merkle_levels_match_scalar_trees_across_roles_padding_and_worker_counts() {
    fn scalar_levels(
        leaves: &[GoldilocksDigest384V1],
        role: MerkleTreeRoleV1,
    ) -> Vec<Vec<GoldilocksDigest384V1>> {
        if leaves.is_empty() {
            return Vec::new();
        }
        let mut current = leaves.to_vec();
        let mut levels = Vec::new();
        loop {
            if current.len() % 2 == 1 {
                current.push(*current.last().unwrap());
            }
            levels.push(current.clone());
            let mut next = Vec::with_capacity(current.len() / 2);
            for parent in 0..current.len() / 2 {
                next.push(
                    merkle_node_hash(
                        role,
                        levels.len(),
                        parent,
                        current[2 * parent],
                        current[2 * parent + 1],
                    )
                    .unwrap(),
                );
            }
            if next.len() == 1 {
                levels.push(next);
                return levels;
            }
            current = next;
        }
    }
    let pools = [1, 4].map(|workers| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
    });
    for role in [
        MerkleTreeRoleV1::Trace,
        MerkleTreeRoleV1::Lde,
        MerkleTreeRoleV1::AirTrace,
        MerkleTreeRoleV1::AirComposition,
        MerkleTreeRoleV1::Fri(0),
        MerkleTreeRoleV1::Fri(17),
    ] {
        for count in [0, 1, 3, 31, 63, 64, 65, 129] {
            let leaves = (0..count)
                .map(|index| {
                    GoldilocksDigest384V1::new(core::array::from_fn(|lane| {
                        1 + 7 * index as u64 + 13 * lane as u64
                    }))
                    .unwrap()
                })
                .collect::<Vec<_>>();
            let expected = scalar_levels(&leaves, role);
            for pool in &pools {
                for mode in [ExecutionMode::Cpu, ExecutionMode::Auto] {
                    let actual = pool
                        .install(|| build_merkle_levels_with_mode(&leaves, role, mode))
                        .unwrap();
                    assert_eq!(
                        actual, expected,
                        "role={role:?}; leaves={count}; mode={mode:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn fold_with_fri_emits_layers_and_betas() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        "fastpq-state-transition-stark-v1",
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let evaluations = (1u64..=16).collect::<Vec<_>>();
    let (layers, betas) = fold_with_fri(
        &evaluations,
        params.fri.arity,
        params.fri.max_reductions,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect("fri folding");
    assert!(!betas.is_empty());
    assert_eq!(layers.len(), betas.len() + 1);
    assert!(
        betas.len()
            <= usize::try_from(params.fri.max_reductions).expect("max reductions fits usize")
    );
    assert!(
        layers
            .iter()
            .all(|layer| *layer != GoldilocksDigest384V1::default())
    );
}
#[test]
fn fold_with_fri_rejects_invalid_arity() {
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        "fastpq-state-transition-stark-v1",
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let err = super::fold_with_fri(
        &[1, 2, 3],
        4,
        1,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect_err("invalid arity");
    assert!(matches!(err, super::Error::FriArity(4)));
}
fn sampler_test_transcript() -> Transcript {
    Transcript::initialise(
        &crate::proof::PublicIO::default(),
        fastpq_isi::FASTPQ_FINAL_V1_ID,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .unwrap()
}

#[test]
fn query_sampler_empty_and_unsupported_shapes_do_not_draw() {
    for (domain_size, target) in [(0, 0), (0, usize::MAX), (usize::MAX, 0)] {
        assert!(
            sample_queries_from(domain_size, target, |_| panic!("empty sampler drew"))
                .unwrap()
                .is_empty()
        );
    }
    let error = sample_queries_from(1024, 513, |_| panic!("oversized sampler drew")).unwrap_err();
    assert!(matches!(
        error,
        Error::VerifierLimitExceeded {
            limit: "max_sampled_queries",
            actual: 513,
            max: 512
        }
    ));
    if let Ok(domain) = usize::try_from(GOLDILOCKS_MODULUS) {
        if let Some(unsupported) = domain.checked_add(1) {
            assert!(matches!(
                sample_queries_from(unsupported, 1, |_| panic!("invalid domain drew")),
                Err(Error::QuerySamplingDomainUnsupported { domain_size }) if domain_size == unsupported
            ));
        }
        // Domain p accepts the entire canonical field, including p-1.
        let mut draws = 0;
        let selected = sample_queries_from(domain, 1, |counter| {
            assert_eq!(counter, 0);
            draws += 1;
            Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1; 6]).unwrap())
        })
        .unwrap();
        assert_eq!(selected, [domain - 1]);
        assert_eq!(draws, 1);
    }
}

#[test]
fn query_sampler_rejected_and_duplicate_sources_exhaust_exactly() {
    for (desired, words, expected_selected) in [(1, [GOLDILOCKS_MODULUS - 1; 6], 0), (2, [0; 6], 1)]
    {
        let mut calls = 0_u32;
        let error = sample_queries_from(32, desired, |counter| {
            assert_eq!(counter, calls);
            calls += 1;
            Ok(GoldilocksDigest384V1::new(words).unwrap())
        })
        .unwrap_err();
        assert!(matches!(error, Error::QuerySamplingExhausted {
            domain_size: 32, requested, selected, draws: 64
        } if requested == desired && selected == expected_selected));
        assert_eq!(calls, 64);
    }
}

#[test]
fn query_sampler_accepts_the_last_allowed_draw_and_never_draws_one_more() {
    for first_new_index_draw in [63, 64] {
        let mut calls = 0_u32;
        let result = sample_queries_from(32, 2, |counter| {
            assert_eq!(counter, calls);
            calls += 1;
            let word = u64::from(counter >= first_new_index_draw);
            Ok(GoldilocksDigest384V1::new([word; 6]).unwrap())
        });
        assert_eq!(calls, 64);
        if first_new_index_draw == 63 {
            assert_eq!(result.unwrap(), [0, 1]);
        } else {
            assert!(matches!(
                result,
                Err(Error::QuerySamplingExhausted {
                    domain_size: 32,
                    requested: 2,
                    selected: 1,
                    draws: 64
                })
            ));
        }
    }
}

#[test]
fn query_sampler_preserves_rejection_lane_order_clamping_and_source_errors() {
    let mut calls = 0;
    // p-1 is rejected for domain 32; 33 and 1 have the same accepted index.
    let selected = sample_queries_from(32, 2, |counter| {
        assert_eq!(counter, 0, "sampler made an unnecessary second draw");
        calls += 1;
        Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1, 33, 1, 2, 7, 9]).unwrap())
    })
    .unwrap();
    assert_eq!(selected, [1, 2]);
    assert_eq!(calls, 1);
    // Large raw targets retain the old min(target, domain) behavior before
    // applying the supported desired-cardinality bound.
    assert_eq!(
        sample_queries_from(5, usize::MAX, |_| {
            Ok(GoldilocksDigest384V1::new([4, 3, 2, 1, 0, 0]).unwrap())
        })
        .unwrap(),
        [0, 1, 2, 3, 4]
    );
    let mut calls = 0;
    let error = sample_queries_from(32, 2, |counter| {
        calls += 1;
        if counter == 2 {
            Err(Error::QuerySamplingTranscriptCounterExhausted)
        } else {
            Ok(GoldilocksDigest384V1::new([0; 6]).unwrap())
        }
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Error::QuerySamplingTranscriptCounterExhausted
    ));
    assert_eq!(calls, 3);
}

#[test]
fn query_sampler_engineering_caps_are_fixed_at_supported_cardinalities() {
    for (desired, expected_draws) in [(1, 64), (8, 64), (136, 1088), (200, 1600), (512, 4096)] {
        let mut calls = 0;
        let error = sample_queries_from(1024, desired, |counter| {
            assert_eq!(counter, calls);
            calls += 1;
            Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1; 6]).unwrap())
        })
        .unwrap_err();
        assert!(matches!(error, Error::QuerySamplingExhausted {
            domain_size: 1024, requested, selected: 0, draws
        } if requested == desired && draws == expected_draws));
        assert_eq!(calls, expected_draws);
    }
}

#[test]
fn query_sampler_matches_legacy_success_and_exact_transcript_state() {
    // A bounded test-only copy of the former algorithm is an independent
    // compatibility oracle. It intentionally has no dependency on the new
    // sampler core/cap and is used only on fixed successful transcripts.
    fn legacy(domain_size: usize, target: usize, transcript: &mut Transcript) -> Vec<usize> {
        let desired = target.min(domain_size);
        let domain = u64::try_from(domain_size).unwrap();
        let mut indices = BTreeSet::new();
        for counter in 0_u32..4096 {
            let tag = format!("{TRANSCRIPT_TAG_QUERY_INDEX}:{counter}");
            let digest = transcript.challenge_digest(&tag);
            let rejection_limit = GOLDILOCKS_MODULUS - GOLDILOCKS_MODULUS % domain;
            for candidate in digest.words() {
                if indices.len() == desired {
                    break;
                }
                if candidate >= rejection_limit {
                    continue;
                }
                indices.insert(usize::try_from(candidate % domain).unwrap());
            }
            if indices.len() == desired {
                return indices.into_iter().collect();
            }
        }
        panic!("fixed legacy sampler fixture exceeded its test budget");
    }
    for (domain, desired) in [(5, 10), (128, 16), (4096, 136), (524_288, 136)] {
        let mut actual_transcript = sampler_test_transcript();
        let mut legacy_transcript = actual_transcript.clone();
        let expected = legacy(domain, desired, &mut legacy_transcript);
        assert_eq!(
            sample_queries(domain, desired, &mut actual_transcript).unwrap(),
            expected
        );
        assert_eq!(actual_transcript.state, legacy_transcript.state);
        assert_eq!(actual_transcript.counter, legacy_transcript.counter);
    }
}

#[test]
fn query_sampler_transcript_counter_errors_without_panic_or_extra_draw() {
    let mut transcript = sampler_test_transcript();
    transcript.counter = u64::MAX;
    let state = transcript.state;
    for (domain, desired) in [(0, 1), (1, 0)] {
        assert!(
            sample_queries(domain, desired, &mut transcript)
                .unwrap()
                .is_empty()
        );
        assert_eq!(transcript.state, state);
        assert_eq!(transcript.counter, u64::MAX);
    }
    assert!(matches!(
        sample_queries(1, 1, &mut transcript),
        Err(Error::QuerySamplingTranscriptCounterExhausted)
    ));
    assert_eq!(transcript.state, state);
    assert_eq!(transcript.counter, u64::MAX);
    transcript.counter = u64::MAX - 1;
    // Domain one guarantees that every canonical digest lane selects zero.
    assert_eq!(sample_queries(1, 1, &mut transcript).unwrap(), [0]);
    assert_eq!(transcript.counter, u64::MAX);
}

#[test]
fn sampled_queries_are_sorted_and_unique() {
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        "fastpq-state-transition-stark-v1",
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let indices = super::sample_queries(128, 16, &mut transcript).unwrap();
    assert_eq!(indices.len(), 16);
    assert!(indices.windows(2).all(|window| window[0] < window[1]));
}
#[test]
fn sampled_queries_cap_at_domain_size() {
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        "fastpq-state-transition-stark-v1",
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let indices = super::sample_queries(5, 10, &mut transcript).unwrap();
    assert_eq!(indices.len(), 5);
    let unique: BTreeSet<_> = indices.iter().copied().collect();
    assert_eq!(unique.len(), indices.len());
    assert!(indices.iter().all(|&idx| idx < 5));
    assert!(indices.windows(2).all(|window| window[0] < window[1]));
}
#[test]
fn low_level_backend_rejects_wide_trace_schema_before_allocation() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let mut batch = TransitionBatch::new(params.name, PublicInputs::default());
    batch.push(StateTransition::new(
        b"wide-value".to_vec(),
        vec![0xA5; (crate::trace::DEFAULT_MAX_TRACE_COLUMNS + 1) * crate::LIMB_BYTES],
        Vec::new(),
        OperationKind::MetaSet,
    ));
    let actual = crate::trace::column_count_for_batch(&batch).expect("schema count");
    assert!(actual > crate::trace::DEFAULT_MAX_TRACE_COLUMNS);

    let backend = StarkBackend::new(BackendConfig::new(params));
    let err = backend
        .prove(&batch, &crate::proof::PublicIO::default(), 1)
        .expect_err("wide trace schema must fail before materialisation");
    assert!(matches!(
        err,
        Error::VerifierLimitExceeded {
            limit: "max_air_row_values",
            actual: observed,
            max: crate::trace::DEFAULT_MAX_TRACE_COLUMNS,
        } if observed == actual
    ));
}
#[test]
fn fri_round_arity_uses_a_real_final_subgroup_and_rejects_padding() {
    assert_eq!(super::fri_round_arity(16, 2).unwrap(), 2);
    assert_eq!(super::fri_round_arity(1, 2).unwrap(), 1);
    let err = super::fri_round_arity(3, 2).unwrap_err();
    assert!(matches!(
        err,
        super::Error::FriDomainSize {
            length: 3,
            arity: 2
        }
    ));
}
#[test]
fn fold_round_matches_direct_constant_and_linear_evaluation() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let domain = super::FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        16,
        params.omega_coset,
    )
    .expect("FRI domain");
    let challenge = fp4(0x1234_5678_9abc_def0 % super::GOLDILOCKS_MODULUS);
    let constant = 37;
    let constant_values = vec![fp4(constant); 16];
    assert_eq!(
        super::fold_round(&constant_values, 2, challenge, domain).unwrap(),
        vec![fp4(constant); 8]
    );

    let intercept = 11;
    let slope = 29;
    let linear_values = (0..16)
        .map(|index| {
            reference_add(
                intercept,
                reference_mul(slope, reference_domain_point(params, 16, index)),
            )
        })
        .map(fp4)
        .collect::<Vec<_>>();
    let expected = fp4(intercept).add(challenge.mul(fp4(slope)));
    assert_eq!(
        super::fold_round(&linear_values, 2, challenge, domain).unwrap(),
        vec![expected; 8]
    );
}
#[test]
fn terminal_fri_degree_check_interpolates_the_folded_coset() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let domain = super::FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        8,
        params.omega_coset,
    )
    .expect("terminal FRI domain");
    let linear = (0..8)
        .map(|index| {
            let x = domain.point(index);
            reference_add(3, reference_mul(5, x))
        })
        .map(fp4)
        .collect::<Vec<_>>();
    assert!(domain.evaluations_have_degree_below(&linear, 2).unwrap());
    assert!(!domain.evaluations_have_degree_below(&linear, 1).unwrap());

    let quadratic = (0..8)
        .map(|index| {
            let x = domain.point(index);
            reference_add(
                reference_add(3, reference_mul(5, x)),
                reference_mul(7, reference_mul(x, x)),
            )
        })
        .map(fp4)
        .collect::<Vec<_>>();
    assert!(domain.evaluations_have_degree_below(&quadratic, 3).unwrap());
    assert!(!domain.evaluations_have_degree_below(&quadratic, 2).unwrap());
}
#[test]
fn fri_rejects_a_reduction_limit_that_cannot_expose_the_terminal_layer() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let error = super::fold_with_fri(
        &(0u64..64).collect::<Vec<_>>(),
        params.fri.arity,
        0,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect_err("zero reductions cannot expose the complete terminal layer");
    assert!(matches!(error, super::Error::FriReductionLimit { .. }));
}
#[test]
fn fri_merkle_leaves_commit_strided_cosets() {
    let values = fp4_values(&(0u64..16).collect::<Vec<_>>());
    let leaves =
        super::hash_fri_leaves_with_mode(0, &values, 2, ExecutionMode::Cpu).expect("FRI leaves");
    let first_coset = (0..2)
        .map(|position| values[position * 8])
        .collect::<Vec<_>>();
    let second_coset = (0..2)
        .map(|position| values[1 + position * 8])
        .collect::<Vec<_>>();
    assert_eq!(
        leaves[0],
        super::hash_fri_chunk(0, 0, &first_coset).unwrap()
    );
    assert_eq!(
        leaves[1],
        super::hash_fri_chunk(0, 1, &second_coset).unwrap()
    );
}
#[test]
fn fri_terminal_leaf_commits_every_value_in_domain_order() {
    let values = fp4_values(&[1, 2, 3, 4]);
    let leaves = hash_fri_terminal_leaves(7, &values).expect("complete terminal leaf");
    assert_eq!(leaves, [hash_fri_chunk(7, 0, &values).unwrap()]);
    for index in 0..values.len() {
        let mut mutated = values.clone();
        mutated[index] = mutated[index].add(fp4(1));
        assert_ne!(leaves, hash_fri_terminal_leaves(7, &mutated).unwrap());
    }
    let mut reordered = values.clone();
    reordered.swap(0, 1);
    assert_ne!(leaves, hash_fri_terminal_leaves(7, &reordered).unwrap());
    for length in [0, 3, 8] {
        assert!(matches!(
            hash_fri_terminal_leaves(7, &vec![fp4(0); length]),
            Err(Error::FriDomainSize { .. })
        ));
    }
}
#[test]
fn retained_fri_layers_preserve_full_field_roots_transcript_and_opening_bytes() {
    for (length, offset, mode) in [
        (1, 7, ExecutionMode::Cpu),
        (2, 11, ExecutionMode::Auto),
        (4, 7, ExecutionMode::Cpu),
        (32, 11, ExecutionMode::Auto),
        (128, 7, ExecutionMode::Cpu),
    ] {
        let mut params = fastpq_isi::FASTPQ_FINAL_V1;
        params.omega_coset = offset;
        let mut transcript =
            Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT)
                .unwrap();
        let mut reference = transcript.clone();
        let values = (0..length)
            .map(|index| {
                let value = index as u64;
                GoldilocksFp4V1::new([value + 1, value + 2, value + 3, value + 4]).unwrap()
            })
            .collect::<Vec<_>>();
        let mut retained =
            fold_with_fri_opening_layers(&values, &params, &mut transcript, mode).unwrap();
        assert_eq!(retained.layer_values[0], values);
        let mut domain =
            FriDomain::from_lde_parameters(params.lde_root, params.lde_log_size, length, offset)
                .unwrap();
        // Rebuild every commitment independently and replay the original
        // root/beta schedule, including all four extension-field lanes.
        for (round, layer) in retained.layer_values.iter().enumerate() {
            let terminal = round + 1 == retained.layer_values.len();
            let leaves = if terminal {
                hash_fri_terminal_leaves(round, layer).unwrap()
            } else {
                hash_fri_leaves_with_mode(round, layer, 2, ExecutionMode::Cpu).unwrap()
            };
            let root = merkle_root_with_mode(
                &leaves,
                MerkleTreeRoleV1::Fri(round as u32),
                ExecutionMode::Cpu,
            )
            .unwrap();
            assert_eq!(retained.roots[round], root);
            if terminal {
                reference.append_fri_final(root);
            } else {
                reference.append_fri_layer(round, root);
                let beta = reference.challenge_beta(round);
                assert_eq!(retained.betas[round], beta);
                assert_eq!(
                    retained.layer_values[round + 1],
                    fold_round(layer, 2, beta, domain).unwrap(),
                );
                domain = domain.folded(2);
            }
        }
        assert_eq!(transcript.state, reference.state);
        let sampled = sample_queries(length, 136, &mut transcript).unwrap();
        assert_eq!(
            sampled,
            sample_queries(length, 136, &mut reference).unwrap()
        );
        assert_eq!(transcript.state, reference.state);
        assert_eq!(transcript.counter, reference.counter);
        assert_eq!(
            retained.opening_trees.as_ref().unwrap().tree_build_count(),
            0
        );
        for count in [0, 1, 7, 136] {
            let indices = sampled
                .iter()
                .copied()
                .cycle()
                .take(count)
                .collect::<Vec<_>>();
            let expected =
                open_fri_query_chains(&retained.layer_values, &indices, 2, ExecutionMode::Cpu)
                    .unwrap();
            let actual = retained.open_query_chains(&indices, 2).unwrap();
            assert_eq!(actual, expected);
            // Openings are payload fields inside Proof, which owns the frame.
            assert_eq!(
                norito::codec::Encode::encode(&actual),
                norito::codec::Encode::encode(&expected),
            );
            assert_eq!(
                retained.opening_trees.as_ref().unwrap().tree_build_count(),
                0
            );
            for opening in actual {
                assert_eq!(opening.final_values, *retained.layer_values.last().unwrap());
                assert_eq!(opening.final_merkle_path.len(), 1);
                let round = retained.layer_values.len() - 1;
                let terminal = hash_fri_chunk(round, 0, &opening.final_values).unwrap();
                assert_eq!(opening.final_merkle_path[0].as_fastpq(), terminal);
                assert_eq!(
                    retained.roots[round],
                    merkle_node_hash(
                        MerkleTreeRoleV1::Fri(round as u32),
                        1,
                        0,
                        terminal,
                        terminal
                    )
                    .unwrap(),
                );
            }
        }
        // Explicit duplicate and upper-half indices exercise occurrence
        // order even when the transcript happens to sample another set.
        let indices = [length - 1, length / 2, 0, length - 1];
        assert_eq!(
            retained.open_query_chains(&indices, 2).unwrap(),
            open_fri_query_chains(&retained.layer_values, &indices, 2, mode).unwrap(),
        );
        assert_eq!(
            retained.opening_trees.as_ref().unwrap().tree_build_count(),
            0
        );
    }
}

#[test]
fn retained_fri_opening_errors_match_legacy_empty_arity_and_index_priority() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    for length in [0, 1, 4, 32] {
        let mut transcript =
            Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT)
                .unwrap();
        let mut retained = fold_with_fri_opening_layers(
            &vec![fp4(42); length],
            &params,
            &mut transcript,
            ExecutionMode::Cpu,
        )
        .unwrap();
        for arity in [2, 4] {
            for indices in [
                vec![],
                vec![length, 0],
                vec![usize::MAX, length],
                vec![0, length],
            ] {
                let expected = open_fri_query_chains(
                    &retained.layer_values,
                    &indices,
                    arity,
                    ExecutionMode::Cpu,
                );
                let actual = retained.open_query_chains(&indices, arity);
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            }
        }
        if length == 0 {
            assert!(retained.opening_trees.is_none());
            assert!(matches!(
                retained.open_query_chains(&[], 2),
                Err(Error::FriDomainSize { length: 0, .. }),
            ));
        } else {
            assert_eq!(
                retained.opening_trees.as_ref().unwrap().tree_build_count(),
                0
            );
        }
    }
}

#[test]
fn fri_terminal_query_opens_the_complete_four_point_domain() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let mut transcript =
        Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
    let result =
        fold_with_fri_opening_layers(&[fp4(42); 8], &params, &mut transcript, ExecutionMode::Cpu)
            .expect("binary fold to four terminal points");
    assert_eq!(
        result.layer_values.iter().map(Vec::len).collect::<Vec<_>>(),
        [8, 4]
    );
    assert_eq!(result.betas.len(), 1);
    let queries = open_fri_query_chains(&result.layer_values, &[0, 3, 4, 7], 2, ExecutionMode::Cpu)
        .expect("complete terminal openings");
    let terminal_values = result.layer_values.last().unwrap();
    for query in queries {
        assert_eq!(query.final_values, *terminal_values);
        assert_eq!(query.final_merkle_path.len(), 1);
        assert_eq!(query.final_index, query.initial_index % 4);
        let leaf = hash_fri_chunk(1, 0, &query.final_values).unwrap();
        assert!(
            verify_merkle_path_for_role(
                MerkleTreeRoleV1::Fri(1),
                result.roots[1],
                leaf,
                0,
                &query
                    .final_merkle_path
                    .iter()
                    .map(|digest| digest.as_fastpq())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        );
    }
}
#[test]
fn transcript_initialisation_separates_the_quotient_integer_and_terminal_schema() {
    let params = fastpq_isi::FASTPQ_FINAL_V1;
    let public_io = PublicIO::default();
    let transcript =
        Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
    let old_payload = norito::core::to_bytes(&(1_u16, params.name, public_io.clone())).unwrap();
    let old_state = hash_bytes_v1(
        TRANSCRIPT_ROLE_V1,
        b"initialise",
        0,
        0,
        0,
        &[TRANSCRIPT_TAG_INIT.as_bytes(), &old_payload],
    )
    .unwrap();
    assert_ne!(transcript.state, old_state);
    let repeated = Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
    assert_eq!(transcript.state, repeated.state);
}
#[test]
fn fri_folding_reduces_layer_length() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let evaluations: Vec<u64> = (0u64..16).map(|idx| idx + 1).collect();
    let (layers, betas) = super::fold_with_fri(
        &evaluations,
        2,
        params.fri.max_reductions,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect("fri folding");
    assert_eq!(layers.len(), betas.len() + 1);
    assert_eq!(betas.len(), 2);
    let mut transcript_again = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let (repeat_layers, repeat_betas) = super::fold_with_fri(
        &evaluations,
        2,
        params.fri.max_reductions,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript_again,
    )
    .expect("fri folding repeat");
    assert_eq!(layers, repeat_layers);
    assert_eq!(betas, repeat_betas);
}
fn reference_add(a: u64, b: u64) -> u64 {
    u64::try_from((u128::from(a) + u128::from(b)) % u128::from(super::GOLDILOCKS_MODULUS))
        .expect("reduced sum fits u64")
}
fn reference_mul(a: u64, b: u64) -> u64 {
    u64::try_from((u128::from(a) * u128::from(b)) % u128::from(super::GOLDILOCKS_MODULUS))
        .expect("reduced product fits u64")
}
fn reference_pow(mut base: u64, mut exponent: u64) -> u64 {
    let mut result = 1;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = reference_mul(result, base);
        }
        base = reference_mul(base, base);
        exponent >>= 1;
    }
    result
}
fn reference_inverse(value: u64) -> u64 {
    reference_pow(value, super::GOLDILOCKS_MODULUS - 2)
}
fn reference_domain_point(
    params: fastpq_isi::StarkParameterSet,
    domain_size: usize,
    index: usize,
) -> u64 {
    let domain_log = domain_size.ilog2();
    let stride = 1u64 << (params.lde_log_size - domain_log);
    let generator = reference_pow(params.lde_root, stride);
    reference_mul(
        params.omega_coset,
        reference_pow(generator, u64::try_from(index).expect("index fits u64")),
    )
}
fn reference_fold_round(
    values: &[GoldilocksFp4V1],
    configured_arity: usize,
    beta: GoldilocksFp4V1,
    domain: super::FriDomain,
) -> Vec<GoldilocksFp4V1> {
    let arity = configured_arity.min(values.len());
    assert_eq!(arity, 2, "V1 reference supports only binary FRI");
    assert_eq!(values.len() % arity, 0);
    let output_len = values.len() / arity;
    let inverse_two = reference_inverse(2);
    (0..output_len)
        .map(|leaf_index| {
            let positive = values[leaf_index];
            let negative = values[leaf_index + output_len];
            let even = positive.add(negative).mul_base(inverse_two);
            let odd = positive.sub(negative).mul_base(reference_mul(
                inverse_two,
                reference_inverse(domain.point(leaf_index)),
            ));
            even.add(beta.mul(odd))
        })
        .collect()
}
fn reference_fri_layer_commitment(
    round: usize,
    values: &[GoldilocksFp4V1],
) -> GoldilocksDigest384V1 {
    let leaves =
        hash_fri_leaves_with_mode(round, values, 2, ExecutionMode::Cpu).expect("typed FRI leaves");
    merkle_root_with_mode(
        &leaves,
        MerkleTreeRoleV1::Fri(u32::try_from(round).expect("round fits u32")),
        ExecutionMode::Cpu,
    )
    .expect("typed FRI root")
}
#[test]
fn fri_layers_match_reference_harness() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let arity = params.fri.arity as usize;
    let evaluations: Vec<u64> = (0u64..64)
        .map(|idx| idx.wrapping_mul(37).wrapping_add(5) % super::GOLDILOCKS_MODULUS)
        .collect();
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let (layers, betas) = super::fold_with_fri(
        &evaluations,
        params.fri.arity,
        4,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect("fold with fri");
    let mut reference_transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("reference transcript");
    let mut current = evaluations.iter().copied().map(fp4).collect::<Vec<_>>();
    let mut reference_layers = Vec::new();
    let mut reference_betas = Vec::new();
    let mut domain = super::FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        current.len(),
        params.omega_coset,
    )
    .expect("reference FRI domain");
    let mut round = 0usize;
    while current.len() > 4 && round < 4 {
        let root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_layer(round, root);
        reference_layers.push(root);
        let beta = reference_transcript.challenge_beta(round);
        reference_betas.push(beta);
        let round_arity = arity.min(current.len());
        current = reference_fold_round(&current, arity, beta, domain);
        domain = domain.folded(round_arity);
        round += 1;
    }
    let terminal_leaf = hash_fri_chunk(round, 0, &current).expect("complete terminal leaf");
    let final_root = merkle_root_with_mode(
        &[terminal_leaf],
        MerkleTreeRoleV1::Fri(round as u32),
        ExecutionMode::Cpu,
    )
    .expect("complete terminal root");
    reference_transcript.append_fri_final(final_root);
    reference_layers.push(final_root);
    assert_eq!(layers, reference_layers);
    assert_eq!(betas, reference_betas);
}
#[test]
fn fri_reference_detects_mutation() {
    let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
    let arity = params.fri.arity as usize;
    let evaluations: Vec<u64> = (0u64..64)
        .map(|idx| idx.wrapping_mul(19).wrapping_add(11) % super::GOLDILOCKS_MODULUS)
        .collect();
    let mut transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("transcript");
    let (baseline_layers, _) = super::fold_with_fri(
        &evaluations,
        params.fri.arity,
        4,
        params.lde_root,
        params.lde_log_size,
        params.omega_coset,
        &mut transcript,
    )
    .expect("baseline fold");
    let mut mutated = evaluations.clone();
    mutated[0] = mutated[0].wrapping_add(1);
    let mut reference_transcript = Transcript::initialise(
        &crate::proof::PublicIO::default(),
        params.name,
        1,
        TRANSCRIPT_TAG_INIT,
    )
    .expect("reference transcript");
    let mut current = mutated.iter().copied().map(fp4).collect::<Vec<_>>();
    let mut mutated_layers = Vec::new();
    let mut domain = super::FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        current.len(),
        params.omega_coset,
    )
    .expect("reference FRI domain");
    let mut round = 0usize;
    while current.len() > 4 && round < 4 {
        let root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_layer(round, root);
        mutated_layers.push(root);
        let beta = reference_transcript.challenge_beta(round);
        let round_arity = arity.min(current.len());
        current = reference_fold_round(&current, arity, beta, domain);
        domain = domain.folded(round_arity);
        round += 1;
    }
    let terminal_leaf = hash_fri_chunk(round, 0, &current).expect("complete terminal leaf");
    let final_root = merkle_root_with_mode(
        &[terminal_leaf],
        MerkleTreeRoleV1::Fri(round as u32),
        ExecutionMode::Cpu,
    )
    .expect("complete terminal root");
    reference_transcript.append_fri_final(final_root);
    mutated_layers.push(final_root);
    assert_ne!(baseline_layers, mutated_layers);
}
mod fri_properties {
    use super::*;
    use crate::Planner;
    use fastpq_isi::CANONICAL_PARAMETER_SETS;
    const MAX_TRACE_LOG: u32 = 4;
    fn fri_input_cases() -> Vec<(u32, Vec<u64>)> {
        let mut cases = Vec::new();
        for trace_log in 0..=MAX_TRACE_LOG {
            let len = 1usize << trace_log;
            for seed in [0_u64, 1, 0x1234, 0xFEED] {
                let coeffs = (0..len)
                    .map(|idx| {
                        seed.wrapping_add(idx as u64)
                            .wrapping_mul(0xD6E8_FEB8_6659_FD93)
                            % crate::poseidon::FIELD_MODULUS
                    })
                    .collect();
                cases.push((trace_log, coeffs));
            }
        }
        cases
    }
    #[test]
    fn fri_layers_and_betas_are_deterministic() {
        for (trace_log, coeffs) in fri_input_cases() {
            let params = CANONICAL_PARAMETER_SETS[0];
            let planner = Planner::new(&params);
            let trace_len = 1usize << trace_log;
            assert_eq!(coeffs.len(), trace_len);
            let evaluations = planner.lde_columns(std::slice::from_ref(&coeffs));
            let evaluation = evaluations.into_iter().next().expect("evaluation column");
            let mut transcript_a = Transcript::initialise(
                &crate::proof::PublicIO::default(),
                params.name,
                1,
                TRANSCRIPT_TAG_INIT,
            )
            .expect("transcript");
            let mut transcript_b = Transcript::initialise(
                &crate::proof::PublicIO::default(),
                params.name,
                1,
                TRANSCRIPT_TAG_INIT,
            )
            .expect("transcript");
            let (layers_a, betas_a) = fold_with_fri(
                &evaluation,
                params.fri.arity,
                params.fri.max_reductions,
                params.lde_root,
                params.lde_log_size,
                params.omega_coset,
                &mut transcript_a,
            )
            .expect("fri folding");
            let (layers_b, betas_b) = fold_with_fri(
                &evaluation,
                params.fri.arity,
                params.fri.max_reductions,
                params.lde_root,
                params.lde_log_size,
                params.omega_coset,
                &mut transcript_b,
            )
            .expect("fri folding");
            assert_eq!(layers_a, layers_b);
            assert_eq!(betas_a, betas_b);
            let expected_len = trace_len << planner.blowup_log();
            assert_eq!(evaluation.len(), expected_len);
        }
    }
}
