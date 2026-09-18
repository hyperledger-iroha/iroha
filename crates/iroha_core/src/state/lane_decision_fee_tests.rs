// Actual native economic execution with funded canonical XOR and signed maxima.
// Direct authority fees are a local burn, not a sponsor relay receipt. These
// scratch-only controls do not publish economics or acknowledge lane Apply.

#[derive(Clone, Copy)]
struct NativeEconomicDirectFee {
    funding: u32,
    signed_max: u32,
}

fn native_economic_direct_fee_asset(source: &AssetId) -> AssetId {
    let definition = AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .expect("the fixture funds the canonical fee selector accepted by State");
    AssetId::new(definition, source.account().clone())
}

fn assert_native_direct_fee_balance(
    world: &impl WorldReadOnly,
    fee_asset: &AssetId,
    expected: u32,
) {
    let expected = Quantity::from(expected);
    assert_eq!(world.assets().get(fee_asset).unwrap().as_ref(), &expected);
    assert_eq!(
        world
            .asset_definition(fee_asset.definition())
            .unwrap()
            .total_quantity(),
        &expected,
        "direct Nexus settlement burns supply as well as the exact payer bucket",
    );
}

fn assert_native_direct_fee_status(fee_asset: &AssetId, charged_executions: u64) {
    let status = crate::sumeragi::status::nexus_fee_snapshot();
    assert_eq!(status.charged_total, charged_executions);
    assert_eq!(status.charged_via_payer_total, charged_executions);
    assert_eq!(status.charged_via_sponsor_total, 0);
    assert_eq!(status.config_errors_total, 0);
    assert_eq!(status.transfer_failures_total, 0);
    assert!(status.last_error.is_none());
    if charged_executions == 0 {
        assert!(status.last_amount.is_none());
        assert!(status.last_payer.is_none());
    } else {
        assert_eq!(status.last_amount, Some(Quantity::from(5u32)));
        assert_eq!(
            status.last_asset_id,
            Some(fee_asset.definition().canonical_address())
        );
        assert_eq!(
            status.last_payer,
            Some(crate::sumeragi::status::NexusFeePayer::Payer)
        );
        assert_eq!(status.last_payer_id, Some(fee_asset.account().to_string()));
    }
}

fn native_direct_fee_quote(
    fixture: &NativeEconomicFixture,
    group: &super::VerifiedLaneDecisionGroupV1,
    carrier: &BlockHeader,
) -> std::result::Result<crate::executor::FeeAdmissionQuote, crate::executor::NexusFeeAdmissionError>
{
    let TransactionEntrypoint::External(transaction) = &group.body().payload().input.entrypoint
    else {
        panic!("this fee control uses the signed external authority input");
    };
    transaction
        .verify_signature()
        .expect("the charged intent is signature-bound");
    assert!(transaction.fee_payment_intent().sponsor_program().is_none());
    assert_eq!(transaction.fee_payment_intent().charge_limits().len(), 1);
    let limit = &transaction.fee_payment_intent().charge_limits()[0];
    assert_eq!(
        limit.kind(),
        iroha_data_model::transaction::FeeChargeKind::Nexus
    );
    assert_eq!(
        limit.asset_definition_id(),
        native_economic_direct_fee_asset(&fixture.source).definition()
    );
    let view = fixture.native.state.view();
    assert_eq!(view.nexus.fees.base_fee, Quantity::from(2u32));
    assert_eq!(view.nexus.fees.per_instruction_fee, Quantity::from(3u32));
    assert!(view.nexus.fees.per_byte_fee.is_zero());
    assert!(view.nexus.fees.per_gas_unit_fee.is_zero());
    assert_eq!(
        view.nexus.fees.settlement_mode,
        iroha_config::parameters::actual::NexusFeeSettlementMode::Direct
    );
    crate::executor::quote_nexus_fee_admission(
        view.world(),
        &view.nexus,
        &view.pipeline,
        transaction,
        carrier.creation_time_ms,
        carrier.height().get(),
        Some(
            group
                .body()
                .payload()
                .input
                .routing_plan()
                .unwrap()
                .coordinator_route()
                .dataspace_id,
        ),
    )
}

state_test! { sync native_economic_direct_fee_exact_burn_and_event_survive_scratch_replay_without_double_debit
    use iroha_data_model::transaction::FeeChargeKind;
    let fixture = native_economic_fixture_with_fee_policy(
        &[NativeEconomicCase::Transfer(25)], true, None,
        Some(NativeEconomicDirectFee { funding: 20, signed_max: 5 }),
    );
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].body().payload().descriptor.slots.len(), 2);
    let fee_asset = native_economic_direct_fee_asset(&fixture.source);
    assert_ne!(fee_asset.definition(), fixture.source.definition());
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = carrier.clone();
    let quote = native_direct_fee_quote(&fixture, &groups[0], &carrier).unwrap();
    let TransactionEntrypoint::External(transaction) = &groups[0].body().payload().input.entrypoint else { unreachable!() };
    assert_eq!(transaction.fee_payment_intent().charge_limits()[0].max_amount(), &Quantity::from(5u32));
    assert_eq!(quote.charges, vec![crate::executor::FeeChargeBound {
        kind: FeeChargeKind::Nexus,
        asset_definition_id: fee_asset.definition().clone(),
        max_bound: Quantity::from(5u32),
    }]);
    assert_eq!(quote.debit_source, iroha_data_model::nexus::FeeDebitSource::Account(fee_asset.account().clone()));
    assert_eq!(quote.authority_balances.get(&fee_asset), Some(&Quantity::from(20u32)));
    assert!(quote.capacities.is_empty() && quote.relay_leases.is_empty());
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    // The existing reentrant status guard isolates actual production Charged
    // events; it does not intercept, fabricate or mutate the economic result.
    let _status = crate::sumeragi::status::nexus_fee_test_lock().lock().unwrap();
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let prepared = state.prepare_native_batch_on_carrier(header, &groups).unwrap();
    let batch = prepared.batch().clone();
    assert!(prepared.executions()[0].result.is_ok(), "{:?}", prepared.executions()[0].result);
    assert_eq!(batch.groups[0], groups[0].to_wire());
    assert_eq!(prepared.executions()[0].settlement_commitment.tx_count, 1, "shared coordinator/participant roles charge once");
    assert!(prepared.executions()[0].settlement_commitment.nexus_fee_receipts.is_empty(), "Direct authority burn must not synthesize a sponsor relay receipt");
    assert!(prepared.executions()[0].settlement_commitment.native_amx_receipts.is_empty());
    assert_native_direct_fee_balance(&prepared.overlay().world, &fee_asset, 15);
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_native_economic_terminal(prepared.overlay(), &groups[0], carrier.height().get());
    assert_native_fastpq_retained_for_test(prepared.overlay(), prepared.executions());
    for transcript in &prepared.executions()[0].fastpq_transcripts {
        assert_eq!(transcript.entry_hash, Hash::from(groups[0].body().payload().input.entrypoint.hash()));
    }
    assert_native_direct_fee_status(&fee_asset, 1);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert_native_direct_fee_balance(state.view().world(), &fee_asset, 20);
    assert_native_direct_fee_status(&fee_asset, 1);

    let replay = state.replay_lane_decision_batch(&carrier, &batch, &groups).unwrap();
    assert_eq!(replay.batch(), &batch, "fee effects are part of the exact replayed write roots/results");
    assert_native_direct_fee_balance(&replay.overlay().world, &fee_asset, 15);
    assert_eq!(replay.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_native_economic_terminal(replay.overlay(), &groups[0], carrier.height().get());
    // Status records each real scratch execution. It is not a global-commit
    // receipt; the monetary owner remains the disposable overlay in both runs.
    assert_native_direct_fee_status(&fee_asset, 2);
    drop(replay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert_native_direct_fee_balance(state.view().world(), &fee_asset, 20);
}

state_test! { sync native_economic_direct_fee_cap_and_payer_rejections_settle_heads_without_any_debit
    use iroha_data_model::nexus::FeeRejectionCode;
    for (policy, expected_code, reason) in [
        (NativeEconomicDirectFee { funding: 20, signed_max: 4 }, FeeRejectionCode::SignedLimitExceeded, "exceeds signed maximum"),
        (NativeEconomicDirectFee { funding: 4, signed_max: 5 }, FeeRejectionCode::AuthorityPayerInsufficient, "is insufficient"),
    ] {
        let fixture = native_economic_fixture_with_fee_policy(&[NativeEconomicCase::Transfer(25)], true, None, Some(policy));
        let state = &fixture.native.state;
        let groups = native_economic_groups(&fixture);
        let fee_asset = native_economic_direct_fee_asset(&fixture.source);
        let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
        let header = carrier.clone();
        let error = native_direct_fee_quote(&fixture, &groups[0], &carrier).unwrap_err();
        assert_eq!(error.code(), expected_code);
        assert!(error.reason().contains(reason), "{error:?}");
        let before = crate::snapshot::canonical_state_snapshot_hash(state);
        let _status = crate::sumeragi::status::nexus_fee_test_lock().lock().unwrap();
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        let prepared = state.prepare_native_batch_on_carrier(header, &groups)
            .expect("an authenticated admitted input that cannot pay is terminally rejected, not an endlessly invalid carrier");
        let batch = prepared.batch().clone();
        let execution = &prepared.executions()[0];
        let actual = &prepared.executions()[0];
        assert!(actual.result.is_err());
        assert!(format!("{:?}", actual.result).contains(reason), "{:?}", actual.result);
        assert_eq!(execution.source, groups[0].to_wire());
        assert!(execution.settlement_commitment.receipts.is_empty());
        assert!(execution.settlement_commitment.nexus_fee_receipts.is_empty());
        assert!(execution.settlement_commitment.native_amx_receipts.is_empty());
        assert!(actual.fastpq_transcripts.is_empty());
        assert_native_direct_fee_balance(&prepared.overlay().world, &fee_asset, policy.funding);
        assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(100u32));
        assert!(prepared.overlay().world.assets.get(&fixture.destination).is_none());
        assert_native_economic_terminal(prepared.overlay(), &groups[0], carrier.height().get());
        assert_native_direct_fee_status(&fee_asset, 0);
        drop(prepared);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
        let replay = state.replay_lane_decision_batch(&carrier, &batch, &groups).unwrap();
        assert_eq!(replay.batch(), &batch);
        assert_native_economic_terminal(replay.overlay(), &groups[0], carrier.height().get());
        assert_native_direct_fee_balance(&replay.overlay().world, &fee_asset, policy.funding);
        assert_native_direct_fee_status(&fee_asset, 0);
        drop(replay);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    }
}

state_test! { sync native_economic_direct_fee_late_batch_failure_discards_real_burn_with_all_frontiers
    use iroha_model_base::state_path::StatePath;
    let fixture = native_economic_fixture_with_fee_policy(
        &[NativeEconomicCase::Transfer(25)], true, None,
        Some(NativeEconomicDirectFee { funding: 20, signed_max: 5 }),
    );
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let fee_asset = native_economic_direct_fee_asset(&fixture.source);
    let slot = &groups[0].body().payload().descriptor.slots[1];
    let marker: StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
    // This actual late batch guard executes after the transfer and fee burn.
    // Its unrelated preexisting marker must remain while all scratch writes go.
    let existing = norito::encode_canonical(&Hash::new(b"fee-control conflicting application")).unwrap();
    let mut storage = state.world.smart_contract_state.block();
    storage.insert(marker.clone(), existing.clone());
    storage.commit();
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = carrier.clone();
    let _status = crate::sumeragi::status::nexus_fee_test_lock().lock().unwrap();
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let error = state.prepare_native_batch_on_carrier(header, &groups).err().expect("late marker conflict");
    assert!(matches!(error, MergeLedgerCommitError::ExecutionMarkerConflict(_)), "{error}");
    assert_native_direct_fee_status(&fee_asset, 1);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    let view = state.view();
    assert_native_direct_fee_balance(view.world(), &fee_asset, 20);
    assert_eq!(view.world().assets().get(&fixture.source).unwrap().as_ref(), &Quantity::from(100u32));
    assert!(view.world().assets().get(&fixture.destination).is_none());
    assert_eq!(view.world().smart_contract_state.get(&marker), Some(&existing));
    assert!(State::pending_queue_plan_binding_for_execution(&view, &groups[0].body().payload().input.entrypoint,
        &groups[0].body().payload().input.routing_plan().unwrap(), carrier.height().get()).unwrap().is_some());
    for slot in &groups[0].body().payload().descriptor.slots {
        let predecessor = groups[0].contexts().iter().find(|context| Hash::from(context.instance_id().0) == slot.instance_id).unwrap().frozen();
        assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(view.world(), slot.route.lane_id, slot.route.dataspace_id, slot.lane_incarnation).unwrap(),
            (predecessor.predecessor_height, predecessor.predecessor_hash, predecessor.predecessor_applied_global_height));
    }
}

state_test! { sync native_economic_direct_fee_execution_time_exhaustion_rejects_only_later_input_and_settles_both
    let fixture = native_economic_fixture_with_fee_policy(
        &[NativeEconomicCase::Transfer(25), NativeEconomicCase::Transfer(25)], false, None,
        Some(NativeEconomicDirectFee { funding: 9, signed_max: 5 }),
    );
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    assert_eq!(groups.len(), 2);
    assert!(groups[0].body().payload().descriptor.admission_priority < groups[1].body().payload().descriptor.admission_priority);
    assert_ne!(groups[0].body().payload().input.entrypoint.hash(), groups[1].body().payload().input.entrypoint.hash());
    let fee_asset = native_economic_direct_fee_asset(&fixture.source);
    let carrier = empty_global_block_after(Some(&fixture.native.block)).header();
    let header = carrier.clone();
    for group in &groups {
        let quote = native_direct_fee_quote(&fixture, group, &carrier).unwrap();
        assert_eq!(quote.charges.len(), 1);
        assert_eq!(quote.charges[0].max_bound, Quantity::from(5u32));
        assert_eq!(quote.authority_balances.get(&fee_asset), Some(&Quantity::from(9u32)),
            "both exact signed inputs can pay before the earlier native input executes");
    }
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let _status = crate::sumeragi::status::nexus_fee_test_lock().lock().unwrap();
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let prepared = state.prepare_native_batch_on_carrier(header, &groups).unwrap();
    let batch = prepared.batch().clone();
    assert_eq!(batch.groups.len(), 2);
    assert!(prepared.executions()[0].result.is_ok(), "{:?}", prepared.executions()[0].result);
    assert!(prepared.executions()[1].result.is_err());
    assert!(format!("{:?}", prepared.executions()[1].result).contains("is insufficient"), "{:?}", prepared.executions()[1].result);
    assert_native_direct_fee_balance(&prepared.overlay().world, &fee_asset, 4);
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert!(prepared.executions()[1].fastpq_transcripts.is_empty());
    for (execution, group) in prepared.executions().iter().zip(&groups) {
        assert_eq!(execution.source, group.to_wire());
        assert!(execution.settlement_commitment.nexus_fee_receipts.is_empty());
        assert!(execution.settlement_commitment.native_amx_receipts.is_empty());
        assert_native_economic_terminal(prepared.overlay(), group, carrier.height().get());
    }
    assert_native_direct_fee_status(&fee_asset, 1);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert_native_direct_fee_balance(state.view().world(), &fee_asset, 9);
    let replay = state.replay_lane_decision_batch(&carrier, &batch, &groups).unwrap();
    assert_eq!(replay.batch(), &batch);
    assert_native_direct_fee_balance(&replay.overlay().world, &fee_asset, 4);
    for group in &groups { assert_native_economic_terminal(replay.overlay(), group, carrier.height().get()); }
    assert_native_direct_fee_status(&fee_asset, 2);
    drop(replay);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert_native_direct_fee_balance(state.view().world(), &fee_asset, 9);
}
