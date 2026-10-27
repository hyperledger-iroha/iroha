// Actual Register-path controls; registration policy itself is unchanged.

fn applying_time_register_fixture(id: &str, arguments: Option<Vec<u8>>) -> Trigger {
    use iroha_data_model::{
        smart_contract::ContractAddress,
        transaction::executable::{
            ContractArgumentRecord, ContractInvocation, ExecutableBatchItem,
        },
    };
    let executable = arguments.map_or_else(
        || Executable::Instructions(Vec::<InstructionBox>::new().into()),
        |arguments| Executable::Batch(vec![ExecutableBatchItem::ContractCall(ContractInvocation {
            contract_address: ContractAddress::derive(
                &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E".parse().unwrap(),
                &ALICE_ID,
                7,
                iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            ).unwrap(),
            expected_code_hash: iroha_crypto::Hash::new(b"applying Time registration fixture"),
            entrypoint: "execute_scheduled_payment".to_owned(),
            arguments: Some(ContractArgumentRecord::try_new(arguments).unwrap()),
        })].into()),
    );
    let mut action = Action::new(
        executable,
        Repeats::Exactly(1),
        ALICE_ID.clone(),
        TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
            Duration::from_millis(10_000),
        ))),
    )
    .unwrap();
    action.retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: core::num::NonZeroU32::new(3).unwrap(),
        retry_after_ms: NonZeroU64::new(5_000).unwrap(),
    });
    Trigger::new(id.parse().unwrap(), action)
}

fn applying_time_register_owner(stx: &mut crate::state::StateTransaction<'_, '_>) {
    Register::domain(Domain::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
    ))
    .execute(&ALICE_ID, stx)
    .unwrap();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, stx)
        .unwrap();
}

fn applying_time_encode_limits() -> norito::core::EncodeWorkLimits {
    norito::core::EncodeWorkLimits {
        max_frame_bytes: 65_536,
        max_byte_visits: 10_000_000,
        max_operations: 1_000_000,
        max_depth: 128,
    }
}

fn applying_time_wire_scanner(
    pool: &iroha_allocation::AllocationBudget,
) -> iroha_data_model::block::execution_output::BoundedActionFrameScannerV1<'_> {
    use iroha_data_model::block::execution_output::{
        ActionScannerLimitsV1, BoundedActionFrameScannerV1,
    };
    BoundedActionFrameScannerV1::try_new(
        pool,
        ActionScannerLimitsV1 {
            max_frame_bytes: 65_536,
            max_total_frame_bytes: 1_000_000,
            max_frames: 16,
            max_work: 10_000_000,
            max_fields: 100_000,
            max_sequence_elements: 1_000_000,
            max_depth: 32,
        },
        norito::core::DecodeLimits::new(100_000, 2_000_000, 100_000, 2_000_000, 32),
    )
    .unwrap()
}

#[test]
fn applying_time_register_capture_preserves_real_registration_metadata_action_and_retry() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentRecordV1, EntrypointValueAtomV1,
    };
    let valid_inner = norito::encode_canonical(&EntrypointArgumentRecordV1 {
        // A real canonical record, deliberately not an authenticated deployed schema.
        schema_hash: [7; 32],
        atoms: vec![EntrypointValueAtomV1::Unit],
    })
    .unwrap();
    for arguments in [None, Some(valid_inner)] {
        let state = State::new(
            component_trigger_world(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            42,
            0,
        ));
        let mut stx = block.transaction();
        applying_time_register_owner(&mut stx);
        let trigger = applying_time_register_fixture("actual_register_capture", arguments);
        let id = trigger.id().clone();
        let original_executable = trigger.action().executable().clone();
        Register::trigger(trigger)
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let registered = stx
            .world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == &id)
            .unwrap();
        assert_eq!(registered.action().executable(), &original_executable);
        assert_eq!(
            registered
                .action()
                .metadata()
                .get::<str>("__registered_block_height")
                .unwrap()
                .get(),
            "1"
        );
        assert_eq!(
            registered
                .action()
                .metadata()
                .get::<str>("__registered_at_ms")
                .unwrap()
                .get(),
            "42"
        );
        let expected = norito::encode_canonical(registered.action()).unwrap();
        assert!(stx.world.triggers.set_time_trigger_retry_state(
            &id,
            Some(super::specialized::TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 52,
            })
        ));
        let pool = iroha_allocation::AllocationBudget::new(1_000_000);
        let captured = stx
            .world
            .triggers
            .capture_applying_time_source(
                &id,
                &mut set::ApplyingTimeSourceWork::new(1_000_000),
                &pool,
                applying_time_encode_limits(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(captured.action_bytes(), expected);
        let address = captured.action_bytes().as_ptr();
        let mut scanner = applying_time_wire_scanner(&pool);
        let checked = captured
            .scan_canonical_arguments(&mut scanner)
            .unwrap_or_else(|failure| {
                panic!(
                    "real registered source wire refused: {:?}",
                    failure.failure()
                )
            });
        assert_eq!(checked.source().action_bytes().as_ptr(), address);
        assert_eq!(checked.source().action_bytes(), expected);
        assert_eq!(checked.source().retry_state(), Some((1, 52)));
        assert!(checked.source().belongs_to(&pool));
        assert!(checked.semantics().has_retry_policy);
        drop(checked);
        drop(scanner);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn applying_time_register_keeps_deferred_batch_policy_but_canonical_capture_refuses_bad_inner_wire()
{
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        42,
        0,
    ));
    let mut stx = block.transaction();
    applying_time_register_owner(&mut stx);
    let trigger = applying_time_register_fixture("bad_deferred_record", Some(vec![0xA5; 64]));
    let id = trigger.id().clone();
    // Existing Register treats this Batch record as deferred bytes. The new
    // scanner must not silently replace that registration policy or normalize it.
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let expected = norito::encode_canonical(
        stx.world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == &id)
            .unwrap()
            .action(),
    )
    .unwrap();
    let pool = iroha_allocation::AllocationBudget::new(1_000_000);
    let captured = stx
        .world
        .triggers
        .capture_applying_time_source(
            &id,
            &mut set::ApplyingTimeSourceWork::new(1_000_000),
            &pool,
            applying_time_encode_limits(),
        )
        .unwrap()
        .unwrap();
    let address = captured.action_bytes().as_ptr();
    let mut scanner = applying_time_wire_scanner(&pool);
    let retained_before = pool.reserved_bytes();
    let failure = match captured.scan_canonical_arguments(&mut scanner) {
        Ok(_) => panic!("malformed deferred original must not receive canonical wire marker"),
        Err(original) => original,
    };
    assert_eq!(failure.id_bytes(), b"bad_deferred_record");
    assert_eq!(failure.retry_state(), None);
    assert_eq!(failure.frame().unwrap().as_bytes().as_ptr(), address);
    assert_eq!(failure.frame().unwrap().as_bytes(), expected);
    assert!(failure.frame().unwrap().belongs_to(&pool));
    assert!(matches!(
        failure.failure(),
        iroha_data_model::block::execution_output::ActionFrameScanFailureV1::Codec(_)
    ));
    assert_eq!(pool.reserved_bytes(), retained_before);
    let source = failure.into_source().unwrap();
    assert_eq!(source.action_bytes().as_ptr(), address);
    assert_eq!(
        norito::encode_canonical(
            stx.world
                .triggers
                .triggers_iter()
                .find(|row| row.id() == &id)
                .unwrap()
                .action()
        )
        .unwrap(),
        expected
    );
    drop(source);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn applying_time_register_resource_refusal_leaves_real_registered_action_unchanged() {
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        42,
        0,
    ));
    let mut stx = block.transaction();
    applying_time_register_owner(&mut stx);
    let trigger = applying_time_register_fixture("registered_resource", None);
    let id = trigger.id().clone();
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let before = norito::encode_canonical(
        stx.world
            .triggers
            .triggers_iter()
            .find(|row| row.id() == &id)
            .unwrap()
            .action(),
    )
    .unwrap();
    let pool = iroha_allocation::AllocationBudget::new(1_000_000);
    assert!(matches!(
        stx.world.triggers.capture_applying_time_source(
            &id,
            &mut set::ApplyingTimeSourceWork::new(1_000_000),
            &pool,
            norito::core::EncodeWorkLimits {
                max_operations: 0,
                ..applying_time_encode_limits()
            },
        ),
        Err(set::ApplyingTimeSourceError::Frame(
            iroha_data_model::block::execution_output::ActionFrameProductionFailureV1::Codec(
                norito::core::BoundedEncodeWorkError::Refused(
                    norito::core::EncodeWorkRefusal::Operations
                )
            )
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(
        norito::encode_canonical(
            stx.world
                .triggers
                .triggers_iter()
                .find(|row| row.id() == &id)
                .unwrap()
                .action()
        )
        .unwrap(),
        before
    );
}
