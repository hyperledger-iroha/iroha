//! Arithmetic, canonical identity and complete-envelope policy regressions.

use super::*;

// Synthetic arithmetic units, not measured release defaults. The separate Core
// corpus fixture supplies actual canonical transcript/statement sizes.
fn intrinsic() -> FastpqSourceLimitsV1 {
    FastpqSourceLimitsV1 {
        max_executed_entries: 1,
        max_transcripts: 3,
        max_deltas: 7,
        max_input_transcript_bytes: 1_013,
        max_statement_bytes: 2_027,
        max_total_statement_bytes: 2_027,
    }
}

fn mandatory() -> FastpqMandatorySourcePolicyV1 {
    FastpqMandatorySourcePolicyV1 {
        max_retained_obligations: 5,
        per_obligation: FastpqSourceLimitsV1 {
            max_executed_entries: 1,
            max_transcripts: 1,
            max_deltas: 1,
            max_input_transcript_bytes: 211,
            max_statement_bytes: 307,
            max_total_statement_bytes: 307,
        },
    }
}

fn output() -> ExecutionOutputPolicyV1 {
    ExecutionOutputPolicyV1 {
        max_pipeline_triggers: 2,
        max_time_invocations: 3,
        ..ExecutionOutputPolicyV1::bootstrap()
    }
}

fn profile(inputs: u32) -> FastpqSourcePolicyV1 {
    FastpqSourcePolicyV1::from_sizing(output(), intrinsic(), mandatory(), inputs).unwrap()
}

#[test]
fn disjoint_aggregation_adds_counts_and_bytes_but_never_maximum() {
    let unit = intrinsic();
    assert_eq!(
        unit.checked_repeat_entries(0).unwrap(),
        FastpqSourceLimitsV1::ZERO
    );
    assert_eq!(
        unit.checked_add_entries(FastpqSourceLimitsV1::ZERO)
            .unwrap(),
        unit
    );
    let repeated = unit.checked_repeat_entries(17).unwrap();
    let accumulated = (0..17)
        .try_fold(FastpqSourceLimitsV1::ZERO, |sum, _| {
            sum.checked_add_entries(unit)
        })
        .unwrap();
    assert_eq!(repeated, accumulated);
    assert_eq!(repeated.max_executed_entries, 17);
    assert_eq!(repeated.max_transcripts, 51);
    assert_eq!(repeated.max_deltas, 119);
    assert_eq!(repeated.max_input_transcript_bytes, 17 * 1_013);
    assert_eq!(repeated.max_total_statement_bytes, 17 * 2_027);
    assert_eq!(repeated.max_statement_bytes, 2_027);
    assert!(!repeated.fits_within(unit));
    assert!(unit.fits_within(repeated));
    assert!(repeated.fits_within(repeated));
}

#[test]
fn every_additive_dimension_rejects_overflow() {
    for dimension in 0..5 {
        let mut changed = intrinsic();
        match dimension {
            0 => changed.max_executed_entries = u32::MAX,
            1 => changed.max_transcripts = u32::MAX,
            2 => changed.max_deltas = u32::MAX,
            3 => changed.max_input_transcript_bytes = u64::MAX,
            4 => changed.max_total_statement_bytes = u64::MAX,
            _ => unreachable!(),
        }
        assert!(
            changed.checked_add_entries(intrinsic()).is_err(),
            "dimension {dimension}"
        );
        assert!(
            changed.checked_repeat_entries(2).is_err(),
            "dimension {dimension}"
        );
    }
    let mut maximum = intrinsic();
    maximum.max_statement_bytes = u64::MAX;
    assert_eq!(
        maximum
            .checked_add_entries(intrinsic())
            .unwrap()
            .max_statement_bytes,
        u64::MAX
    );
}

#[test]
fn empty_usage_and_all_six_ceilings_are_inclusive() {
    let limit = intrinsic();
    assert!(FastpqSourceLimitsV1::ZERO.fits_within(limit));
    for dimension in 0..6 {
        let mut value = limit;
        match dimension {
            0 => value.max_executed_entries += 1,
            1 => value.max_transcripts += 1,
            2 => value.max_deltas += 1,
            3 => value.max_input_transcript_bytes += 1,
            4 => value.max_statement_bytes += 1,
            5 => value.max_total_statement_bytes += 1,
            _ => unreachable!(),
        }
        assert!(!value.fits_within(limit), "dimension {dimension}");
    }
}

#[test]
fn mandatory_reservation_counts_all_retained_obligations() {
    let policy = mandatory();
    let actual = policy.reservation().unwrap();
    assert_eq!(actual.max_executed_entries, 5);
    assert_eq!(actual.max_transcripts, 5);
    assert_eq!(actual.max_statement_bytes, 307);
    assert_eq!(actual.max_total_statement_bytes, 5 * 307);
    let mut changed = policy;
    changed.max_retained_obligations = 0;
    assert!(changed.reservation().is_err());
    changed = policy;
    changed.per_obligation.max_executed_entries = 2;
    assert!(changed.reservation().is_err());
    changed = policy;
    changed.max_retained_obligations = u32::MAX;
    changed.per_obligation.max_transcripts = 2;
    changed.per_obligation.max_deltas = 2;
    assert!(changed.reservation().is_err());
}

#[test]
fn complete_source_envelope_preserves_output_fanout_and_mandatory_pool() {
    let policy = profile(4);
    // Four Network, ten possible Pipeline, three Time, five obligations.
    assert_eq!(policy.block.max_executed_entries, 4 + 10 + 3 + 5);
    assert_eq!(policy.block.max_transcripts, 17 * 3 + 5);
    assert_eq!(policy.block.max_deltas, 17 * 7 + 5);
    assert_eq!(
        policy.block.max_input_transcript_bytes,
        17 * 1_013 + 5 * 211
    );
    assert_eq!(policy.block.max_total_statement_bytes, 17 * 2_027 + 5 * 307);
    assert_eq!(policy.block.max_statement_bytes, 2_027);
    assert_eq!(policy.maximum_network_inputs(output()).unwrap(), 4);
    policy.validate(output()).unwrap();
    let bootstrap = ExecutionOutputPolicyV1::bootstrap();
    assert_eq!(invocation_count(bootstrap, 1).unwrap(), 1_025);
    assert_eq!(invocation_count(bootstrap, 252).unwrap(), 65_532);
    assert!(bootstrap.maximum_terminal_network_inputs().unwrap() <= 252);
}

#[test]
fn pure_capacity_formula_matches_exhaustive_small_envelopes() {
    for pipeline in 0..=5 {
        for time in 1..=5 {
            let output = ExecutionOutputPolicyV1 {
                max_pipeline_triggers: pipeline,
                max_time_invocations: time,
                ..output()
            };
            for network in 1..=8 {
                let policy =
                    FastpqSourcePolicyV1::from_sizing(output, intrinsic(), mandatory(), network)
                        .unwrap();
                assert_eq!(policy.maximum_network_inputs(output).unwrap(), network);
                let reserve = mandatory().reservation().unwrap();
                for candidate in 0..=10 {
                    let required = intrinsic()
                        .checked_repeat_entries(invocation_count(output, candidate).unwrap())
                        .unwrap()
                        .checked_add_entries(reserve)
                        .unwrap();
                    assert_eq!(required.fits_within(policy.block), candidate <= network);
                }
            }
        }
    }
}

#[test]
fn every_additive_ceiling_can_independently_reduce_admission() {
    let original = profile(4);
    for dimension in 0..5 {
        let mut policy = original;
        match dimension {
            0 => policy.block.max_executed_entries -= 1,
            1 => policy.block.max_transcripts -= 1,
            2 => policy.block.max_deltas -= 1,
            3 => policy.block.max_input_transcript_bytes -= 1,
            4 => policy.block.max_total_statement_bytes -= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            policy.maximum_network_inputs(output()).unwrap(),
            3,
            "dimension {dimension}"
        );
    }
    let mut changed = original;
    changed.block.max_statement_bytes -= 1;
    assert!(changed.maximum_network_inputs(output()).is_err());
    let mut changed = profile(1);
    changed.block.max_executed_entries -= 1;
    assert_eq!(changed.maximum_network_inputs(output()).unwrap(), 0);
    assert!(changed.validate(output()).is_err());
}

#[test]
fn rejects_malformed_intrinsic_and_uncovered_obligations() {
    for mutation in 0..8 {
        let mut value = intrinsic();
        match mutation {
            0 => value.max_executed_entries = 0,
            1 => value.max_executed_entries = 2,
            2 => value.max_transcripts = 0,
            3 => value.max_transcripts = value.max_deltas + 1,
            4 => value.max_deltas = u32::MAX / 2 + 1,
            5 => value.max_input_transcript_bytes = 0,
            6 => {
                value.max_statement_bytes = 0;
                value.max_total_statement_bytes = 0;
            }
            7 => value.max_total_statement_bytes += 1,
            _ => unreachable!(),
        }
        assert!(
            FastpqSourcePolicyV1::from_sizing(output(), value, mandatory(), 1).is_err(),
            "mutation {mutation}"
        );
    }
    assert!(FastpqSourcePolicyV1::from_sizing(output(), intrinsic(), mandatory(), 0).is_err());
    let mut obligation = mandatory();
    obligation.per_obligation.max_input_transcript_bytes =
        intrinsic().max_input_transcript_bytes + 1;
    assert!(FastpqSourcePolicyV1::from_sizing(output(), intrinsic(), obligation, 1).is_err());
    let too_many = output().maximum_terminal_network_inputs().unwrap() + 1;
    assert!(
        FastpqSourcePolicyV1::from_sizing(output(), intrinsic(), mandatory(), too_many).is_err()
    );
}

#[test]
fn canonical_profile_identity_changes_with_each_policy_dimension() {
    let original = profile(4);
    let digest = original.digest(output()).unwrap();
    let frame = norito::encode_canonical(&original).unwrap();
    assert_eq!(
        norito::decode_canonical::<FastpqSourcePolicyV1>(&frame).unwrap(),
        original
    );
    assert!(norito::decode_canonical::<FastpqSourceLimitsV1>(&frame).is_err());
    for mutation in 0..7 {
        let mut changed = original;
        match mutation {
            0 => changed.block.max_executed_entries += 1,
            1 => changed.block.max_transcripts += 1,
            2 => changed.block.max_deltas += 1,
            3 => changed.block.max_input_transcript_bytes += 1,
            4 => changed.block.max_statement_bytes += 1,
            5 => changed.block.max_total_statement_bytes += 1,
            6 => changed.mandatory.max_retained_obligations -= 1,
            _ => unreachable!(),
        }
        assert_ne!(
            changed.digest(output()).unwrap(),
            digest,
            "mutation {mutation}"
        );
    }
    let mut json = norito::json::to_value(&original).unwrap();
    assert_eq!(
        norito::json::from_value::<FastpqSourcePolicyV1>(json.clone()).unwrap(),
        original
    );
    json.as_object_mut()
        .unwrap()
        .insert("local_override".into(), 1_u64.into());
    assert!(norito::json::from_value::<FastpqSourcePolicyV1>(json).is_err());
    let mut missing = norito::json::to_value(&original).unwrap();
    missing.as_object_mut().unwrap().remove("mandatory");
    assert!(norito::json::from_value::<FastpqSourcePolicyV1>(missing).is_err());
}

#[test]
fn measured_bounded_corpus_derives_a_finite_candidate_without_selecting_a_default() {
    // Sep26 sizing-evidence: largest vector in the selected ML-DSA single-key
    // sixteen-singleton corpus, maximum quantity mantissas/scales and 256 paths.
    // These are measured corpus bounds, not maxima for every legal controller.
    let entry = FastpqSourceLimitsV1 {
        max_executed_entries: 1,
        max_transcripts: 16,
        max_deltas: 16,
        max_input_transcript_bytes: 670_864,
        max_statement_bytes: 272_175,
        max_total_statement_bytes: 272_175,
    };
    // The measured sixteen-member ML-DSA singleton also fits this entry bound.
    // Sixty-four retained obligations is a sizing scenario, not a shipped cap.
    let mandatory = FastpqMandatorySourcePolicyV1 {
        max_retained_obligations: 64,
        per_obligation: FastpqSourceLimitsV1 {
            max_executed_entries: 1,
            max_transcripts: 1,
            max_deltas: 1,
            max_input_transcript_bytes: 159_609,
            max_statement_bytes: 252_617,
            max_total_statement_bytes: 252_617,
        },
    };
    let output = ExecutionOutputPolicyV1::bootstrap();
    let candidate = FastpqSourcePolicyV1::from_sizing(output, entry, mandatory, 1).unwrap();
    assert_eq!(candidate.block.max_executed_entries, 1_025 + 64);
    assert_eq!(candidate.block.max_transcripts, 1_025 * 16 + 64);
    assert_eq!(candidate.block.max_deltas, 1_025 * 16 + 64);
    assert_eq!(
        candidate.block.max_input_transcript_bytes,
        1_025 * 670_864 + 64 * 159_609
    );
    assert_eq!(candidate.block.max_statement_bytes, 272_175);
    assert_eq!(
        candidate.block.max_total_statement_bytes,
        1_025 * 272_175 + 64 * 252_617
    );
    assert_eq!(candidate.maximum_network_inputs(output).unwrap(), 1);
    // Reserving every potential invocation exposes the real scale. Source policy
    // feasibility alone must not be mistaken for full wire/host feasibility.
    assert!(candidate.block.max_input_transcript_bytes > output.max_executed_wire_bytes);
    assert!(candidate.block.max_total_statement_bytes > output.max_executed_wire_bytes);
}

#[test]
fn bootstrap_matches_measured_runtime_corpus_and_checked_sizing() {
    let policy = FastpqSourcePolicyV1::bootstrap();
    let output = ExecutionOutputPolicyV1::bootstrap();
    assert_eq!(
        policy,
        FastpqSourcePolicyV1::from_sizing(
            output,
            policy.intrinsic,
            policy.mandatory,
            FastpqSourcePolicyV1::BOOTSTRAP_NETWORK_INPUTS
        )
        .unwrap()
    );
    assert_eq!(policy.maximum_network_inputs(output).unwrap(), 11);
    assert_eq!(policy.block.max_input_transcript_bytes, 501_314_448);
    assert_eq!(policy.block.max_total_statement_bytes, 994_636_613);
    // Original runtime paths are empty. A richer sixteen-occurrence input is
    // still charged in full and must not fit merely because archive paths exist.
    let with_full_paths = FastpqSourceLimitsV1 {
        max_input_transcript_bytes: 670_864,
        ..policy.intrinsic
    };
    assert!(!with_full_paths.fits_within(policy.intrinsic));
    assert!(
        policy
            .mandatory
            .per_obligation
            .fits_within(policy.intrinsic)
    );
    let additional_obligation = policy
        .mandatory
        .per_obligation
        .checked_repeat_entries(65)
        .unwrap();
    assert!(!additional_obligation.fits_within(policy.mandatory.reservation().unwrap()));
}

#[test]
fn policy_display_retains_all_three_profile_components() {
    let policy = FastpqSourcePolicyV1::bootstrap();
    assert_eq!(
        policy.to_string(),
        format!(
            "{:?},{:?},{:?}_FASTPQ_SOURCE",
            policy.intrinsic, policy.block, policy.mandatory
        )
    );
}
