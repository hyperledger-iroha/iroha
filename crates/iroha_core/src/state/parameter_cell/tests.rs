//! Actual MV envelope, both retained graphs and original control/refusal tests.

use super::*;
use crate::{pipeline_gas_policy::StateParameters, test_allocations::allocations_during};
use iroha_data_model::parameter::{Parameter, Parameters, SumeragiParameter};
use std::time::{Duration, Instant};

fn fixture() -> String {
    let cell = Cell::new(StateParameters::default());
    let mut block = cell.block();
    block
        .get_mut()
        .set_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(7)))
        .unwrap();
    block.commit();
    json::to_json(&cell).unwrap()
}
fn collect(pool: &AllocationBudget) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while pool.reserved_bytes() != 0 {
        assert!(
            Instant::now() < deadline,
            "original Cell and graph backing still retained"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

#[test]
fn exact_cell_restore_keeps_current_undo_and_late_graph_reader_on_original_pool() {
    let source = fixture();
    let pool = AllocationBudget::new(1 << 24);
    let cell = restore(&source, &pool).unwrap();
    assert_eq!(json::to_json(&cell).unwrap(), source);
    assert!(cell.view().get().belongs_to(&pool));
    assert_eq!(cell.view().sumeragi.max_clock_drift_ms, 7);
    {
        let previous = cell.predecessor_view();
        let previous = previous.get().as_ref().unwrap();
        assert!(previous.belongs_to(&pool));
        assert_eq!(
            previous.sumeragi.max_clock_drift_ms,
            Parameters::default().sumeragi.max_clock_drift_ms
        );
    }
    let late = cell.view().get().clone();
    drop(cell);
    for _ in 0..32 {
        crossbeam_epoch::pin().flush();
    }
    assert!(pool.reserved_bytes() > 0);
    assert_eq!(late.sumeragi.max_clock_drift_ms, 7);
    drop(late);
    collect(&pool);
}

#[test]
fn full_initial_controls_refuse_before_parse_and_every_partial_graph_refunds() {
    let pool = AllocationBudget::new(0);
    assert_eq!(
        allocations_during(|| {
            assert!(matches!(
                restore("not JSON", &pool),
                Err(ParameterCellRestoreError::Initialization(
                    CellInitializationError::Admission(_)
                ))
            ));
        }),
        0
    );
    let source = fixture();
    let measured = AllocationBudget::new(1 << 24);
    drop(restore(&source, &measured).unwrap());
    collect(&measured);
    let peak = measured.peak_reserved_bytes();
    let pool = AllocationBudget::new(peak - 1);
    assert!(restore(&source, &pool).is_err());
    // Refusal happens before EBR initialization, so no collector is needed.
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(peak);
    drop(restore(&source, &pool).unwrap());
    collect(&pool);
}

#[test]
fn canonical_refusal_happens_before_initialization_and_preserves_original_decoder_scope() {
    use norito::core::{DecodeAttemptErrorKind, DecodeBudgetContext};
    let source = fixture();
    let pool = AllocationBudget::new(1 << 24);
    // The shared grammar accepts whitespace and escaped field spelling; the
    // canonical writer compares against the exact original bytes before install.
    for altered in [
        format!(" {source}"),
        source.replacen("\"blocks\"", "\"bl\\u006fcks\"", 1),
        source[..source.len() - 1].to_owned(),
    ] {
        assert!(restore(&altered, &pool).is_err());
        assert_eq!(pool.reserved_bytes(), 0);
    }
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let context = DecodeBudgetContext::new(limits);
    let error = context.with(|| restore(&source, &pool)).err().unwrap();
    let ParameterCellRestoreError::Ingress(ParameterIngressAttemptError::Codec(attempt)) = error
    else {
        panic!("actual original decoder refusal required")
    };
    assert_eq!(attempt.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert!(attempt.local_refusal().is_some());
    assert_eq!(pool.reserved_bytes(), 0);
    let replay = ParameterIngressAttemptError::observe::<()>(|| Err(attempt.into_error().into()))
        .unwrap_err();
    let ParameterIngressAttemptError::Codec(replay) = replay else {
        panic!("replay must classify")
    };
    assert_eq!(replay.kind(), DecodeAttemptErrorKind::Invalid);
    drop(restore(&source, &pool).unwrap());
    collect(&pool);
}
