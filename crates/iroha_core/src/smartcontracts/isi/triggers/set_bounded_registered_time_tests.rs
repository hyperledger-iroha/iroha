// Authored source controls only; current canonical execution remains unclaimed.
fn registration_metadata(height: u64, at_ms: u64) -> Metadata {
    let mut metadata = Metadata::default();
    metadata.insert(
        "__registered_block_height".parse().unwrap(),
        dm::Json::from(height),
    );
    metadata.insert("__registered_at_ms".parse().unwrap(), dm::Json::from(at_ms));
    metadata
}

fn scheduled_invocation(entrypoint: &str) -> ContractInvocation {
    use iroha_data_model::{
        smart_contract::{
            ContractAddress,
            entrypoint::{EntrypointArgumentRecordV1, EntrypointValueAtomV1},
        },
        transaction::executable::ContractArgumentRecord,
    };
    // Same real argument-record construction as the retained SDK constructor's
    // native fixture. This is not a claim about a deployed business schema.
    let arguments = norito::encode_canonical(&EntrypointArgumentRecordV1 {
        schema_hash: [7; 32],
        atoms: vec![EntrypointValueAtomV1::Unit],
    })
    .unwrap();
    ContractInvocation {
        contract_address: ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .unwrap(),
            &checked_account_id(),
            7,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .unwrap(),
        expected_code_hash: iroha_crypto::Hash::new(b"retained scheduled producer fixture"),
        entrypoint: entrypoint.to_owned(),
        arguments: Some(ContractArgumentRecord::try_new(arguments).unwrap()),
    }
}

fn registered_time_set(id: &str, invocation: ContractInvocation, metadata: Metadata) -> Set {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let mut action = SpecializedAction::new(
        dm::Executable::Batch(ConstVec::from(vec![ExecutableBatchItem::ContractCall(
            invocation,
        )])),
        dm::Repeats::Exactly(1),
        checked_account_id(),
        dm::TimeEventFilter(ExecutionTime::Schedule(
            Schedule::starting_at(std::time::Duration::from_millis(10_000))
                .with_period(std::time::Duration::from_millis(5_000)),
        )),
    )
    .unwrap();
    action.metadata = metadata;
    action.retry_policy = Some(dm::TimeTriggerRetryPolicy {
        max_retries: std::num::NonZeroU32::new(3).unwrap(),
        retry_after_ms: NonZeroU64::new(5_000).unwrap(),
    });
    tx.add_time_trigger(SpecializedTrigger::new(id.parse().unwrap(), action))
        .unwrap();
    tx.apply();
    block.commit();
    set
}

#[test]
fn registered_time_original_batch_and_numeric_metadata_preserve_exact_action_bytes() {
    for (id, entrypoint) in [
        ("fut01", "execute_scheduled_payment"),
        ("fut02", "execute_scheduled_payment"),
        ("fut03", "execute_scheduled_payment"),
        ("lock03", "expire_scheduled_lock"),
    ] {
        for (height, time) in [(0, 0), (3_804, 1_792_800_000_000), (u64::MAX, u64::MAX)] {
            let set = registered_time_set(
                id,
                scheduled_invocation(entrypoint),
                registration_metadata(height, time),
            );
            let view = set.view();
            let original = view.triggers_iter().next().unwrap();
            let expected = norito::encode_canonical(original.action()).unwrap();
            let pool = iroha_allocation::AllocationBudget::new(65_536);
            view.visit_borrowed_trigger_sources(|source| {
                let source = source.unwrap();
                let before_retry = source.retry_state();
                let frame = source
                    .try_prepare_bounded_time_frame(&pool, bounded_frame_limits())
                    .unwrap();
                assert_eq!(frame.as_bytes(), expected);
                assert_eq!(source.retry_state(), before_retry);
                assert_eq!(source.repeats(), dm::Repeats::Exactly(1));
                assert_eq!(source.retry_policy().unwrap().max_retries.get(), 3);
                assert_eq!(source.retry_policy().unwrap().retry_after_ms.get(), 5_000);
                assert_eq!(
                    source
                        .metadata()
                        .get::<str>("__registered_block_height")
                        .unwrap()
                        .get(),
                    &height.to_string()
                );
                assert_eq!(
                    source
                        .metadata()
                        .get::<str>("__registered_at_ms")
                        .unwrap()
                        .get(),
                    &time.to_string()
                );
                assert_eq!(pool.reserved_bytes(), frame.backing_bytes());
                drop(frame);
                assert_eq!(pool.reserved_bytes(), 0);
                Ok(core::ops::ControlFlow::Continue(()))
            })
            .unwrap();
            assert_eq!(
                norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn registered_metadata_rejects_quoted_negative_fractional_and_extra_values_before_payload() {
    let set = supported_time_set(false, true, false);
    let view = set.view();
    let mut variants = Vec::new();
    for value in [
        dm::Json::new("7"),
        dm::Json::new(-1_i64),
        dm::Json::new(1.5_f64),
        dm::Json::new(true),
        dm::Json::default(),
    ] {
        let mut metadata = registration_metadata(7, 10_000);
        metadata.insert("__registered_at_ms".parse().unwrap(), value);
        variants.push(metadata);
    }
    let mut partial = Metadata::default();
    partial.insert("__registered_at_ms".parse().unwrap(), dm::Json::from(1_u64));
    variants.push(partial);
    let mut extra = registration_metadata(1, 2);
    extra.insert("extra".parse().unwrap(), dm::Json::from(3_u64));
    variants.push(extra);
    let mut wrong_name = Metadata::default();
    wrong_name.insert("__registered_at_ms".parse().unwrap(), dm::Json::from(1_u64));
    wrong_name.insert("wrong_height".parse().unwrap(), dm::Json::from(2_u64));
    variants.push(wrong_name);
    view.visit_borrowed_trigger_sources(|source| {
        let source = source.unwrap();
        for metadata in &variants {
            let mut probe = source_with_probe(source);
            probe.metadata = metadata;
            assert_source_frame_refuses(&probe);
        }
        Ok(core::ops::ControlFlow::Continue(()))
    })
    .unwrap();
}

#[test]
fn scheduled_batch_refuses_multiple_calls_absent_empty_arguments_and_bad_entrypoints() {
    use iroha_data_model::transaction::executable::ContractArgumentRecord;
    let base = scheduled_invocation("execute_scheduled_payment");
    let mut candidates = vec![ExecutableRef::Batch(ConstVec::from(vec![
        ExecutableBatchItem::ContractCall(base.clone()),
        ExecutableBatchItem::ContractCall(base.clone()),
    ]))];
    for arguments in [None, Some(ContractArgumentRecord::try_new(vec![]).unwrap())] {
        let mut call = base.clone();
        call.arguments = arguments;
        candidates.push(ExecutableRef::Batch(ConstVec::from(vec![
            ExecutableBatchItem::ContractCall(call),
        ])));
    }
    for entrypoint in [
        "".to_owned(),
        " padded".to_owned(),
        "padded ".to_owned(),
        "a".repeat(256),
        "\u{2003}main".to_owned(),
    ] {
        let mut call = base.clone();
        call.entrypoint = entrypoint;
        candidates.push(ExecutableRef::Batch(ConstVec::from(vec![
            ExecutableBatchItem::ContractCall(call),
        ])));
    }
    let set = supported_time_set(false, true, false);
    let view = set.view();
    view.visit_borrowed_trigger_sources(|source| {
        for executable in &candidates {
            let mut probe = source_with_probe(source.unwrap());
            probe.executable_source = executable;
            assert_source_frame_refuses(&probe);
        }
        Ok(core::ops::ControlFlow::Continue(()))
    })
    .unwrap();
}

#[test]
fn scheduled_argument_wire_cap_and_entrypoint_byte_boundary_preserve_original_bytes() {
    use iroha_data_model::transaction::executable::{
        ContractArgumentRecord, MAX_CONTRACT_ARGUMENT_RECORD_BYTES,
    };
    for entrypoint in ["x".repeat(255), "é".repeat(127) + "x"] {
        let mut call = scheduled_invocation(&entrypoint);
        // Opaque signed outer record boundary. Inner schema validity is deferred.
        call.arguments = Some(
            ContractArgumentRecord::try_new(vec![0xA5; MAX_CONTRACT_ARGUMENT_RECORD_BYTES])
                .unwrap(),
        );
        let set = registered_time_set("argument_boundary", call, registration_metadata(1, 2));
        let view = set.view();
        let expected =
            norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap();
        let pool = iroha_allocation::AllocationBudget::new(2 * MAX_CONTRACT_ARGUMENT_RECORD_BYTES);
        view.visit_borrowed_trigger_sources(|source| {
            let frame = source
                .unwrap()
                .try_prepare_bounded_time_frame(
                    &pool,
                    norito::core::EncodeWorkLimits {
                        max_frame_bytes: 2 * MAX_CONTRACT_ARGUMENT_RECORD_BYTES,
                        max_byte_visits: 100_000_000,
                        ..bounded_frame_limits()
                    },
                )
                .unwrap();
            assert_eq!(frame.as_bytes(), expected);
            drop(frame);
            assert_eq!(pool.reserved_bytes(), 0);
            Ok(core::ops::ControlFlow::Continue(()))
        })
        .unwrap();
    }
}

#[test]
fn registered_frame_keeps_its_charge_and_original_bytes_after_source_view_drops() {
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    let (frame, expected) = {
        let set = registered_time_set(
            "lifetime",
            scheduled_invocation("expire_scheduled_lock"),
            registration_metadata(3, 4),
        );
        let view = set.view();
        let expected =
            norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap();
        let mut owned = None;
        view.visit_borrowed_trigger_sources(|source| {
            owned = Some(
                source
                    .unwrap()
                    .try_prepare_bounded_time_frame(&pool, bounded_frame_limits())
                    .unwrap(),
            );
            Ok(core::ops::ControlFlow::Continue(()))
        })
        .unwrap();
        (owned.unwrap(), expected)
    };
    assert_eq!(frame.as_bytes(), expected);
    assert_eq!(pool.reserved_bytes(), frame.backing_bytes());
    drop(frame);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn registered_frame_resource_refusal_keeps_store_and_retry_policy_unchanged() {
    let set = registered_time_set(
        "refusal",
        scheduled_invocation("execute_scheduled_payment"),
        registration_metadata(3, 4),
    );
    let view = set.view();
    let expected = norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap();
    let pool = iroha_allocation::AllocationBudget::new(65_536);
    view.visit_borrowed_trigger_sources(|source| {
        let source = source.unwrap();
        let retry = source.retry_state();
        let refusal = source.try_prepare_bounded_time_frame(
            &pool,
            norito::core::EncodeWorkLimits {
                max_frame_bytes: 1,
                ..bounded_frame_limits()
            },
        );
        assert!(refusal.is_err());
        assert_eq!(source.retry_state(), retry);
        assert_eq!(pool.peak_reserved_bytes(), 0);
        Ok(core::ops::ControlFlow::Continue(()))
    })
    .unwrap();
    assert_eq!(
        norito::encode_canonical(view.triggers_iter().next().unwrap().action()).unwrap(),
        expected
    );
}
