//! Standalone, dependency-free checks for lifecycle source ordering.
//!
//! Run with the repository-pinned compiler before compiling Core:
//! `rustc --edition 2024 --test <this-file> -o <test-binary>`.

include!("v2_lifecycle_source_test_helpers.rs");
include!("v2_lifecycle_launch_pending_kura_source_tests.rs");

#[test]
fn missing_or_moved_post_dispatch_retry_is_rejected() {
    let source = include_str!("v2_runner/lifecycle_pending_kura.rs");
    let readiness = source_token_position(
        source,
        "let ready_to_finish = match activated.with_runner_runtime(",
    );
    let dispatch = readiness
        + source_token_position(
            &source[readiness..],
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?;",
        );
    let retry = dispatch
        + source_token_position(
            &source[dispatch..],
            "                let _ = retry_exact_output_and_apply_sidecar_admissions(",
        );
    let ready = retry
        + source_token_position(
            &source[retry..],
            "                Ok(executor.ready_to_finish())",
        );
    let mut missing = source.to_owned();
    missing.replace_range(retry..ready, "");
    assert!(
        std::panic::catch_unwind(|| assert_pending_kura_actor_backpressure_contract(&missing))
            .is_err()
    );
    let mut moved = missing;
    moved.insert_str(dispatch, &source[retry..ready]);
    assert!(
        std::panic::catch_unwind(|| assert_pending_kura_actor_backpressure_contract(&moved))
            .is_err()
    );
}

#[test]
fn post_dispatch_remote_wait_is_rejected() {
    let source = include_str!("v2_runner/lifecycle_pending_kura.rs");
    let readiness = source_token_position(
        source,
        "let ready_to_finish = match activated.with_runner_runtime(",
    );
    let dispatch = readiness
        + source_token_position(
            &source[readiness..],
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?;",
        );
    let retry = dispatch
        + source_token_position(
            &source[dispatch..],
            "                let _ = retry_exact_output_and_apply_sidecar_admissions(",
        );
    let mut gated = source.to_owned();
    gated.insert_str(
        retry,
        "                if terminal_exact_output_pending { continue; }\n",
    );
    assert!(
        std::panic::catch_unwind(|| assert_pending_kura_actor_backpressure_contract(&gated))
            .is_err()
    );
}
