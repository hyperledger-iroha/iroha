// Actual signed Native inputs produce PipelineGas receipts and relay metadata.
// Direct Nexus fees remain burns; no receipt, policy root, or output is injected.

const NATIVE_RELAY_FUNDING: u32 = 1_000_000;

fn native_economic_relay_fixture(
    atomic_group: bool,
    with_manifest: bool,
) -> (Box<NativeEconomicFixture>, u64, Option<[u8; 32]>) {
    use iroha_config::parameters::actual::{GasLiquidity, GasRate, GasVolatility};
    use iroha_data_model::transaction::{FeeChargeKind, FeeChargeLimit, FeePaymentIntent};

    let policy = NativeEconomicDirectFee {
        funding: NATIVE_RELAY_FUNDING,
        signed_max: 5,
    };
    let mut manifest_root = None;
    // This constructor returns before the first nested StateBlock acquisition.
    // Keep the original boxed State and the existing default-stack fixture split.
    let mut setup = native_economic_state_setup(Some(policy), |world| {
        let issuer = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
        let account = AccountId::new(issuer.public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"native economic relay issuer"));
        let (id, value) = Account::new(account.clone())
            .with_uaid(Some(uaid))
            .build(&account)
            .into_key_value();
        world.accounts.insert(id, value);
        if with_manifest {
            let mut record = SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
                version: ManifestVersion::default(),
                uaid,
                dataspace: DataSpaceId::UNIVERSAL,
                issued_ms: 0,
                activation_epoch: 1,
                expiry_epoch: None,
                entries: Vec::new(),
            });
            record.lifecycle.mark_activated(1);
            let mut root = [0; Hash::LENGTH];
            root.copy_from_slice(record.manifest_hash.as_ref());
            manifest_root = Some(root);
            let mut manifests = SpaceDirectoryManifestSet::default();
            manifests.upsert(record);
            world
                .space_directory_manifests_mut_for_testing()
                .insert(uaid, manifests);
        }
    });
    let instructions: Vec<InstructionBox> = vec![
        Transfer::asset_quantity(
            setup.source.clone(),
            25u32,
            setup.destination_account.clone(),
        )
        .into(),
    ];
    let gas = crate::gas::meter_instructions(&instructions);
    assert!(gas > 0 && gas + u64::from(policy.signed_max) < u64::from(policy.funding));
    let asset = setup.fee_asset.definition().canonical_address();
    setup.state.pipeline.gas.tech_account_id = setup.destination_account.to_string();
    setup.state.pipeline.gas.accepted_assets = vec![asset.clone()];
    setup.state.pipeline.gas.units_per_gas = vec![GasRate {
        asset,
        units_per_gas: 1,
        twap_local_per_xor: Numeric::one(),
        liquidity: GasLiquidity::Tier1,
        volatility: GasVolatility::Stable,
    }];
    setup.fee_intent = Some(FeePaymentIntent::authority(
        vec![
            FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                setup.fee_asset.definition().clone(),
                Quantity::from(policy.signed_max),
            ),
            FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                setup.fee_asset.definition().clone(),
                Quantity::from(gas),
            ),
        ],
        None,
    ));
    let fixture = native_economic_fixture_from_state(
        &[NativeEconomicCase::Transfer(25)],
        atomic_group,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
        Some(policy),
        setup,
    );
    // Observe the actual directory projection before acquiring execution writers.
    let snapshot = fixture.native.state.axt_policy_snapshot();
    let projected_root = snapshot
        .entries
        .iter()
        .find(|entry| entry.dsid == DataSpaceId::UNIVERSAL)
        .map(|entry| entry.policy.manifest_root);
    assert_eq!(projected_root, manifest_root);
    (fixture, gas, manifest_root)
}

fn assert_native_economic_relay_input(
    group: &super::VerifiedLaneDecisionGroupV1,
    atomic_group: bool,
    gas: u64,
) {
    use iroha_data_model::transaction::FeeChargeKind;
    assert_eq!(
        group.body().payload().descriptor.slots.len(),
        if atomic_group { 2 } else { 1 }
    );
    assert_eq!(group.decisions().len(), group.contexts().len());
    for (decision, context) in group.decisions().iter().zip(group.contexts()) {
        assert_eq!(context.frozen().committee.len(), 4);
        assert_eq!(context.frozen().validator_set_pops.len(), 4);
        assert_eq!(
            context.frozen().da_layout.encoding,
            PayloadEncoding::ReedSolomon16
        );
        assert_eq!(decision.commit_qc.shares.len(), 3);
        assert_eq!(
            decision.manifest.byte_len,
            u64::try_from(group.body().canonical_bytes().len()).unwrap()
        );
    }
    let TransactionEntrypoint::External(transaction) = &group.body().payload().input.entrypoint
    else {
        panic!("the fee receipt belongs to the original signed external input");
    };
    transaction.verify_signature().unwrap();
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        transaction.instructions()
    else {
        panic!("the signed input carries the exact metered transfer");
    };
    assert_eq!(instructions.len(), 1);
    assert_eq!(crate::gas::meter_instructions(instructions.as_ref()), gas);
    let intent = transaction.fee_payment_intent();
    assert!(intent.sponsor_program().is_none());
    assert_eq!(intent.charge_limits().len(), 2);
    assert_eq!(
        intent
            .charge_limits()
            .iter()
            .find(|limit| limit.kind() == FeeChargeKind::Nexus)
            .unwrap()
            .max_amount(),
        &Quantity::from(5u32)
    );
    assert_eq!(
        intent
            .charge_limits()
            .iter()
            .find(|limit| limit.kind() == FeeChargeKind::PipelineGas)
            .unwrap()
            .max_amount(),
        &Quantity::from(gas)
    );
}

fn assert_native_economic_relay_effects(
    fixture: &NativeEconomicFixture,
    overlay: &StateBlock<'_>,
    execution: &super::PreexecutedLaneDecisionGroupV1,
    gas: u64,
) {
    assert!(execution.result.is_ok(), "{:?}", execution.result);
    let commitment = &execution.settlement_commitment;
    assert_eq!(
        commitment.tx_count, 1,
        "one input is charged once across all participant slots"
    );
    assert_eq!(commitment.receipts.len(), 1);
    assert!(
        commitment.nexus_fee_receipts.is_empty(),
        "Direct Nexus settlement is a real burn"
    );
    assert!(
        commitment.native_amx_receipts.is_empty(),
        "Native Decisions do not synthesize old AMX receipts"
    );
    let receipt = &commitment.receipts[0];
    let TransactionEntrypoint::External(transaction) = &execution.source.payload.input.entrypoint
    else {
        unreachable!()
    };
    assert_eq!(receipt.source_id.as_slice(), transaction.hash().as_ref());
    assert_eq!(receipt.local_amount, Quantity::from(gas));
    assert!(!receipt.xor_due.is_zero());
    assert_eq!(commitment.total_local_amount, receipt.local_amount);
    assert_eq!(commitment.total_xor_due, receipt.xor_due);
    assert_eq!(
        commitment.total_xor_after_haircut,
        receipt.xor_after_haircut
    );
    assert_eq!(commitment.total_xor_variance, receipt.xor_variance);
    assert!(commitment.swap_metadata.is_some());
    assert_eq!(
        iroha_data_model::nexus::compute_settlement_hash(commitment).unwrap(),
        execution.settlement_hash
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().0,
        Quantity::from(75u32)
    );
    assert_eq!(
        overlay.world.assets.get(&fixture.destination).unwrap().0,
        Quantity::from(25u32)
    );
    let payer = native_economic_direct_fee_asset(&fixture.source);
    let tech = AssetId::new(
        payer.definition().clone(),
        fixture.destination.account().clone(),
    );
    assert_eq!(
        overlay.world.assets.get(&payer).unwrap().0,
        Quantity::from(u64::from(NATIVE_RELAY_FUNDING) - gas - 5)
    );
    assert_eq!(
        overlay.world.assets.get(&tech).unwrap().0,
        Quantity::from(gas)
    );
    assert_eq!(
        overlay
            .world
            .asset_definition(payer.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(NATIVE_RELAY_FUNDING - 5),
        "gas transfers to the actual technical account; only Nexus burns supply"
    );
}

fn assert_native_economic_relay_recorder_released() {
    let guard = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    let empty = crate::sumeragi::witness::drain_exec_witness_checked(|_| Ok(())).unwrap();
    assert!(
        empty.reads.is_empty() && empty.writes.is_empty() && empty.fastpq_transcripts.is_empty()
    );
    drop(guard);
}

fn native_recorded_economic_relay_success(atomic_group: bool) {
    use super::NativeLaneBatchSourcePreparationV1;
    let (fixture, gas, manifest_root) = native_economic_relay_fixture(atomic_group, true);
    let state = &fixture.native.state;
    let applying =
        native_control_verified_context(state, fixture.native.block.header().height().get());
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("actual first-carrier admission and signed Decisions");
    };
    assert_eq!(source.groups_for_test().len(), 1);
    let group = &source.groups_for_test()[0];
    assert_native_economic_relay_input(group, atomic_group, gas);
    let pointers = (
        group.body().canonical_bytes().as_ptr(),
        group.decisions().as_ptr(),
        group.contexts().as_ptr(),
    );
    let recorded = source
        .record_execution(carrier, applying)
        .unwrap()
        .expect("the exact captured source State remains current");
    let prepared = recorded.prepared_for_test();
    let group = &prepared.sources_for_test()[0];
    assert_eq!(
        (
            group.body().canonical_bytes().as_ptr(),
            group.decisions().as_ptr(),
            group.contexts().as_ptr()
        ),
        pointers
    );
    assert_eq!(prepared.executions().len(), 1);
    let overlay = prepared.overlay();
    let execution = &prepared.executions()[0];
    assert_native_economic_relay_effects(&fixture, overlay, execution, gas);
    assert_native_economic_terminal(overlay, group, recorded.carrier().header().height().get());
    let statements = recorded.carrier().lane_finality_statements();
    assert_eq!(
        statements.len(),
        1,
        "one coordinator economic effect, not one duplicate per atomic slot"
    );
    let statement = &statements[0];
    let route = group
        .body()
        .payload()
        .input
        .routing_plan()
        .unwrap()
        .coordinator_route();
    let (index, slot) = group
        .body()
        .payload()
        .descriptor
        .slots
        .iter()
        .enumerate()
        .find(|(_, slot)| slot.route == route)
        .unwrap();
    assert_eq!(
        (
            statement.lane_id,
            statement.dataspace_id,
            statement.lane_incarnation,
            statement.block_height
        ),
        (
            route.lane_id,
            route.dataspace_id,
            slot.lane_incarnation,
            slot.lane_height
        )
    );
    assert_eq!(statement.block_header_hash, recorded.carrier().hash());
    assert_eq!(
        statement.da_commitment_hash,
        recorded.carrier().header().da_commitments_hash()
    );
    assert_eq!(
        statement.lane_block_descriptor_hash,
        group.body().payload().descriptor.canonical_hash().unwrap()
    );
    assert_eq!(statement.manifest_root, manifest_root.unwrap());
    assert_eq!(
        statement.settlement_commitment,
        execution.settlement_commitment
    );
    assert_eq!(statement.settlement_hash, execution.settlement_hash);
    assert_eq!(
        statement.rbc_bytes_total,
        group.decisions()[index].manifest.byte_len
    );
    let rows = recorded.carrier().execution_outputs();
    assert_eq!(rows.len(), 1);
    assert!(matches!(
        &rows[0],
        iroha_data_model::block::execution_output::ExecutionOutputV1::Network(_)
    ));
    assert!(rows[0].result().is_ok());
    overlay
        .verify_execution_output_seal(recorded.carrier())
        .unwrap();
    overlay
        .verified_fastpq_source_inventory_for_capture()
        .unwrap()
        .verify_ordinary_witness_bundles(&overlay.exec_witness.as_ref().unwrap().fastpq_transcripts)
        .unwrap();
    assert!(
        crate::block::ValidBlock::validate_inactive_native_carrier_for_test(recorded.carrier())
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    drop(recorded);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

fn native_recorded_economic_relay_missing_manifest(atomic_group: bool) {
    use super::NativeLaneBatchSourcePreparationV1;
    let (fixture, gas, root) = native_economic_relay_fixture(atomic_group, false);
    assert!(root.is_none());
    let state = &fixture.native.state;
    let applying =
        native_control_verified_context(state, fixture.native.block.header().height().get());
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    // The real fee execution succeeds and creates its receipt before metadata
    // encounters the intentionally absent manifest. This scratch is discarded;
    // it is never used to inject an output into the subsequent recorded owner.
    let groups = native_economic_groups(&fixture);
    assert_native_economic_relay_input(&groups[0], atomic_group, gas);
    let scratch = state
        .prepare_native_batch_on_carrier(carrier.header(), groups)
        .unwrap();
    assert_native_economic_relay_effects(
        &fixture,
        scratch.overlay(),
        &scratch.executions()[0],
        gas,
    );
    drop(scratch);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("the complete source remains authentic without a relay policy root");
    };
    let error = source
        .record_execution(carrier, applying)
        .err()
        .expect("a real receipt cannot be sealed without a manifest root");
    let reason = error.to_string();
    assert!(
        reason.contains("Native economic relay effect is incomplete"),
        "{reason}"
    );
    assert!(
        reason.contains(&iroha_data_model::nexus::LaneRelayError::MissingManifestRoot.to_string()),
        "{reason}"
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_recorded_economic_relay_single_uses_actual_pipeline_gas_settlement
    native_recorded_economic_relay_success(false);
}
state_test! { sync native_recorded_economic_relay_atomic_uses_actual_pipeline_gas_settlement
    native_recorded_economic_relay_success(true);
}
state_test! { sync native_recorded_economic_relay_single_missing_manifest_root_rolls_back
    native_recorded_economic_relay_missing_manifest(false);
}
state_test! { sync native_recorded_economic_relay_atomic_missing_manifest_root_rolls_back
    native_recorded_economic_relay_missing_manifest(true);
}
