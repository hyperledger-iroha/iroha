// Concrete applying-store controls. The helper fixtures here deliberately use
// typed insertion; actual Register coverage lives in separate registration tests.

#[test]
fn applying_time_source_refuses_before_cursor_next_and_full_key_comparison() {
    let id: dm::TriggerId = "source_work".parse().unwrap();
    let set = registered_time_set(
        "source_work",
        scheduled_invocation("execute_scheduled_payment"),
        registration_metadata(7, 11),
    );
    let mut block = set.block();
    let tx = block.transaction();
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    let setup = 29 * u64::from(usize::BITS) + 44;
    let next = 17 * u64::from(usize::BITS) + 26;
    for (limit, expected) in [
        (0, [0, 0, 0, 0]),
        (setup + 2, [1, 0, 0, 0]),
        (
            setup + 2 + next + 7 + 2 * "source_work".len() as u64 - 1,
            [1, 1, 0, 0],
        ),
    ] {
        let mut work = ApplyingTimeSourceWork::new(limit);
        assert!(matches!(
            tx.capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits()),
            Err(ApplyingTimeSourceError::Work(
                TriggerContractError::WorkLimit
            ))
        ));
        assert_eq!(work.observations(), expected);
        assert_eq!(pool.reserved_bytes(), 0);
        assert!(matches!(
            tx.capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits()),
            Err(ApplyingTimeSourceError::ClosedAfterWorkRefusal)
        ));
        assert_eq!(work.observations(), expected);
    }
}

#[test]
fn applying_time_source_empty_terminal_step_is_charged_and_budget_is_cumulative() {
    let set = Set::default();
    let mut block = set.block();
    let tx = block.transaction();
    let pool = iroha_allocation::AllocationBudget::new(0);
    let id = "missing".parse().unwrap();
    let one_scan = (29 + 17) * u64::from(usize::BITS) + 44 + 26 + 2;
    let mut work = ApplyingTimeSourceWork::new(one_scan);
    assert!(
        tx.capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits())
            .unwrap()
            .is_none()
    );
    assert_eq!(work.observations(), [1, 1, 0, 0]);
    assert!(matches!(
        tx.capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits()),
        Err(ApplyingTimeSourceError::Work(
            TriggerContractError::WorkLimit
        ))
    ));
    assert_eq!(work.observations(), [1, 1, 0, 0]);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn applying_time_source_retains_original_bytes_pool_and_retry_across_overlay_rollback() {
    let id: dm::TriggerId = "overlay_source".parse().unwrap();
    let set = registered_time_set(
        "overlay_source",
        scheduled_invocation("expire_scheduled_lock"),
        registration_metadata(7, 11),
    );
    let expected = {
        let view = set.view();
        norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap()
    };
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    let other_pool = iroha_allocation::AllocationBudget::new(65_536);
    let mut block = set.block();
    let captured = {
        let mut tx = block.transaction();
        assert!(tx.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: 2,
                next_retry_at_ms: 41,
            })
        ));
        let mut work = ApplyingTimeSourceWork::new(1_000_000);
        let captured = tx
            .capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits())
            .unwrap()
            .unwrap();
        assert!(tx.set_time_trigger_retry_state(&id, None));
        assert_eq!(captured.retry_state(), Some((2, 41)));
        // Dropping this real applying transaction restores its original roots.
        captured
    };
    assert_eq!(captured.id_bytes(), b"overlay_source");
    assert_eq!(captured.action_bytes(), expected);
    assert!(captured.belongs_to(&pool));
    assert!(!captured.belongs_to(&other_pool));
    assert_eq!(
        pool.reserved_bytes(),
        expected.len() + b"overlay_source".len()
    );
    {
        let tx = block.transaction();
        let restored = tx
            .capture_applying_time_source(
                &id,
                &mut ApplyingTimeSourceWork::new(1_000_000),
                &pool,
                bounded_frame_limits(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(restored.retry_state(), None);
        assert_eq!(restored.action_bytes(), captured.action_bytes());
    }
    assert_eq!(captured.retry_state(), Some((2, 41)));
    drop(captured);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn applying_time_source_pool_and_original_encoder_refusals_release_partial_id_backing() {
    let id: dm::TriggerId = "pool_source".parse().unwrap();
    let set = registered_time_set(
        "pool_source",
        scheduled_invocation("execute_scheduled_payment"),
        registration_metadata(7, 11),
    );
    let mut block = set.block();
    let tx = block.transaction();
    let no_backing = iroha_allocation::AllocationBudget::new(0);
    assert!(matches!(
        tx.capture_applying_time_source(
            &id,
            &mut ApplyingTimeSourceWork::new(1_000_000),
            &no_backing,
            bounded_frame_limits(),
        ),
        Err(ApplyingTimeSourceError::IdBacking(_))
    ));
    let id_only = iroha_allocation::AllocationBudget::new(b"pool_source".len());
    assert!(matches!(
        tx.capture_applying_time_source(
            &id,
            &mut ApplyingTimeSourceWork::new(1_000_000),
            &id_only,
            bounded_frame_limits(),
        ),
        Err(ApplyingTimeSourceError::Frame(
            iroha_data_model::block::execution_output::ActionFrameProductionFailureV1::Allocation(
                _
            )
        ))
    ));
    assert_eq!(id_only.reserved_bytes(), 0);
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    assert!(matches!(
        tx.capture_applying_time_source(
            &id,
            &mut ApplyingTimeSourceWork::new(1_000_000),
            &pool,
            norito::core::EncodeWorkLimits {
                max_operations: 0,
                ..bounded_frame_limits()
            },
        ),
        Err(ApplyingTimeSourceError::Frame(
            iroha_data_model::block::execution_output::ActionFrameProductionFailureV1::Codec(
                norito::core::BoundedEncodeWorkError::Refused(
                    norito::core::EncodeWorkRefusal::Operations
                )
            )
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    let captured = tx
        .capture_applying_time_source(
            &id,
            &mut ApplyingTimeSourceWork::new(1_000_000),
            &pool,
            bounded_frame_limits(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(captured.retry_state(), None);
}

#[test]
fn applying_time_source_refuses_loaded_ivm_before_original_contract_lookup_or_projection() {
    let id: dm::TriggerId = "ivm_source".parse().unwrap();
    let set = registered_time_set(
        "ivm_source",
        scheduled_invocation("execute_scheduled_payment"),
        registration_metadata(7, 11),
    );
    let mut block = set.block();
    let mut tx = block.transaction();
    // Deliberately no matching original code: the generic projection would
    // perform a contracts lookup and return None. This seam must refuse first.
    tx.time_triggers.get_mut(&id).unwrap().executable =
        ExecutableRef::Ivm(HashOf::new(&IvmBytecode::from_compiled(vec![1, 2, 3])));
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    let mut work = ApplyingTimeSourceWork::new(1_000_000);
    assert!(matches!(
        tx.capture_applying_time_source(&id, &mut work, &pool, bounded_frame_limits()),
        Err(ApplyingTimeSourceError::LoadedIvm)
    ));
    assert_eq!(work.observations(), [1, 1, 1, 0]);
    assert_eq!(pool.reserved_bytes(), 0);
}
