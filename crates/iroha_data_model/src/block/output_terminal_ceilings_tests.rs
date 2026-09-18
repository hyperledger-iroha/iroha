//! Canonical terminal sizing controls; no State or invocation authority is claimed.

use norito::codec::Encode as _;

use super::*;
use crate::block::output_budget::{
    ExecutionOutputBudget, ExecutionOutputLimits, ExecutionOutputPhaseReservation,
    ReservedExecutionOutput,
};

const PHASES: [ExecutionOutputPhase; 4] = [
    ExecutionOutputPhase::Network,
    ExecutionOutputPhase::Pipeline,
    ExecutionOutputPhase::Pipeline,
    ExecutionOutputPhase::Time,
];

fn rows(
    name: &str,
    position: u32,
    registered_at_height: u64,
    interval: TimeInterval,
    action_hash: Hash,
) -> [ExecutionOutputV1; 4] {
    let trigger = TriggerUseV1 {
        trigger_id: name.parse().unwrap(),
        registered_at_height,
        action_hash,
    };
    [
        ExecutionOutputV1::network_output_limit_rejection(position),
        ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
            event: PipelineEventPositionV1::Network(position),
            candidate_index: position,
            trigger: trigger.clone(),
        }),
        ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
            event: PipelineEventPositionV1::BlockApproved,
            candidate_index: position,
            trigger: trigger.clone(),
        }),
        ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
            schedule_index: position,
            event: TimeEvent { interval },
            trigger,
        }),
    ]
}

fn largest_rows() -> [ExecutionOutputV1; 4] {
    rows(
        &"x".repeat(MAX_NAME_BYTES),
        u32::MAX,
        u64::MAX - 1,
        TimeInterval {
            since_ms: u64::MAX - 1,
            length_ms: 1,
        },
        Hash::prehashed([0xff; Hash::LENGTH]),
    )
}

fn measured_roundtrip(row: &ExecutionOutputV1) -> u64 {
    assert!(row.is_output_limit_rejection());
    let bytes = norito::encode_canonical(row).unwrap();
    assert_eq!(norito::canonical_frame_len(row).unwrap(), bytes.len());
    assert_eq!(
        norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
        *row
    );
    u64::try_from(bytes.len()).unwrap()
}

fn policy(count: u32, row: u64, total: u64) -> ExecutionOutputLimits {
    ExecutionOutputLimits {
        max_outputs: count,
        max_output_bytes: row,
        max_total_output_bytes: total,
        // A test envelope only, not a complete block-wire feasibility claim.
        max_executed_wire_bytes: total + 4096,
    }
}

#[test]
fn derived_terminal_ceilings_measure_both_pipeline_variants_and_roundtrip() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    let sizes = largest_rows().each_ref().map(measured_roundtrip);
    assert_eq!(bounds.for_phase(ExecutionOutputPhase::Network), sizes[0]);
    assert_eq!(
        bounds.for_phase(ExecutionOutputPhase::Pipeline),
        sizes[1].max(sizes[2])
    );
    assert_eq!(bounds.for_phase(ExecutionOutputPhase::Time), sizes[3]);
    assert!(sizes.iter().all(|size| *size > 0));
}

#[test]
fn terminal_ceilings_cover_every_name_byte_length_and_maximum_utf8_names() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    let interval = TimeInterval {
        since_ms: 0,
        length_ms: u64::MAX,
    };
    // Include every legal byte length, including compact-prefix boundaries.
    // Roundtrips exercise the actual Name parser, not an unchecked long string.
    for bytes in 1..=MAX_NAME_BYTES {
        for (phase, row) in PHASES.into_iter().zip(rows(
            &"x".repeat(bytes),
            0,
            0,
            interval,
            Hash::new(b"a different fixed-width action identity"),
        )) {
            assert!(measured_roundtrip(&row) <= bounds.for_phase(phase));
        }
    }
    let ascii_sizes = largest_rows().each_ref().map(measured_roundtrip);
    for name in [
        format!("{}x", "é".repeat(127)),
        "界".repeat(85),
        format!("{}abc", "🦀".repeat(63)),
    ] {
        assert_eq!(name.len(), MAX_NAME_BYTES);
        let unicode = rows(
            &name,
            u32::MAX,
            u64::MAX - 1,
            interval,
            Hash::prehashed([0x81; Hash::LENGTH]),
        );
        assert_eq!(unicode.each_ref().map(measured_roundtrip), ascii_sizes);
    }
    assert!(
        "x".repeat(MAX_NAME_BYTES + 1)
            .parse::<crate::trigger::TriggerId>()
            .is_err()
    );
}

#[test]
fn terminal_scalar_values_use_fixed_width_not_decimal_or_varint_size() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    let maximum_sizes = largest_rows().each_ref().map(measured_roundtrip);
    let positions = [0, 1, 127, 128, 255, 256, u32::MAX - 1, u32::MAX];
    let heights = [0, 1, 127, 128, 255, 256, u32::MAX.into(), u64::MAX - 1];
    let intervals = [
        TimeInterval {
            since_ms: 0,
            length_ms: 0,
        },
        TimeInterval {
            since_ms: 0,
            length_ms: u64::MAX,
        },
        TimeInterval {
            since_ms: u64::MAX,
            length_ms: 0,
        },
        TimeInterval {
            since_ms: u64::MAX - 1,
            length_ms: 1,
        },
    ];
    for position in positions {
        assert_eq!(position.encode().len(), 4);
        for height in heights {
            assert_eq!(height.encode().len(), 8);
            for interval in intervals {
                assert!(interval.since_ms.checked_add(interval.length_ms).is_some());
                for hash_byte in [1, 0x7f, 0x80, 0xff] {
                    let hash = Hash::prehashed([hash_byte; Hash::LENGTH]);
                    assert_eq!(hash.encode().len(), Hash::LENGTH);
                    let sample = rows(
                        &"x".repeat(MAX_NAME_BYTES),
                        position,
                        height,
                        interval,
                        hash,
                    );
                    let sizes = sample.each_ref().map(|row| output_bytes(row).unwrap());
                    assert_eq!(sizes, maximum_sizes);
                    for (phase, size) in PHASES.into_iter().zip(sizes) {
                        assert!(size <= bounds.for_phase(phase));
                    }
                }
            }
        }
    }
    // Even the inadmissible registration-height encoding has the same width;
    // this helper is a size fact, not proof of chronology/source eligibility.
    assert_eq!(u64::MAX.encode().len(), 8);
}

#[test]
fn terminal_ceilings_are_independent_of_ambient_layout_guards() {
    let expected = ExecutionOutputTerminalCeilings::derive().unwrap();
    let expected_frames = largest_rows()
        .each_ref()
        .map(|row| norito::encode_canonical(row).unwrap());
    for flags in [
        0,
        norito::core::header_flags::PACKED_STRUCT
            | norito::core::header_flags::COMPACT_LEN
            | norito::core::header_flags::FIELD_BITSET,
    ] {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(ExecutionOutputTerminalCeilings::derive().unwrap(), expected);
        assert_eq!(
            largest_rows()
                .each_ref()
                .map(|row| norito::encode_canonical(row).unwrap()),
            expected_frames
        );
    }
}

#[test]
fn derived_envelope_reserves_exact_phase_counts_and_zeros_only_absent_phases() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    let network = bounds.for_phase(ExecutionOutputPhase::Network);
    let pipeline = bounds.for_phase(ExecutionOutputPhase::Pipeline);
    let time = bounds.for_phase(ExecutionOutputPhase::Time);
    let row = network.max(pipeline).max(time);
    let total = network + 2 * pipeline + time;
    let limits = policy(4, row, total);
    let envelope = bounds.envelope(1, 1);
    assert_eq!(envelope.terminal_bytes, [network, pipeline, time]);
    assert_eq!(envelope.maximum_network_inputs(&limits).unwrap(), 1);
    let phases = envelope.reservations(1, &limits).unwrap();
    assert_eq!(phases.map(|phase| phase.count), [1, 2, 1]);
    assert_eq!(
        phases.map(|phase| phase.terminal_bytes_per_output),
        [network, pipeline, time]
    );
    assert!(envelope.reservations(2, &limits).is_err());
    assert_eq!(
        envelope
            .maximum_network_inputs(&policy(4, row, total - 1))
            .unwrap(),
        0
    );
    assert!(
        envelope
            .reservations(1, &policy(4, row - 1, total))
            .is_err()
    );
    assert_eq!(bounds.envelope(0, 0).terminal_bytes, [network, 0, 0]);
    assert_eq!(bounds.envelope(1, 0).terminal_bytes, [network, pipeline, 0]);
    assert_eq!(bounds.envelope(0, 1).terminal_bytes, [network, 0, time]);
    let only_network = bounds.envelope(0, 0);
    let phases = only_network
        .reservations(1, &policy(1, network, network))
        .unwrap();
    assert_eq!(phases.map(|phase| phase.count), [1, 0, 0]);
    assert_eq!(
        phases.map(|phase| phase.terminal_bytes_per_output),
        [network, 0, 0]
    );
}

#[test]
fn caller_substituted_smaller_bounds_cannot_admit_a_maximum_terminal() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    for (phase, terminal) in PHASES.into_iter().zip(largest_rows()) {
        let actual_bytes = output_bytes(&terminal).unwrap();
        let maximum = bounds.for_phase(phase);
        let single = |bytes| {
            std::array::from_fn(|index| ExecutionOutputPhaseReservation {
                count: u32::from(index == phase.index()),
                terminal_bytes_per_output: if index == phase.index() { bytes } else { 0 },
            })
        };
        let mut admitted =
            ExecutionOutputBudget::new(policy(1, maximum, maximum), single(maximum)).unwrap();
        assert!(matches!(
            admitted
                .begin(terminal.clone())
                .unwrap()
                .finish(terminal.clone())
                .unwrap(),
            ReservedExecutionOutput::Accepted(_)
        ));
        assert_eq!(admitted.finish().unwrap(), (1, actual_bytes));

        // Existing raw envelope/budget arithmetic is deliberately not authority.
        // A caller altering its public scalar cannot defeat begin's exact check.
        let mut substituted =
            ExecutionOutputBudget::new(policy(1, maximum, maximum), single(actual_bytes - 1))
                .unwrap();
        assert!(substituted.begin(terminal).is_err());
        assert!(substituted.finish().is_err());
    }
}

#[test]
fn derived_capacity_never_authorizes_a_different_invocation_origin() {
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    for (phase, terminal) in PHASES.into_iter().zip(largest_rows()) {
        let maximum = bounds.for_phase(phase);
        let phases = std::array::from_fn(|index| ExecutionOutputPhaseReservation {
            count: u32::from(index == phase.index()),
            terminal_bytes_per_output: if index == phase.index() { maximum } else { 0 },
        });
        let mut actual = terminal.clone();
        match &mut actual {
            ExecutionOutputV1::Network(row) => row.input_index -= 1,
            ExecutionOutputV1::Pipeline(row) => {
                row.invocation.trigger.action_hash = Hash::new(b"substituted pipeline action");
            }
            ExecutionOutputV1::Time(row) => {
                row.invocation.trigger.action_hash = Hash::new(b"substituted Time action");
            }
        }
        assert_eq!(
            output_bytes(&actual).unwrap(),
            output_bytes(&terminal).unwrap()
        );
        let mut owner = ExecutionOutputBudget::new(policy(1, maximum, maximum), phases).unwrap();
        assert!(owner.begin(terminal).unwrap().finish(actual).is_err());
        assert!(owner.finish().is_err());
    }
}

#[test]
fn actual_internal_rejection_fallback_fits_derived_terminals_at_descriptor_extremes() {
    use crate::{
        ValidationFail,
        block::execution_output::TriggerFailureRootV1,
        events::trigger_completed::TriggerCompletedOutcome,
        transaction::{ExecutionStep, TransactionResult, error::TransactionRejectionReason},
    };
    let bounds = ExecutionOutputTerminalCeilings::derive().unwrap();
    for name in ["x".to_owned(), "x".repeat(MAX_NAME_BYTES), "界".repeat(85)] {
        for position in [0, 127, 128, u32::MAX] {
            for (height, interval) in [
                (
                    0,
                    TimeInterval {
                        since_ms: 0,
                        length_ms: 0,
                    },
                ),
                (
                    u64::MAX - 1,
                    TimeInterval {
                        since_ms: 0,
                        length_ms: u64::MAX,
                    },
                ),
                (
                    u64::MAX - 1,
                    TimeInterval {
                        since_ms: u64::MAX,
                        length_ms: 0,
                    },
                ),
            ] {
                let samples = rows(
                    &name,
                    position,
                    height,
                    interval,
                    Hash::prehashed([0xff; Hash::LENGTH]),
                );
                for (phase, terminal) in PHASES.into_iter().zip(samples).skip(1) {
                    let unit = output_bytes(&terminal).unwrap();
                    let mut actual = terminal.clone();
                    let (result, root, completions) = match &mut actual {
                        ExecutionOutputV1::Pipeline(row) => {
                            (&mut row.result, &mut row.failure_root, &mut row.completions)
                        }
                        ExecutionOutputV1::Time(row) => {
                            (&mut row.result, &mut row.failure_root, &mut row.completions)
                        }
                        ExecutionOutputV1::Network(_) => unreachable!(),
                    };
                    *result = TransactionResult::new(Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted("actual rejection".repeat(1024)),
                    )));
                    *root = Some(TriggerFailureRootV1::DeclaredInstructionProjection(
                        ExecutionStep(Vec::new().into()),
                    ));
                    completions[0].outcome =
                        TriggerCompletedOutcome::Failure("actual root failure".into());
                    assert!(output_bytes(&actual).unwrap() > unit);
                    let mut phases = [ExecutionOutputPhaseReservation {
                        count: 0,
                        terminal_bytes_per_output: 0,
                    }; 3];
                    phases[phase.index()] = ExecutionOutputPhaseReservation {
                        count: 1,
                        terminal_bytes_per_output: unit,
                    };
                    let mut owner =
                        ExecutionOutputBudget::new(policy(1, unit, unit), phases).unwrap();
                    let row = owner
                        .begin(terminal)
                        .unwrap()
                        .finish_internal_rejection(actual)
                        .unwrap();
                    assert!(row.is_internal_rejection_diagnostic_omitted());
                    assert!(!row.is_output_limit_rejection());
                    let bytes = norito::encode_canonical(&row).unwrap();
                    assert_eq!(norito::canonical_frame_len(&row).unwrap(), bytes.len());
                    assert!(bytes.len() as u64 <= unit);
                    assert!(unit <= bounds.for_phase(phase));
                    assert_eq!(
                        norito::decode_canonical::<ExecutionOutputV1>(&bytes).unwrap(),
                        row
                    );
                    let json = norito::json::to_json(&row).unwrap();
                    assert_eq!(
                        norito::json::from_str::<ExecutionOutputV1>(&json).unwrap(),
                        row
                    );
                    assert_eq!(owner.finish().unwrap(), (1, bytes.len() as u64));
                }
            }
        }
    }
}
