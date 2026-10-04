//! Source-authored genuine native private-root controls; execution is owned by Root.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    asset::{
        Asset, AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId, AssetId,
    },
    block::consensus::PrivateRootFeePolicy,
    domain::Domain,
    isi::InstructionBox,
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneVisibility},
    parameter::Parameter,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::json::Json;
use std::num::{NonZeroU32, NonZeroU64};

fn allocation_control_limits() -> CounterLimitsV1 {
    CounterLimitsV1 {
        max_entries: 103,
        max_groups: 256,
        max_carrier_work: 10_000,
        max_total_work: 10_000,
        max_source_bytes: 16 * 1024 * 1024,
        max_retained_bytes: 16 * 1024 * 1024,
        max_time_to_live_ms: 30_000,
        max_clock_skew_ms: 5_000,
        max_signature_age_ms: 30_000,
    }
}

#[test]
fn counter_graph_admission_uses_actual_remaining_capacity_and_one_original_pool() {
    let limits = allocation_control_limits();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let caller = budget.try_reserve_bytes(28 * 1024 * 1024 + 1).unwrap();
    assert!(matches!(
        CounterGraphAllocations::admit(&limits, &budget),
        Err(PrivateCountersErrorV1::Bounds)
    ));
    assert_eq!(
        budget.reserved_bytes(),
        caller.remaining_bytes(),
        "refused aggregate admission acquires no partial graph"
    );
    drop(caller);
    let owner = CounterGraphAllocations::admit(&limits, &budget).unwrap();
    assert!(
        owner.source.belongs_to(&budget)
            && owner.retained.belongs_to(&budget)
            && owner.frame.belongs_to(&budget)
    );
    assert_eq!(budget.reserved_bytes(), 36 * 1024 * 1024);
    let different_pool = AllocationBudget::new(budget.limit_bytes());
    assert!(
        !owner.retained.belongs_to(&different_pool),
        "equal ceilings never grant original pool authority"
    );
    assert_eq!(different_pool.reserved_bytes(), 0);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(budget.peak_reserved_bytes() <= budget.limit_bytes());
}

#[test]
fn wire_maximum_graph_caps_cannot_obtain_unfunded_service_capacity() {
    let mut limits = allocation_control_limits();
    limits.max_source_bytes = 256 * 1024 * 1024;
    limits.max_retained_bytes = 256 * 1024 * 1024;
    limits.validate().unwrap();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    assert!(matches!(
        CounterGraphAllocations::admit(&limits, &budget),
        Err(PrivateCountersErrorV1::Bounds)
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(budget.peak_reserved_bytes(), 0);
}

/// Shared genuine signed, funded private native chain, with exact installed genesis owner.
/// It changes no runtime configuration after original signed genesis executes.
pub(crate) struct PrivateCounterTestChain {
    pub(crate) chain: CertifiedTestChain,
    pub(crate) policy_key: KeyPair,
    pub(crate) readers: [KeyPair; 3],
    pub(crate) scope: SumeragiRootScope,
    pub(crate) fee_asset: AssetDefinitionId,
}

pub(crate) fn private_chain() -> PrivateCounterTestChain {
    private_chain_with_genesis(Vec::new())
}

fn private_chain_with_genesis(instructions: Vec<InstructionBox>) -> PrivateCounterTestChain {
    let ds = DataSpaceId::new(u64::MAX - 15);
    let policy_key = KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519);
    let readers = [
        policy_key.clone(),
        KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519),
        KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519),
    ];
    let clock = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
    let ids: Vec<_> = readers
        .iter()
        .chain([&clock])
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect();
    let owner = ids[0].clone();
    let domain =
        iroha_model_base::domain::DomainId::parse_fully_qualified("app.private-counter-test")
            .unwrap();
    let fee_asset =
        AssetDefinitionId::derive_from_components(domain.clone(), "gas".parse().unwrap());
    let mut definition = AssetDefinition::numeric(
        fee_asset.clone(),
        "Private counter gas",
        AssetBalancePolicy::DataspaceRestricted,
        Some(domain.clone()),
    )
    .build(&owner);
    definition.total_quantity = 10_000_000_u32.into();
    let world = World::with_assets(
        [Domain::new(domain).build(&owner)],
        ids.iter().map(|id| Account::new(id.clone()).build(&owner)),
        [definition],
        ids.iter().map(|id| {
            Asset::new(
                AssetId::with_scope(
                    fee_asset.clone(),
                    id.clone(),
                    AssetBalanceScope::Dataspace(ds),
                ),
                2_500_000_u32,
            )
        }),
        [],
    );
    let mut config = TestChainConfig::new(world, 1_000);
    config.genesis_instructions = instructions;
    assert_eq!(config.genesis_key.public_key(), policy_key.public_key());
    config.genesis_parameters.push(Parameter::Custom(
        PrivateRootFeePolicy {
            asset_definition_id: fee_asset.clone(),
            base_fee: 1_u32.into(),
            per_byte_fee: 0_u32.into(),
            per_instruction_fee: 1_u32.into(),
            per_gas_unit_fee: 1_u32.into(),
        }
        .into_custom_parameter()
        .unwrap(),
    ));
    let scope = SumeragiRootScope::Dataspace {
        parent_network_id: iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::from_untyped_unchecked(Hash::new(b"independent counter parent")),
        ),
        dataspace_id: ds,
    };
    config.root_scope = scope;
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.fee_asset_id = fee_asset.to_string();
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(1).unwrap(),
        vec![LaneConfig {
            dataspace_id: ds,
            visibility: LaneVisibility::Restricted,
            ..LaneConfig::default()
        }],
    )
    .unwrap();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: ds,
        alias: "private-counter-test".into(),
        description: None,
        fault_tolerance: 1,
    }])
    .unwrap();
    nexus.configured_dataspace_catalog = nexus.dataspace_catalog.clone();
    nexus.routing_policy.default_dataspace = ds;
    config.nexus = Some(nexus);
    let chain =
        CertifiedTestChain::start(config).expect("original signed private genesis executes");
    PrivateCounterTestChain {
        chain,
        policy_key,
        readers,
        scope,
        fee_asset,
    }
}

pub(crate) fn request_at_tip(
    fixture: &PrivateCounterTestChain,
    reader: usize,
    now_ms: u64,
) -> SignedPrivateCountersRequestV1 {
    let tip = fixture.chain.committed(fixture.chain.height());
    PrivateCountersRequestV1 {
        domain: PRIVATE_COUNTER_REQUEST_DOMAIN_V1,
        version: 1,
        network_id: fixture.chain.network_id(),
        scope: fixture.scope,
        authority: AccountId::new(fixture.readers[reader].public_key().clone()),
        purpose: CounterPurposeV1::ConnectedProvider,
        policy_hash: Hash::new(b"independently selected policy"),
        manifest_hash: Hash::new(b"independently selected manifest"),
        cut: CounterCutV1 {
            height: tip.height(),
            block_hash: tip.block_hash(),
            context_id: Hash::from_marked_bytes(*tip.id().0.as_ref()).expect("native context hash"),
            world_root: tip.commitment().execution.world_state_root,
            epoch_context_id: tip.commitment().schedule.current.context_id().unwrap(),
        },
        creation_time_ms: now_ms,
        time_to_live_ms: NonZeroU64::new(60_000).unwrap(),
        nonce: [42; 32],
    }
    .try_sign(&fixture.readers[reader])
    .unwrap()
}

#[test]
fn compiled_connected_plan_requires_both_exact_branch_selections_and_all103_actions() {
    for wholesale in [56, 57] {
        for seizure in [102, 103] {
            let ids: Vec<_> = (1..=105)
                .filter(|id| {
                    !([56, 57].contains(id) && *id != wholesale)
                        && !([102, 103].contains(id) && *id != seizure)
                })
                .collect();
            let plan = compiled_private_counter_plan_v1(CounterPurposeV1::ConnectedProvider, &ids)
                .unwrap();
            assert_eq!(plan.len(), 103);
            let bindings = source_authored_bindings(&plan);
            let commitment = counter_plan_commitment_v1(&plan, &bindings).unwrap();
            let mut substituted = plan.clone();
            substituted[0].category = CounterCategoryV1::Mint;
            assert_ne!(
                commitment,
                counter_plan_commitment_v1(&substituted, &bindings).unwrap()
            );
            let mut omitted = ids.clone();
            omitted.pop();
            assert!(
                compiled_private_counter_plan_v1(CounterPurposeV1::ConnectedProvider, &omitted)
                    .is_err()
            );
            let mut repeated = ids.clone();
            repeated[1] = repeated[0];
            assert!(
                compiled_private_counter_plan_v1(CounterPurposeV1::ConnectedProvider, &repeated)
                    .is_err()
            );
        }
    }
    assert!(
        compiled_private_counter_plan_v1(
            CounterPurposeV1::ConnectedProvider,
            &(1..=105).collect::<Vec<_>>()
        )
        .is_err()
    );
}

fn source_authored_bindings(plan: &[CounterSemanticV1]) -> Vec<CounterExecutableBindingV1> {
    let key = KeyPair::from_seed(vec![31; 32], Algorithm::Ed25519);
    let executable = Executable::Instructions(
        vec![InstructionBox::from(iroha_data_model::isi::Log::new(
            iroha_logger::Level::DEBUG,
            "plan commitment control".to_owned(),
        ))]
        .into(),
    );
    let executable_hash = HashOf::<Executable>::try_new(&executable).unwrap();
    plan.iter()
        .map(|semantic| CounterExecutableBindingV1 {
            action_id: semantic.action_id,
            authority: AccountId::new(key.public_key().clone()),
            executable_hash,
        })
        .collect()
}

#[path = "published_fixture.rs"]
mod published_fixture;
pub(crate) use published_fixture::{
    PublishedPrivateCounterFixture, published_interactions_fixture_v1,
};

#[test]
fn compiled_interaction_plan_requires_complete_current30_transactions() {
    for branch in [11, 12] {
        let ids: Vec<_> = (1..=31)
            .filter(|id| !([11, 12].contains(id) && *id != branch))
            .collect();
        let plan =
            compiled_private_counter_plan_v1(CounterPurposeV1::WalkthroughInteractions, &ids)
                .unwrap();
        assert_eq!(plan.len(), 30);
        assert_eq!(
            plan.iter()
                .filter(|entry| entry.category == CounterCategoryV1::BatchLeg)
                .count(),
            1
        );
        let mut reordered = ids.clone();
        reordered.reverse();
        assert!(
            compiled_private_counter_plan_v1(CounterPurposeV1::WalkthroughInteractions, &reordered)
                .is_err()
        );
    }
}

#[test]
fn policy_carrier_refuses_json_object_uppercase_escape_and_oversize_before_decode() {
    let owner = AccountId::new(
        KeyPair::from_seed(vec![31; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    for original in ["{}", "\"AA\"", "\"a\\u0061\"", "\"a\"", "\"aa\" trailing"] {
        let mut account = Account::new(owner.clone()).build(&owner).into_key_value().1;
        // Raw JSON control uses only valid payloads; trailing data is refused by Json itself.
        if let Ok(value) = Json::from_raw_json(original.to_owned()) {
            account.metadata.insert(
                PRIVATE_COUNTER_POLICY_METADATA_KEY_V1.parse().unwrap(),
                value,
            );
            assert!(read_policy(&account).is_err());
        }
    }
    let mut account = Account::new(owner.clone()).build(&owner).into_key_value().1;
    account.metadata.insert(
        PRIVATE_COUNTER_POLICY_METADATA_KEY_V1.parse().unwrap(),
        Json::new("aa".repeat(MAX_PRIVATE_COUNTER_POLICY_BYTES_V1 + 1)),
    );
    assert_eq!(read_policy(&account), Err(PrivateCountersErrorV1::Bounds));
}

#[test]
fn genuine_private_native_cut_authenticates_original_accounts_but_missing_policy_refuses() {
    let mut fixture = private_chain();
    fixture.chain.commit_at(2_000, vec![]);
    let request = request_at_tip(&fixture, 1, 2_000);
    let owner = AccountId::new(fixture.policy_key.public_key().clone());
    let tip = fixture.chain.committed(2);
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let mut consumed = false;
    fixture
        .chain
        .state()
        .with_native_private_counter_accounts_v1(
            &tip,
            &owner,
            &request.payload.authority,
            &budget,
            |snapshot, authority, reader| {
                assert_eq!(
                    snapshot.root().unwrap(),
                    tip.commitment().execution.world_state_root
                );
                assert!(authority.metadata.is_empty());
                assert!(reader.metadata.is_empty());
                consumed = true;
                Ok(())
            },
        )
        .expect("genuine immutable account originals");
    assert!(consumed);
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.chain.state(),
            &owner,
            &request,
            2_000,
            &budget
        ),
        Err(PrivateCountersErrorV1::Context)
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn signed_foreign_cut_and_invalid_signature_refuse_on_genuine_private_native_chain() {
    let mut fixture = private_chain();
    fixture.chain.commit_at(2_000, vec![]);
    let owner = AccountId::new(fixture.policy_key.public_key().clone());
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let mut request = request_at_tip(&fixture, 1, 2_000);
    request.payload.cut.world_root = Hash::new(b"different certified world");
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.chain.state(),
            &owner,
            &request,
            2_000,
            &budget
        ),
        Err(PrivateCountersErrorV1::Signature)
    ));
    let request = request
        .payload
        .clone()
        .try_sign(&fixture.readers[1])
        .unwrap();
    assert!(matches!(
        compute_private_transaction_counters_v1(
            fixture.chain.state(),
            &owner,
            &request,
            2_000,
            &budget
        ),
        Err(PrivateCountersErrorV1::Context)
    ));
}
