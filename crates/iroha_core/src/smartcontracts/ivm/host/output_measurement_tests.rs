//! Original local output-measurement refusals never become certified byte-limit faults.
#![expect(
    clippy::disallowed_types,
    reason = "these refusals exercise Norito's canonical sorting scratch, which only an unordered map allocates"
)]

use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use iroha_test_samples::ALICE_ID;
use norito::core::{DecodeLimits, DecodeLocalRefusalKind, with_decode_limits_scope};
use std::{cell::Cell, collections::HashMap, io::Write};

fn scratch_limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}

fn host_with_prior_effect() -> CoreHost {
    let mut host = CoreHost::new(ALICE_ID.clone());
    host.queue_instruction(
        iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "prior effect".into())
            .into(),
    )
    .unwrap();
    host
}

#[test]
fn real_map_scratch_refusal_keeps_original_owner_and_prevents_artifact_export() {
    let value = HashMap::from([(1_u64, 7_u64), (3, 11)]);
    let scratch = value.len() * std::mem::size_of::<(&u64, &u64)>();
    let mut host = host_with_prior_effect();
    let before = (host.retained_output_items(), host.retained_output_bytes());
    with_decode_limits_scope(scratch_limit(scratch - 1), || {
        assert!(!host.try_reserve_serialized_output(&value, 1));
    });
    let ivm::VMError::DecodeDeferred(original) = host.ensure_output_budget().unwrap_err() else {
        panic!("canonical map sorting must retain its actual enclosing allocation refusal");
    };
    assert_eq!(original.kind(), DecodeLocalRefusalKind::EnclosingLimit);
    assert_eq!(
        original.resource_error(),
        norito::core::DecodeResourceError::TotalAllocationExceeded {
            attempted: u64::try_from(scratch).unwrap(),
            limit: u64::try_from(scratch - 1).unwrap(),
        }
    );
    assert_eq!(host.output_budget_violation(), None);
    assert_eq!(host.queued.len(), 1);
    assert_eq!(
        (host.retained_output_items(), host.retained_output_bytes()),
        before
    );
    assert!(!host.try_reserve_output(1, 1));
    assert!(!host.try_reserve_serialized_output(&value, 1));
    let Err(ExecutionAttemptError::Deferred(retained)) = host.take_execution_artifacts(None) else {
        panic!("unfinished measurement must not publish even the preceding valid instruction");
    };
    assert_eq!(retained.decode_refusal(), Some(&original));
    assert_eq!(host.queued.len(), 1, "failed export does not drain custody");
    for refusal in [
        host.drain_instructions().unwrap_err(),
        host.drain_queued_effects_with_contract_runtime_context(None)
            .unwrap_err(),
    ] {
        assert!(
            matches!(refusal, ivm::VMError::DecodeDeferred(ref retained) if retained == &original)
        );
    }
    assert_eq!(
        host.queued.len(),
        1,
        "raw instruction drains preserve refused custody"
    );
    assert_eq!(
        (host.retained_output_items(), host.retained_output_bytes()),
        before
    );

    // A new complete attempt with enough local scratch retains the identical
    // output price. It does not resume the poisoned measurement or invent a result.
    let mut unconstrained = host_with_prior_effect();
    assert!(unconstrained.try_reserve_serialized_output(&value, 1));
    let mut retry = host_with_prior_effect();
    with_decode_limits_scope(scratch_limit(scratch), || {
        assert!(retry.try_reserve_serialized_output(&value, 1));
    });
    assert_eq!(
        retry.retained_output_items(),
        unconstrained.retained_output_items()
    );
    assert_eq!(
        retry.retained_output_bytes(),
        unconstrained.retained_output_bytes()
    );
    assert!(retry.ensure_output_budget().is_ok());
    assert!(retry.take_execution_artifacts(None).is_ok());
    assert_eq!(value, HashMap::from([(1_u64, 7_u64), (3, 11)]));
}

#[derive(Clone, Copy)]
enum Script {
    Allocator,
    Invalid,
    ReconstructedLimit,
    SwallowOverflow,
    PropagateOverflow,
    Panic,
}

struct ScriptedEncoder {
    script: Script,
    calls: Cell<usize>,
}

impl norito::NoritoSchema for ScriptedEncoder {
    fn nominal_name() -> String {
        "iroha:test:output-measurement-script".into()
    }

    fn static_nominal_name() -> Option<&'static str> {
        Some("iroha:test:output-measurement-script")
    }
}

impl norito::SerializePayload for ScriptedEncoder {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.calls.set(self.calls.get() + 1);
        encoder.write_all(b"a")?;
        match self.script {
            Script::Allocator => Err(norito::Error::AllocationFailed { bytes: 73 }),
            Script::Invalid => Err(norito::Error::LengthMismatch),
            Script::ReconstructedLimit => Err(norito::Error::TotalAllocationExceeded {
                attempted: 73,
                limit: 72,
            }),
            Script::SwallowOverflow => {
                let _ignored = encoder.write_all(b"oversized");
                Ok(())
            }
            Script::PropagateOverflow => {
                encoder.write_all(b"oversized")?;
                Ok(())
            }
            Script::Panic => panic!("output measurement unwind after a partial write"),
        }
    }
}

#[test]
fn allocator_and_malformed_encoders_cannot_claim_a_byte_overflow() {
    for script in [
        Script::Allocator,
        Script::Invalid,
        Script::ReconstructedLimit,
    ] {
        let mut host = host_with_prior_effect();
        let before = (host.retained_output_items(), host.retained_output_bytes());
        let value = ScriptedEncoder {
            script,
            calls: Cell::new(0),
        };
        assert!(!host.try_reserve_serialized_output(&value, 1));
        assert_eq!(value.calls.get(), 1);
        assert_eq!(host.output_budget_violation(), None);
        assert_eq!(
            (host.retained_output_items(), host.retained_output_bytes()),
            before
        );
        match script {
            Script::Allocator => {
                let ivm::VMError::DecodeDeferred(original) =
                    host.ensure_output_budget().unwrap_err()
                else {
                    panic!("an allocator report remains a local refusal");
                };
                assert_eq!(original.kind(), DecodeLocalRefusalKind::Allocator);
                assert_eq!(original.allocation_bytes(), Some(73));
                let metered = ivm::VMError::metered(41, host.ensure_output_budget().unwrap_err());
                assert_eq!(metered.fault_kind(), None);
                assert_eq!(metered.split_metered().0, None);
                let Err(ExecutionAttemptError::Deferred(retained)) =
                    host.take_execution_artifacts(None)
                else {
                    panic!("allocator report cannot export a partial artifact");
                };
                assert_eq!(retained.decode_refusal(), Some(&original));
            }
            _ => {
                assert!(matches!(
                    host.ensure_output_budget(),
                    Err(ivm::VMError::NoritoInvalid)
                ));
                assert!(matches!(
                    host.take_execution_artifacts(None),
                    Err(ExecutionAttemptError::Rejected(_))
                ));
            }
        }
        assert!(!host.try_reserve_serialized_output(&value, 1));
        assert_eq!(value.calls.get(), 1, "the first failure stays sticky");
        assert_eq!(host.queued.len(), 1);
        assert!(host.drain_instructions().is_err());
        assert!(
            host.drain_queued_effects_with_contract_runtime_context(None)
                .is_err()
        );
        assert_eq!(host.queued.len(), 1);
    }
}

#[test]
fn measurement_failures_survive_nested_rollback_and_limit_reconfiguration() {
    for malformed in [false, true] {
        for outcome in [
            NestedContractCallOutcome::Commit,
            NestedContractCallOutcome::Rollback,
            NestedContractCallOutcome::RollbackPreservingReads,
        ] {
            let mut host = host_with_prior_effect();
            let before = (host.retained_output_items(), host.retained_output_bytes());
            let snapshot = host.snapshot_nested_contract_call();
            if malformed {
                let value = ScriptedEncoder {
                    script: Script::Invalid,
                    calls: Cell::new(0),
                };
                assert!(!host.try_reserve_serialized_output(&value, 1));
            } else {
                let value = HashMap::from([(1_u64, 2_u64)]);
                with_decode_limits_scope(scratch_limit(0), || {
                    assert!(!host.try_reserve_serialized_output(&value, 1));
                });
            }
            host.finish_nested_contract_call(snapshot, outcome).unwrap();
            assert_eq!(
                (host.retained_output_items(), host.retained_output_bytes()),
                before
            );
            assert_eq!(host.queued.len(), 1);
            // Both methods are documented as pre-execution setup. Even their
            // accidental use here must not erase an incomplete attempt.
            host.set_output_limits_from_parameters(SmartContractParameters::default());
            host.restrict_output_limits(HostOutputLimits::new(u64::MAX, u64::MAX));
            assert!(!host.try_reserve_output(1, 1));
            if malformed {
                assert!(matches!(
                    host.ensure_output_budget(),
                    Err(ivm::VMError::NoritoInvalid)
                ));
                assert!(matches!(
                    host.take_execution_artifacts(None),
                    Err(ExecutionAttemptError::Rejected(_))
                ));
            } else {
                let ivm::VMError::DecodeDeferred(original) =
                    host.ensure_output_budget().unwrap_err()
                else {
                    panic!("nested rollback must retain local measurement failure");
                };
                let Err(ExecutionAttemptError::Deferred(retained)) =
                    host.take_execution_artifacts(None)
                else {
                    panic!("nested local refusal cannot become an artifact");
                };
                assert_eq!(retained.decode_refusal(), Some(&original));
            }
            assert_eq!(host.queued.len(), 1);
        }
    }
}

#[test]
fn actual_counting_sink_overflow_remains_a_deterministic_limit_violation() {
    for script in [Script::PropagateOverflow, Script::SwallowOverflow] {
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.restrict_output_limits(HostOutputLimits::new(1, 65));
        let value = ScriptedEncoder {
            script,
            calls: Cell::new(0),
        };
        assert!(!host.try_reserve_serialized_output(&value, 1));
        assert_eq!(
            host.output_budget_violation(),
            Some(HostOutputBudgetViolation::EncodedBytes {
                attempted: 66,
                limit: 65,
            })
        );
        assert!(host.output_measurement_failure.is_none());
        assert_eq!(host.retained_output_items(), 0);
        assert_eq!(host.retained_output_bytes(), 0);
        assert!(matches!(
            host.ensure_output_budget(),
            Err(ivm::VMError::HostOutputBudgetExceeded {
                resource: ivm::HostOutputResource::Bytes,
                attempted: 66,
                limit: 65,
            })
        ));
        assert!(matches!(
            host.take_execution_artifacts(None),
            Err(ExecutionAttemptError::Rejected(_))
        ));
    }
    let mut writer = BoundedCountingWriter {
        written: usize::MAX,
        max: usize::MAX,
        exceeded: false,
    };
    assert!(writer.write_all(b"a").is_err());
    assert!(writer.exceeded);
    assert_eq!(writer.written, usize::MAX);
}

#[test]
fn prior_output_limit_refusal_stops_the_encoder_before_local_work() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    host.restrict_output_limits(HostOutputLimits::new(0, u64::MAX));
    assert!(!host.try_reserve_output(1, 0));
    let value = ScriptedEncoder {
        script: Script::Allocator,
        calls: Cell::new(0),
    };
    assert!(!host.try_reserve_serialized_output(&value, 1));
    assert_eq!(value.calls.get(), 0);
    assert!(host.output_measurement_failure.is_none());
    assert!(matches!(
        host.ensure_output_budget(),
        Err(ivm::VMError::HostOutputBudgetExceeded {
            resource: ivm::HostOutputResource::Items,
            attempted: 1,
            limit: 0,
        })
    ));
}

#[test]
fn encoder_unwind_cannot_export_prior_effects_or_a_partial_measurement() {
    let mut host = host_with_prior_effect();
    let before = (host.retained_output_items(), host.retained_output_bytes());
    let value = ScriptedEncoder {
        script: Script::Panic,
        calls: Cell::new(0),
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            host.try_reserve_serialized_output(&value, 1)
        }))
        .is_err()
    );
    assert_eq!(value.calls.get(), 1);
    assert_eq!(host.output_budget_violation(), None);
    assert_eq!(
        (host.retained_output_items(), host.retained_output_bytes()),
        before
    );
    assert!(matches!(
        host.ensure_output_budget(),
        Err(ivm::VMError::ExecutionDeferred(
            ivm::ExecutionDeferral::LocalInvariantViolation
        ))
    ));
    assert!(matches!(
        host.take_execution_artifacts(None),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    assert!(!host.try_reserve_serialized_output(&value, 1));
    assert_eq!(value.calls.get(), 1);
    assert_eq!(host.queued.len(), 1);
}

#[test]
fn deterministic_output_fault_keeps_diagnostic_drains_but_never_certified_artifacts() {
    let instruction = InstructionBox::from(iroha_data_model::isi::Log::new(
        iroha_data_model::Level::INFO,
        "retained diagnostic".into(),
    ));
    let mut measured = CoreHost::new(ALICE_ID.clone());
    measured.queue_instruction(instruction.clone()).unwrap();
    let cost = measured.retained_output_bytes();
    for item_limit in [false, true] {
        for context_drain in [false, true] {
            let mut host = CoreHost::new(ALICE_ID.clone());
            host.restrict_output_limits(HostOutputLimits::new(
                if item_limit { 1 } else { 2 },
                cost,
            ));
            host.queue_instruction(instruction.clone()).unwrap();
            if item_limit {
                assert!(!host.try_reserve_output(1, 0));
            } else {
                assert!(!host.try_reserve_serialized_output(&instruction, 1));
            }
            assert!(host.output_budget_violation().is_some());
            assert!(host.output_measurement_failure.is_none());
            assert!(host.ensure_output_measurement().is_ok());
            assert!(matches!(
                host.ensure_output_budget(),
                Err(ivm::VMError::HostOutputBudgetExceeded { .. })
            ));
            assert!(matches!(
                host.take_execution_artifacts(None),
                Err(ExecutionAttemptError::Rejected(_))
            ));
            assert_eq!(host.queued.len(), 1);
            if context_drain {
                let diagnostic = host
                    .drain_queued_effects_with_contract_runtime_context(None)
                    .unwrap();
                assert_eq!(diagnostic.len(), 1);
                assert_eq!(diagnostic[0].instruction(), Some(&instruction));
            } else {
                assert_eq!(
                    host.drain_instructions().unwrap(),
                    vec![instruction.clone()]
                );
            }
            assert!(host.queued.is_empty());
            assert!(matches!(
                host.take_execution_artifacts(None),
                Err(ExecutionAttemptError::Rejected(_))
            ));
        }
    }
}
