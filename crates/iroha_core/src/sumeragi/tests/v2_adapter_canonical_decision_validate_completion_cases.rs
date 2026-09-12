#[cfg(feature = "bls")]
fn continue_canonical_decision_validate_cold_fixture(
    safety: &TempDir,
    storage: &TempDir,
    body_store: crate::sumeragi::v2_body_store::V2BodyStore,
    verified: &VerifiedHeightContext,
    marker: u8,
) {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lifecycle_coordinator::{
            LifecycleWorkClass, ProductionCompletionDispatchV1,
            ProductionLifecycleLiveClockActivationPermitV1, ReadyValidateSuccessorDispatchV1,
        },
        v2_worker::LifecycleCompletionTakeV1,
    };
    use std::sync::Arc;

    let catalog = body_store
        .recovery_catalog()
        .expect("one durable Decision body");
    assert_eq!(catalog.len(), 1);
    let (_, (_, receipt)) = catalog
        .first_key_value()
        .expect("exact durable body receipt");
    let expected_body = body_store
        .load_canonical_wire(receipt)
        .expect("original signed Decision body");
    let expected_subject = receipt.subject();
    assert!(body_store.validated_recovery_catalog().is_empty());
    let context = verified.context.clone();
    let proofs = verified.proofs_of_possession.clone();
    drop(body_store);
    let mut body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
        storage.path().join("body"),
        context.clone(),
    )
    .expect("cold-open the original durable-only body store");
    body_store
        .revalidate_recovered_markers(|_| -> Result<wire::ExecutionCommitment, String> {
            panic!("the canonical Ready Validate cut has no completed semantic marker")
        })
        .expect("confirm the cold body store has no outcome to replay");
    let (same_context, keys, _) = authenticated_context();
    assert_eq!(same_context, context);
    let signer = &keys[0];
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let before = std::fs::read(&ledger_path).expect("canonical three-row Validate crash prefix");
    let startup = reopen_authenticated_decision_startup(safety, &context, proofs.clone(), marker)
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("reauthenticate actual Decision: {error}"));
    let mut owner = startup
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &storage.path().join("ledger"),
            &storage.path().join("serve"),
            body_store
                .into_revalidated_startup()
                .expect("seal exact body without marker"),
            signer,
        )
        .unwrap_or_else(|error| panic!("normal owner reopens canonical Validate: {error}"));
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    let validate = 3;
    let snapshot = owner
        .active_body_owner_before_decision_cold_for_test(validate, LifecycleWorkClass::Validate);
    assert_eq!(
        std::fs::read(&ledger_path).expect("canonical Validate ledger after open"),
        before
    );
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let guard = ConsensusOutputGuard::isolated();
    let wal_path = safety.path().join("authenticated-fifo-safety.wal");
    let (mut executor, mut planner_io, gate, _ordinals) = owner
        .bind_recovered_cancelled_body_executor_for_test(
            &wal_path,
            &mut services,
            Arc::clone(&guard),
            0,
        );
    owner.assert_active_body_owner_after_decision_cold_for_test(
        &snapshot,
        LifecycleWorkClass::Validate,
    );
    executor.assert_cold_decision_protection_for_test(expected_subject, false);
    executor
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            Instant::now(),
        )
        .expect("activate canonical recovered runtime");
    crate::sumeragi::v2_worker::tests::install_active_tag_for_test(
        &mut services,
        executor.current_tag(),
    );
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(&mut services, signer);
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("queue real canonical Decision Validate"),
        ProductionCompletionDispatchV1::ValidateQueued { ordinal: validate }
    );
    planner_io.activate_one_lifecycle_validate();
    assert_eq!(
        planner_io.execute_held_lifecycle_validate_fixture(
            execution_commitment(marker),
            Arc::clone(&guard)
        ),
        1
    );
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("actual physical canonical validation")
    {
        LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("the only worker result must belong to Validate"),
    };
    let successor = owner.publish_validate_successor_for_retry_test(completion, validate, false);
    let published = owner
        .dispatch_ready_validate_successor_for_test(&mut services, &mut executor, successor, 0)
        .expect("canonical recovered Validate must publish its typed Apply");
    let ReadyValidateSuccessorDispatchV1::Resolved(
        ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child_ordinal,
            child: LifecycleWorkClass::Apply,
        },
    ) = published
    else {
        panic!("actual current Commit must advance Validate to Apply")
    };
    assert_eq!(parent_ordinal, validate);
    executor.assert_cold_decision_protection_for_test(expected_subject, true);
    assert_eq!(owner.apply_ordinals_for_retry_test(), vec![child_ordinal]);
    let apply = owner
        .active_body_owner_before_decision_cold_for_test(child_ordinal, LifecycleWorkClass::Apply);
    let applied_prefix = std::fs::read(&ledger_path).expect("real canonical linked Apply prefix");
    planner_io.detach(&mut services);
    drop(services);
    drop(owner);
    drop(executor);
    drop(gate);
    let mut body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
        storage.path().join("body"),
        context.clone(),
    )
    .expect("reopen exact body after completed Validate");
    let mut revalidated = 0;
    body_store
        .revalidate_recovered_markers(|body| {
            assert_eq!(
                body.encode_wire().expect("canonical second-restart body"),
                expected_body
            );
            revalidated += 1;
            Ok::<_, String>(execution_commitment(marker))
        })
        .expect("revalidate the actual durable successful marker");
    assert_eq!(revalidated, 1);
    let startup = reopen_authenticated_decision_startup(safety, &context, proofs, marker)
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("reauthenticate Apply WAL: {error}"));
    let mut owner = startup
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &storage.path().join("ledger"),
            &storage.path().join("serve"),
            body_store
                .into_revalidated_startup()
                .expect("seal real semantic success"),
            signer,
        )
        .unwrap_or_else(|error| panic!("normal owner reopens physically produced Apply: {error}"));
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
    owner.assert_active_body_owner_after_decision_cold_for_test(&apply, LifecycleWorkClass::Apply);
    assert_eq!(owner.apply_ordinals_for_retry_test(), vec![child_ordinal]);
    assert_eq!(
        std::fs::read(&ledger_path).expect("canonical Apply ledger after second restart"),
        applied_prefix
    );
}
