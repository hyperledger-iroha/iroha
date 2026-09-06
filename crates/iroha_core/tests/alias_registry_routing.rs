//! Signed admission, routing, and native execution regressions for additive SNS bootstrap.
use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingRule};
use iroha_core::{
    alias_setup::{alias_intent_owner, selector_for_resolved_alias_target},
    governance::manifest::LaneManifestRegistry,
    query::store::LiveQueryStore,
    queue::{
        ConfigLaneRouter, LaneRouter, RoutingDecision, RoutingPlan, RoutingResolveError,
        evaluate_policy_plan_with_nexus_and_world_at,
        evaluate_policy_plan_with_nexus_and_world_at_block_height,
    },
    smartcontracts::ivm::cache::IvmCache,
    sns,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    alias_setup::{
        AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1, AliasDataSpaceIntentV1,
        AliasDataspaceBootstrapGrantV1, AliasDomainIntentV1, AliasIntentV1,
        AliasLeaseAcquisitionV1, AliasQuoteGuardV1, AliasRegistryRoutingActivationV1,
        ResolvedAccountAliasV1, ResolvedDataSpaceV1, ResolvedDomainV1,
    },
    isi::alias_setup::{EnsureAlias, RenewAliasLease},
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
    prelude::*,
};
use iroha_executor_data_model::permission::parameter::CanSetParameters;
use iroha_primitives::time::TimeSource;
use mv::storage::StorageReadOnly;
use std::{collections::BTreeSet, num::NonZeroU32, num::NonZeroU64, sync::Arc, time::Duration};

const PRIVATE_DATASPACE: DataSpaceId = DataSpaceId::new(10);
const PRIVATE_LANE: LaneId = LaneId::new(1);

struct Fixture {
    state: State,
    signer: KeyPair,
    owner: AccountId,
    collector: AccountId,
    payment_asset: AssetDefinitionId,
}

fn fixture() -> Fixture {
    fixture_with_expanded_catalog(false)
}

fn fixture_with_expanded_catalog(include_bpng: bool) -> Fixture {
    let signer = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519).expect("signer");
    let owner = AccountId::new(signer.public_key().clone());
    let collector = AccountId::new(
        KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519)
            .expect("collector")
            .public_key()
            .clone(),
    );
    let payment_asset: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("current configured XOR");
    let domain = Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain"))
        .build(&collector);
    let definition = AssetDefinition::numeric(
        payment_asset.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&collector);
    let mut world = World::with_assets(
        [domain],
        [
            Account::new(owner.clone()).build(&collector),
            Account::new(collector.clone()).build(&collector),
        ],
        [definition],
        [Asset::new(
            AssetId::of(payment_asset.clone(), owner.clone()),
            Quantity::from(100_u64),
        )],
        [],
    );
    world.account_permissions_mut_for_testing().insert(
        owner.clone(),
        BTreeSet::from([Permission::from(CanSetParameters)]),
    );
    sns::seed_default_namespace_policies(&mut world);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.fee_asset_id = payment_asset.to_string();
    // Match the standard State test fixture's zero ordinary transaction fees. The independent
    // SNS quote/lease charge remains enabled and is asserted against actual global XOR balances.
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    let mut dataspaces = vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: PRIVATE_DATASPACE,
            alias: "paynet".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ];
    if include_bpng {
        dataspaces.push(DataSpaceMetadata {
            id: sns::dataspace_id_for_sns_alias("bpng").expect("BPNG ID"),
            alias: "bpng".to_owned(),
            description: None,
            fault_tolerance: 1,
        });
    }
    nexus.dataspace_catalog = DataSpaceCatalog::new(dataspaces).expect("existing dataspaces");
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("lane bound"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: PRIVATE_LANE,
                dataspace_id: PRIVATE_DATASPACE,
                alias: "paynet".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .expect("existing lanes");
    nexus.routing_policy.rules.push(LaneRoutingRule {
        lane: PRIVATE_LANE,
        dataspace: Some(PRIVATE_DATASPACE),
        matcher: LaneRoutingMatcher {
            account: Some(owner.to_string()),
            ..LaneRoutingMatcher::default()
        },
    });
    let state =
        State::new_with_pre_genesis_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    state
        .block(header(1))
        .commit_empty_block_for_testing()
        .expect("commit existing genesis fixture");
    Fixture {
        state,
        signer,
        owner,
        collector,
        payment_asset,
    }
}

fn header(height: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("height"),
        None,
        None,
        None,
        0,
        0,
    )
}

fn accepted(fixture: &Fixture, instructions: Vec<InstructionBox>) -> AcceptedTransaction<'static> {
    accepted_executable(fixture, Executable::Instructions(instructions.into()))
}

fn accepted_executable(fixture: &Fixture, executable: Executable) -> AcceptedTransaction<'static> {
    let gas_limit = matches!(&executable, Executable::IvmProved(_))
        .then(|| NonZeroU64::new(1_000_000).expect("bounded proved-VM gas"));
    let mut builder = TransactionBuilder::new(
        *fixture.state.network_id_ref(),
        fixture.owner.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), gas_limit),
    )
    .with_executable(executable);
    builder.set_creation_time(Duration::from_millis(1));
    let signed = builder.sign(fixture.signer.private_key());
    let view = fixture.state.view();
    let drift = view.world().parameters().sumeragi().max_clock_drift();
    let parameters = view.world().parameters().transaction();
    let crypto = fixture.state.crypto.read().clone();
    let (_, clock) = TimeSource::new_mock(Duration::from_millis(2));
    AcceptedTransaction::accept_with_time_source(
        signed,
        fixture.state.network_id_ref(),
        drift,
        parameters,
        crypto.as_ref(),
        &clock,
    )
    .expect("signed transaction passes stateless admission")
}

fn plans(
    fixture: &Fixture,
    transaction: &AcceptedTransaction<'_>,
    height: u64,
) -> Result<RoutingPlan, RoutingResolveError> {
    let view = fixture.state.view();
    evaluate_policy_plan_with_nexus_and_world_at_block_height(
        view.nexus(),
        transaction,
        view.world(),
        0,
        height,
    )
}

fn assert_universal_queue_and_block(fixture: &Fixture, transaction: &AcceptedTransaction<'_>) {
    let view = fixture.state.view();
    let nexus = view.nexus();
    let router = ConfigLaneRouter::new(
        nexus.routing_policy.clone(),
        nexus.dataspace_catalog.clone(),
        nexus.lane_catalog.clone(),
    );
    assert_eq!(
        router
            .try_route_plan_without_state(transaction)
            .expect("state-less deferral"),
        None
    );
    let queue = router
        .try_route_plan_with_view(transaction, &view)
        .expect("queue route");
    let block = evaluate_policy_plan_with_nexus_and_world_at_block_height(
        nexus,
        transaction,
        view.world(),
        0,
        u64::try_from(view.height()).expect("height") + 1,
    )
    .expect("block route");
    assert_eq!(
        queue, block,
        "queue and block must use the same activation boundary"
    );
    assert!(
        matches!(queue, RoutingPlan::Single(_)),
        "alias-only operations must not create a private participant: {queue:?}"
    );
    assert_eq!(
        queue.coordinator_route(),
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    );
}

fn apply(fixture: &Fixture, instructions: Vec<InstructionBox>) -> Result<(), String> {
    let transaction = accepted(fixture, instructions);
    let height = u64::try_from(fixture.state.view().height()).expect("height") + 1;
    let mut block = fixture.state.block(header(height));
    // This is the production stateful admission/execution path: it derives its own route,
    // enforces lane and executor permissions, and rolls back the whole transaction on failure.
    let result = block
        .validate_transaction(transaction, &mut IvmCache::new())
        .1;
    result.map_err(|error| format!("{error:?}"))?;
    // Persist the validated world overlay without manufacturing consensus/QC evidence in this
    // local routing/execution regression. Carrier-height fixtures are committed separately.
    block
        .commit_world_overlay_for_testing()
        .map_err(|error| format!("{error:?}"))
}

fn activate(fixture: &Fixture) {
    let parameter = AliasRegistryRoutingActivationV1::new(3).into_custom_parameter();
    apply(
        fixture,
        vec![SetParameter::new(Parameter::Custom(parameter)).into()],
    )
    .expect("governed future activation");
    fixture
        .state
        .block(header(2))
        .commit_empty_block_for_testing()
        .expect("commit activation carrier fixture");
}

fn bootstrap_grant(fixture: &Fixture) -> InstructionBox {
    SetParameter::new(Parameter::Custom(
        AliasDataspaceBootstrapGrantV1::try_new("bpng", fixture.owner.clone())
            .expect("canonical BPNG grant")
            .into_custom_parameter()
            .expect("canonical grant parameter"),
    ))
    .into()
}

fn ensure(fixture: &Fixture, intent: AliasIntentV1) -> EnsureAlias {
    let view = fixture.state.view();
    let selector = selector_for_resolved_alias_target(&intent.target()).expect("selector");
    let quote = sns::quote_resolved_name_registration(
        view.world(),
        selector.clone(),
        alias_intent_owner(&intent),
        1,
        None,
        0,
    )
    .expect("lease quote");
    let policy = sns::policy_by_id(view.world(), selector.suffix_id)
        .expect("policy storage")
        .expect("policy");
    EnsureAlias::new(
        intent,
        AliasLeaseAcquisitionV1::new(1, None),
        AliasQuoteGuardV1 {
            expected_policy_version: policy.policy_version,
            expected_payment_asset: quote.payment_asset_definition_id,
            max_amount: quote.charge_amount,
            valid_until_ms: u64::MAX,
        },
    )
}

fn bpng_intent(owner: &AccountId) -> AliasIntentV1 {
    let dataspace_id = sns::dataspace_id_for_sns_alias("bpng").expect("canonical hash-derived ID");
    assert_eq!(dataspace_id.as_u64(), 8_648_377_547_929_788_715);
    AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
        dataspace: ResolvedDataSpaceV1::new("bpng".parse().expect("name"), dataspace_id),
        owner: owner.clone(),
    })
}

fn balance(fixture: &Fixture, account: &AccountId) -> Quantity {
    fixture
        .state
        .view()
        .world()
        .asset(&AssetId::of(fixture.payment_asset.clone(), account.clone()))
        .map(|asset| asset.value().clone().into_inner())
        .unwrap_or_else(|_| Quantity::zero())
}

#[test]
fn alias_registry_routing_paid_post_genesis_dataspace_domain_and_renewal() {
    let mut fixture = fixture();
    let dataspace = ensure(&fixture, bpng_intent(&fixture.owner));
    let bpng = dataspace.intent.target().dataspace_id();
    assert!(
        plans(
            &fixture,
            &accepted(&fixture, vec![dataspace.clone().into()]),
            2
        )
        .is_err(),
        "legacy routing cannot acquire an uncatalogued dataspace"
    );
    activate(&fixture);
    apply(&fixture, vec![bootstrap_grant(&fixture)]).expect("grant precedes the first paid lease");
    assert_universal_queue_and_block(
        &fixture,
        &accepted(&fixture, vec![dataspace.clone().into()]),
    );
    let before = balance(&fixture, &fixture.owner);
    apply(&fixture, vec![dataspace.clone().into()]).expect("paid BPNG acquisition after genesis");
    let paid = balance(&fixture, &fixture.owner);
    assert!(paid < before, "acquisition must debit actual global XOR");
    assert!(balance(&fixture, &fixture.collector) > Quantity::zero());
    assert!(
        fixture
            .state
            .nexus_snapshot()
            .dataspace_catalog
            .by_id(bpng)
            .is_none(),
        "the name lease must not fabricate a catalog entry"
    );
    apply(&fixture, vec![dataspace.clone().into()]).expect("exact replay is idempotent");
    assert_eq!(
        balance(&fixture, &fixture.owner),
        paid,
        "no-op must not charge another lease"
    );

    let mut nexus = fixture.state.nexus_snapshot();
    let old_lanes = nexus.lane_catalog.clone();
    let mut entries = nexus.dataspace_catalog.entries().to_vec();
    entries.push(DataSpaceMetadata {
        id: bpng,
        alias: "bpng".to_owned(),
        description: None,
        fault_tolerance: 1,
    });
    nexus.dataspace_catalog = DataSpaceCatalog::new(entries).expect("additive canonical catalog");
    fixture
        .state
        .set_nexus_from_config(nexus)
        .expect("add matching static dataspace without resetting genesis or lanes");
    assert_eq!(fixture.state.nexus_snapshot().lane_catalog, old_lanes);
    assert_universal_queue_and_block(
        &fixture,
        &accepted(&fixture, vec![dataspace.clone().into()]),
    );

    let domain_id = DomainId::try_new("mibank", "bpng").expect("domain");
    let domain = ensure(
        &fixture,
        AliasIntentV1::Domain(AliasDomainIntentV1 {
            domain: ResolvedDomainV1::new(domain_id.clone(), bpng),
            owner: fixture.owner.clone(),
        }),
    );
    assert_universal_queue_and_block(&fixture, &accepted(&fixture, vec![domain.clone().into()]));
    apply(&fixture, vec![domain.clone().into()])
        .expect("paid domain creation uses universal registry");
    assert_eq!(
        fixture
            .state
            .view()
            .world()
            .domain(&domain_id)
            .expect("derived domain")
            .owned_by(),
        &fixture.owner
    );
    let domain_paid = balance(&fixture, &fixture.owner);
    apply(&fixture, vec![domain.clone().into()]).expect("idempotent domain repair");
    assert_eq!(balance(&fixture, &fixture.owner), domain_paid);

    let account_alias =
        ResolvedAccountAliasV1::new("treasury@mibank.bpng".parse().expect("account alias"), bpng);
    let account = ensure(
        &fixture,
        AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
            alias: account_alias.clone(),
            target_account: fixture.owner.clone(),
            provision: AccountProvisionV1::Existing,
            role: AccountAliasRoleV1::Additional,
        }),
    );
    apply(&fixture, vec![account.into()]).expect("paid account alias after domain");
    assert_eq!(
        fixture
            .state
            .view()
            .world()
            .account_aliases()
            .get(&account_alias.account_alias()),
        Some(&fixture.owner)
    );

    let view = fixture.state.view();
    let selector = selector_for_resolved_alias_target(&domain.intent.target()).expect("selector");
    let record =
        sns::get_name_record_by_selector(view.world(), &selector, 0).expect("domain lease");
    let quote = sns::quote_resolved_name_renewal(
        view.world(),
        selector.clone(),
        record.expires_at_ms,
        record.expires_at_ms + 31_536_000_000,
        0,
    )
    .expect("renewal quote");
    let policy = sns::policy_by_id(view.world(), selector.suffix_id)
        .expect("policy")
        .expect("namespace");
    let renew = RenewAliasLease::new(
        domain.intent.target(),
        record.expires_at_ms,
        quote.expires_at_ms,
        AliasQuoteGuardV1 {
            expected_policy_version: policy.policy_version,
            expected_payment_asset: quote.payment_asset_definition_id,
            max_amount: quote.charge_amount,
            valid_until_ms: u64::MAX,
        },
    );
    drop(view);
    assert_universal_queue_and_block(&fixture, &accepted(&fixture, vec![renew.clone().into()]));
    let before_renewal = balance(&fixture, &fixture.owner);
    apply(&fixture, vec![renew.clone().into()]).expect("paid renewal");
    assert!(balance(&fixture, &fixture.owner) < before_renewal);
    assert!(
        apply(&fixture, vec![renew.into()])
            .expect_err("stale expiry CAS must reject")
            .contains("alias.lease.expiry_conflict")
    );
    assert_eq!(
        fixture.state.view().height(),
        2,
        "the original carrier history remains intact"
    );
}

#[test]
fn alias_registry_routing_preserves_historical_route_and_fails_closed_without_height() {
    let fixture = fixture();
    let mut historical = ensure(&fixture, bpng_intent(&fixture.owner));
    if let AliasIntentV1::Dataspace(intent) = &mut historical.intent {
        intent.dataspace =
            ResolvedDataSpaceV1::new("paynet".parse().expect("name"), PRIVATE_DATASPACE);
    }
    let transaction = accepted(&fixture, vec![historical.into()]);
    let legacy = plans(&fixture, &transaction, 2).expect("historical route");
    assert_eq!(
        legacy.coordinator_route(),
        RoutingDecision::new(PRIVATE_LANE, PRIVATE_DATASPACE)
    );
    activate(&fixture);
    assert_eq!(
        plans(&fixture, &transaction, 2).expect("replay before activation"),
        legacy
    );
    assert_universal_queue_and_block(&fixture, &transaction);
    let view = fixture.state.view();
    assert_eq!(
        evaluate_policy_plan_with_nexus_and_world_at(view.nexus(), &transaction, view.world(), 0),
        Err(RoutingResolveError::AliasRegistryRoutingHeightUnavailable)
    );
}

#[test]
fn alias_registry_routing_nested_walkers_keep_the_explicit_activation_height() {
    use iroha_data_model::transaction::{ExecutableBatchItem, IvmBytecode, IvmProved};
    use iroha_executor_data_model::isi::multisig::{
        MultisigApprove, MultisigProposalState, MultisigPropose,
    };

    let fixture = fixture();
    let mut lease = ensure(&fixture, bpng_intent(&fixture.owner));
    if let AliasIntentV1::Dataspace(intent) = &mut lease.intent {
        intent.dataspace =
            ResolvedDataSpaceV1::new("paynet".parse().expect("name"), PRIVATE_DATASPACE);
    }
    let payload: Vec<InstructionBox> = vec![lease.into()];
    let instructions_hash = HashOf::new(&payload);
    let stored = MultisigProposalState {
        multisig_account_id: fixture.owner.clone(),
        instructions_hash,
        instructions: payload.clone(),
        proposed_at_ms: 0,
        expires_at_ms: 1_000,
        approvals: BTreeSet::new(),
        is_relayed: None,
    };
    {
        let mut block = fixture.state.block(header(2));
        let mut transaction = block.transaction();
        transaction
            .world
            .smart_contract_state_mut_for_testing()
            .insert(
                format!(
                    "multisig/proposal/{}/{}",
                    HashOf::new(&fixture.owner),
                    instructions_hash
                )
                .parse()
                .expect("proposal key"),
                norito::to_bytes(&stored).expect("native proposal encoding"),
            );
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("stored proposal fixture");
    }
    let trigger_id: TriggerId = "alias_registry_height".parse().expect("trigger id");
    let trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            payload.clone(),
            Repeats::Exactly(1),
            fixture.owner.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
        )
        .expect("trigger action"),
    ));
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version: 1,
    };
    let mut bytecode = meta.encode();
    bytecode.extend_from_slice(b"LTLB");
    for _ in 0..3 {
        bytecode.extend_from_slice(&0u32.to_le_bytes());
    }
    bytecode.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let executables = vec![
        Executable::Batch(
            payload
                .iter()
                .cloned()
                .map(ExecutableBatchItem::Instruction)
                .collect(),
        ),
        Executable::Instructions(
            vec![MultisigPropose::new(fixture.owner.clone(), payload.clone(), None).into()].into(),
        ),
        Executable::Instructions(
            vec![MultisigApprove::new(fixture.owner.clone(), instructions_hash).into()].into(),
        ),
        Executable::Instructions(vec![trigger.into()].into()),
        Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(bytecode),
            overlay: payload.into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas-policy"),
        }),
    ];
    let transactions: Vec<_> = executables
        .into_iter()
        .map(|executable| accepted_executable(&fixture, executable))
        .collect();
    let historical: Vec<_> = transactions
        .iter()
        .map(|transaction| plans(&fixture, transaction, 2).expect("legacy nested route"))
        .collect();
    activate(&fixture);
    for (transaction, historical) in transactions.iter().zip(historical) {
        assert_eq!(
            plans(&fixture, transaction, 2).expect("historical nested route"),
            historical
        );
        assert_universal_queue_and_block(&fixture, transaction);
    }
}

#[test]
fn alias_registry_routing_does_not_bypass_id_owner_quote_or_catalog_guards() {
    let fixture = fixture();
    let lease = ensure(&fixture, bpng_intent(&fixture.owner));
    activate(&fixture);
    let before = balance(&fixture, &fixture.owner);
    let mut invalid_id = lease.clone();
    if let AliasIntentV1::Dataspace(intent) = &mut invalid_id.intent {
        intent.dataspace.dataspace_id = DataSpaceId::new(10);
    }
    assert!(
        apply(&fixture, vec![invalid_id.into()])
            .expect_err("wrong hash-derived ID")
            .contains(sns::ALIAS_CATALOG_MAPPING_CONFLICT_CODE)
    );
    let mut wrong_owner = lease.clone();
    if let AliasIntentV1::Dataspace(intent) = &mut wrong_owner.intent {
        intent.owner = fixture.collector.clone();
    }
    assert!(
        apply(&fixture, vec![wrong_owner.into()])
            .expect_err("foreign owner")
            .contains("alias.setup.authority_forbidden")
    );
    let mut unpaid = lease.clone();
    unpaid.quote_guard.max_amount = Quantity::zero();
    assert!(
        apply(&fixture, vec![unpaid.into()])
            .expect_err("zero payment cap")
            .contains("alias.quote.cap_exceeded")
    );
    let mut catalog_claim = lease;
    if let AliasIntentV1::Dataspace(intent) = &mut catalog_claim.intent {
        intent.dataspace =
            ResolvedDataSpaceV1::new("paynet".parse().expect("name"), PRIVATE_DATASPACE);
    }
    assert!(
        apply(&fixture, vec![catalog_claim.into()])
            .expect_err("ungranted catalogued name")
            .contains("alias.catalog.bootstrap_required")
    );
    assert_eq!(
        balance(&fixture, &fixture.owner),
        before,
        "every rejected creation rolls back without a lease charge"
    );
}

#[test]
fn alias_registry_routing_keeps_real_private_participants_in_mixed_transactions() {
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias,
    };

    let fixture = fixture();
    let lease = ensure(&fixture, bpng_intent(&fixture.owner));
    activate(&fixture);
    let private_permission = Grant::account_permission(
        CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(PRIVATE_DATASPACE),
        },
        fixture.owner.clone(),
    );
    let transaction = accepted(&fixture, vec![lease.into(), private_permission.into()]);
    let plan = plans(&fixture, &transaction, 3).expect("mixed private/universal routing");
    let RoutingPlan::NativeAmx(native) = plan else {
        panic!("mixed registry/private write must retain AMX participants");
    };
    assert_eq!(
        native.coordinator.route,
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    );
    assert_eq!(native.participants.len(), 1);
    assert_eq!(
        native.participants[0].route,
        RoutingDecision::new(PRIVATE_LANE, PRIVATE_DATASPACE)
    );
}

#[test]
fn alias_registry_routing_cold_replay_with_expanded_catalog_preserves_paid_bootstrap() {
    // Reconstruct the original pre-lease world, not a snapshot already containing the SNS name.
    // The second independent instance has the future static catalog from startup. Both replay
    // exactly the same governed activation, bootstrap grant, and signed lease/domain sequence.
    let original = fixture_with_expanded_catalog(false);
    let replay = fixture_with_expanded_catalog(true);
    let dataspace = ensure(&original, bpng_intent(&original.owner));
    let target = dataspace.intent.target();
    let selector = selector_for_resolved_alias_target(&target).expect("BPNG selector");
    assert!(
        sns::record_by_selector(replay.state.view().world(), &selector)
            .expect("record storage")
            .is_none(),
        "cold replay must start without the newly acquired lease"
    );
    let grant = bootstrap_grant(&original);
    activate(&original);
    activate(&replay);
    for instruction in [grant.clone(), dataspace.clone().into()] {
        assert_eq!(
            accepted(&original, vec![instruction.clone()]).entrypoint(),
            accepted(&replay, vec![instruction]).entrypoint(),
            "cold execution replays the identical signed entrypoint"
        );
    }
    for fixture in [&original, &replay] {
        apply(fixture, vec![grant.clone()]).expect("replay the prior governed grant");
        assert_universal_queue_and_block(
            fixture,
            &accepted(fixture, vec![dataspace.clone().into()]),
        );
        apply(fixture, vec![dataspace.clone().into()])
            .expect("replay the first paid acquisition, not a no-op");
    }
    let domain_id = DomainId::try_new("mibank", "bpng").expect("domain");
    let domain = ensure(
        &original,
        AliasIntentV1::Domain(AliasDomainIntentV1 {
            domain: ResolvedDomainV1::new(domain_id.clone(), target.dataspace_id()),
            owner: original.owner.clone(),
        }),
    );
    assert_eq!(
        accepted(&original, vec![domain.clone().into()]).entrypoint(),
        accepted(&replay, vec![domain.clone().into()]).entrypoint()
    );
    for fixture in [&original, &replay] {
        apply(fixture, vec![domain.clone().into()]).expect("replay paid derived domain creation");
    }
    assert_eq!(
        sns::record_by_selector(original.state.view().world(), &selector).expect("original lease"),
        sns::record_by_selector(replay.state.view().world(), &selector).expect("replayed lease")
    );
    let domain_selector =
        selector_for_resolved_alias_target(&domain.intent.target()).expect("domain selector");
    assert_eq!(
        sns::record_by_selector(original.state.view().world(), &domain_selector)
            .expect("original domain lease"),
        sns::record_by_selector(replay.state.view().world(), &domain_selector)
            .expect("replayed domain lease")
    );
    assert_eq!(
        balance(&original, &original.owner),
        balance(&replay, &replay.owner)
    );
    assert_eq!(
        balance(&original, &original.collector),
        balance(&replay, &replay.collector)
    );
    assert_eq!(
        original
            .state
            .view()
            .world()
            .domain(&domain_id)
            .expect("original domain"),
        replay
            .state
            .view()
            .world()
            .domain(&domain_id)
            .expect("replayed domain")
    );
    assert_eq!(original.state.view().height(), replay.state.view().height());
}
