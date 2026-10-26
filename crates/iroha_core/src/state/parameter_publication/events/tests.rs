//! Original event graph and escaped JSON lifetime, finite and physical refusal.

use super::*;
use crate::test_allocations::{allocations_during, refuse_one_layout_during};
use iroha_primitives::json::Json;

fn custom(number: u64) -> CustomParameter {
    CustomParameter::new(
        "explicit_long_parameter_identifier_backing_for_event_custody"
            .parse()
            .unwrap(),
        Json::new(number),
    )
}
#[test]
fn complete_event_retains_name_and_shared_json_until_last_reader() {
    let old = custom(1);
    let new = custom(2);
    let pool = AllocationBudget::new(1 << 20);
    let event = prepare(
        ParameterValue::Custom(&old),
        ParameterValue::Custom(&new),
        &pool,
    )
    .unwrap();
    assert!(event.belongs_to(&pool));
    let mut late = None;
    assert_eq!(allocations_during(|| late = Some(event.clone())), 0);
    let DataEvent::Configuration(ConfigurationEvent::Changed(change)) = event.as_ref() else {
        panic!("configuration event required")
    };
    assert_eq!(change.old_value, Parameter::Custom(old));
    assert_eq!(change.new_value, Parameter::Custom(new));
    let Parameter::Custom(next) = &change.new_value else {
        unreachable!()
    };
    let escaped_json = next.payload().clone();
    let all = pool.reserved_bytes();
    drop(event);
    assert_eq!(pool.reserved_bytes(), all);
    drop(late);
    assert!(pool.reserved_bytes() > 0 && pool.reserved_bytes() < all);
    assert_eq!(escaped_json.get(), "2");
    drop(escaped_json);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn exact_name_allocator_refusal_refunds_complete_private_batch() {
    let old = custom(1);
    let new = custom(2);
    let pool = AllocationBudget::new(1 << 20);
    let layout = old.id().name().admission_clone_layout().unwrap();
    let (result, refused) = refuse_one_layout_during(layout, || {
        crate::pipeline_gas_policy::ParameterIngressAttemptError::observe(|| {
            prepare(
                ParameterValue::Custom(&old),
                ParameterValue::Custom(&new),
                &pool,
            )
        })
    });
    assert!(refused);
    let crate::pipeline_gas_policy::ParameterIngressAttemptError::Codec(actual) =
        result.unwrap_err()
    else {
        panic!("actual Name allocator refusal must preserve codec observer")
    };
    assert_eq!(
        actual.kind(),
        norito::core::DecodeAttemptErrorKind::Allocator
    );
    assert_eq!(pool.reserved_bytes(), 0);
    drop(
        prepare(
            ParameterValue::Custom(&old),
            ParameterValue::Custom(&new),
            &pool,
        )
        .unwrap(),
    );
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn finite_first_and_late_refusal_releases_all_event_storage_before_retry() {
    let old = custom(1);
    let new = custom(2);
    let zero = AllocationBudget::new(0);
    assert_eq!(
        allocations_during(|| assert!(
            prepare(
                ParameterValue::Custom(&old),
                ParameterValue::Custom(&new),
                &zero
            )
            .is_err()
        )),
        0
    );
    let measured = AllocationBudget::new(1 << 20);
    drop(
        prepare(
            ParameterValue::Custom(&old),
            ParameterValue::Custom(&new),
            &measured,
        )
        .unwrap(),
    );
    let peak = measured.peak_reserved_bytes();
    let pool = AllocationBudget::new(peak - 1);
    assert!(
        prepare(
            ParameterValue::Custom(&old),
            ParameterValue::Custom(&new),
            &pool
        )
        .is_err()
    );
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(peak);
    drop(
        prepare(
            ParameterValue::Custom(&old),
            ParameterValue::Custom(&new),
            &pool,
        )
        .unwrap(),
    );
    assert_eq!(pool.reserved_bytes(), 0);
}
