// Lane-reservation restart, reconciliation, and fee-capacity regression tests.
//
// Included by `queue::tests` so source-bound libtest names remain stable.
use crate::{
    kura::{
        AutonomousLifecycleCursorPhaseV1, AutonomousLifecycleCursorRead,
        AutonomousLifecycleProcessGenerationClaim,
    },
    sumeragi::{
        lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal,
        v2_lifecycle_recovery::sign_lifecycle_cursor,
    },
};

#[test]
fn fee_capacity_reservations_prevent_queue_oversubscription() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let second_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    assert_ne!(
        first_hash, second_hash,
        "fixtures must identify two transactions"
    );
    let (sponsor, _) = gen_account_in("sponsor");
    let (beneficiary, _) = gen_account_in("beneficiary");
    let program_id = FeeSponsorProgramId::new(
        sponsor,
        "queue_capacity".parse().expect("valid program name"),
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let amount = Quantity::from(6_u32);
    let remaining = Quantity::from(10_u32);
    let reservation = || {
        let block_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height: 7 }),
        };
        let program_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::ProgramEpoch(FeeSponsorProgramEpochBudgetWindow {
                epoch: 1,
            }),
        };
        let beneficiary_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::BeneficiaryEpoch(
                FeeSponsorBeneficiaryEpochBudgetWindow {
                    epoch: 1,
                    beneficiary: beneficiary.clone(),
                },
            ),
        };
        let source = FeeReservationAssetSource::SponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::from([(source.clone(), amount.clone())]),
            window_charges: BTreeMap::from([
                (block_key.clone(), amount.clone()),
                (program_epoch_key.clone(), amount.clone()),
                (beneficiary_epoch_key.clone(), amount.clone()),
            ]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::from([(source, Quantity::from(100_u32))]),
            window_remaining: BTreeMap::from([
                (block_key, remaining.clone()),
                (program_epoch_key, remaining.clone()),
                (beneficiary_epoch_key, remaining.clone()),
            ]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves the shared capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the same snapshot");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released capacity is immediately reusable");
}
#[test]
fn fee_reservation_refresh_moves_carried_transaction_to_current_block_window() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let second_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let (sponsor, _) = gen_account_in("refresh_fee_reservation");
    let (beneficiary, _) = gen_account_in("refresh_fee_beneficiary");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "rollover".parse().expect("valid program name"));
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let reservation_at = |height| {
        let key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height }),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::new(),
            window_charges: BTreeMap::from([(key.clone(), Quantity::from(6_u32))]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::new(),
            window_remaining: BTreeMap::from([(key, Quantity::from(10_u32))]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation_at(7))
        .expect("transaction reserves its enqueue-height window");
    store
        .refresh(first_hash, Some(reservation_at(8)))
        .expect("pop-time recheck moves the hold to the execution-height window");
    let err = store
        .reserve(second_hash, reservation_at(8))
        .expect_err("a competing transaction must see the refreshed current-height hold");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));
    store
        .refresh(first_hash, None)
        .expect("disabling fee charging releases the stale hold");
    store
        .reserve(second_hash, reservation_at(8))
        .expect("released current-height capacity is reusable");
}
#[test]
fn unsigned_payload_routing_matches_signed_queue_admission_routing() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let payload = tx
        .external()
        .expect("external transaction fixture")
        .payload()
        .clone();
    let state = State::new_for_testing(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let queue = Queue::test(config_factory(), &time_source);
    let signed = queue
        .route_plan_with_state(&tx, &state)
        .expect("signed route");
    let unsigned = queue
        .route_payload_plan_with_state(&payload, &state)
        .expect("unsigned route");
    assert_eq!(unsigned, signed);
}
#[test]
fn receipt_settled_queue_admission_rejects_authority_payer() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (authority, keypair) = gen_account_in("receipt_fee_admission");
    let domain_id =
        DomainId::try_new("receipt_fee_admission", "universal").expect("receipt fee domain");
    let fee_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("receipt fee asset name"),
    );
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::numeric(
            fee_asset.clone(),
            "receipt fee XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut nexus = state.nexus.write();
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
        nexus.fees.fee_asset_id = fee_asset.canonical_address();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    let queue = Queue::test(config_factory(), &time_source);
    let transaction = accepted_tx_by(authority, &keypair, &time_source);
    let error = queue
        .push(transaction, state.view())
        .expect_err("receipt-settled queue admission must require a sponsor");
    assert!(matches!(
        error.err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ref reason,
        } if reason.contains("active fee sponsor program")
            && reason.contains("exact active revision")
    ));
    assert_eq!(queue.active_len(), 0);
}
#[test]
fn authority_fee_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let second_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let (authority, _) = gen_account_in("authority_fee_reservation");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let asset_id = AssetId::new(asset_definition_id, authority.clone());
    let source = FeeReservationAssetSource::Authority(asset_id);
    let reservation = || FeeAdmissionReservation {
        program_revision: None,
        beneficiary: authority.clone(),
        asset_charges: BTreeMap::from([(source.clone(), Quantity::from(6_u32))]),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::new(),
        asset_remaining: BTreeMap::from([(source.clone(), Quantity::from(10_u32))]),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::new(),
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first authority transaction reserves its balance");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second authority transaction must not overbook the balance");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::AuthorityPayerInsufficient,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released authority capacity is immediately reusable");
}
#[test]
fn relay_spend_lease_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let second_hash = accepted_tx_by_someone(&time_source).hash_as_entrypoint();
    let (beneficiary, _) = gen_account_in("relay_lease_reservation");
    let lease_id = Hash::new(b"exact verified sponsor spend lease");
    let reservation = || FeeAdmissionReservation {
        program_revision: Some(3),
        beneficiary: beneficiary.clone(),
        asset_charges: BTreeMap::new(),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::from([(lease_id, Quantity::from(6_u32))]),
        asset_remaining: BTreeMap::new(),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::from([(lease_id, Quantity::from(10_u32))]),
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves exact lease capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the exact lease");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released lease capacity is immediately reusable");
}
#[test]
fn relay_spend_lease_reservation_maps_use_aggregate_per_asset_charges() {
    let (sponsor, _) = gen_account_in("relay_lease_map_sponsor");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "relay_maps".parse().expect("valid program name"));
    let domain_id = DomainId::try_new("relay_lease_maps", "universal").expect("valid fee domain");
    let shared_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "shared".parse().expect("valid shared asset name"),
    );
    let distinct_asset = AssetDefinitionId::derive_from_components(
        domain_id,
        "distinct".parse().expect("valid distinct asset name"),
    );
    let shared_lease = Hash::new(b"queue-shared-asset-spend-lease");
    let distinct_lease = Hash::new(b"queue-distinct-asset-spend-lease");
    let component_charges = [
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(4_u32),
        },
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::PipelineGas,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(6_u32),
        },
    ];
    let sponsor_charges = sponsored_charge_totals(&component_charges)
        .expect("Nexus and PipelineGas charges aggregate by fee asset");
    assert_eq!(sponsor_charges[&shared_asset], Quantity::from(10_u32));
    let sponsor_charges = BTreeMap::from([
        (shared_asset.clone(), sponsor_charges[&shared_asset].clone()),
        (distinct_asset.clone(), Quantity::from(7_u32)),
    ]);
    let selections = BTreeMap::from([
        (
            shared_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: shared_lease,
                remaining: Quantity::from(15_u32),
            },
        ),
        (
            distinct_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: distinct_lease,
                remaining: Quantity::from(8_u32),
            },
        ),
    ]);
    let (charges, remaining) =
        relay_lease_reservation_maps(&program_id, &sponsor_charges, selections)
            .expect("each charged asset maps to its exact selected lease");
    assert_eq!(charges[&shared_lease], Quantity::from(10_u32));
    assert_eq!(charges[&distinct_lease], Quantity::from(7_u32));
    assert_eq!(remaining[&shared_lease], Quantity::from(15_u32));
    assert_eq!(remaining[&distinct_lease], Quantity::from(8_u32));
}
