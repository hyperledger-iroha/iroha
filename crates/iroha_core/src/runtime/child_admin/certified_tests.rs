//! Signed child genesis, ordinary paid administration, validator support and exact native replay.
//!
//! The maintained chain fixture executes real node paths and exact-quorum certificates. It signs
//! votes with engineering keys in one process; this is not four-peer network qualification.

use super::*;
use crate::{
    state::{World, WorldReadOnly as _},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::Account,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetId},
    block::consensus::RootFeePolicy,
    domain::Domain,
    isi::{InstructionBox, Mint, Register},
    nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig},
    parameter::{Parameter, runtime_upgrade_policy::RuntimeUpgradePolicyV1},
    runtime::{
        child_admin::{
            ChildAdminSignatureV1, ChildRuntimeAdminPlanV1, ChildRuntimeAdminPolicyV1,
            foundation::ChildRuntimeFoundationV1,
        },
        lifecycle::{ChangeSetV1, RuntimeNoticeV1},
    },
};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_primitives::numeric::{Numeric, Quantity};

const HOME: DataSpaceId = DataSpaceId::new((1_u64 << 42) + 119);

fn admins() -> Vec<KeyPair> {
    let mut keys: Vec<_> = (0x71..=0x73)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

fn config() -> TestChainConfig {
    config_for_root(
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"child runtime certified parent",
        ))),
        HOME,
    )
}

/// Build the same signed, funded child foundation for an exact parent and data space.
pub(crate) fn config_for_root(parent: NetworkId, home: DataSpaceId) -> TestChainConfig {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.chain_id = "child-runtime-certified-journey".into();
    config.clock_bootstrap_in_genesis = true;
    config.root_scope = SumeragiRootScope::Dataspace {
        parent_network_id: parent,
        dataspace_id: home,
    };
    let fee_asset = iroha_config::parameters::defaults::nexus::fees::genesis_policy().fee_asset_id;
    let payer = AccountId::new(config.genesis_key.public_key().clone());
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        1_u32.try_into().unwrap(),
        vec![LaneConfig {
            dataspace_id: home,
            alias: "child-runtime-certified".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: home,
        alias: "child-runtime-certified".into(),
        description: None,
        fault_tolerance: 1,
    }])
    .unwrap();
    nexus.configured_dataspace_catalog = nexus.dataspace_catalog.clone();
    nexus.routing_policy.default_dataspace = home;
    nexus.fees.fee_asset_id = fee_asset.clone();
    nexus.fees.fee_sink_account_id = payer.clone();
    config.nexus = Some(nexus);
    config.genesis_parameters.push(Parameter::Custom(
        RootFeePolicy {
            fee_asset_id: fee_asset.clone(),
            fee_sink_account_id: payer.clone(),
            base_fee: 1_u32.into(),
            per_byte_fee: Quantity::zero(),
            per_instruction_fee: 1_u32.into(),
            per_gas_unit_fee: 1_u32.into(),
            ..iroha_config::parameters::defaults::nexus::fees::genesis_policy()
        }
        .into_custom_parameter()
        .unwrap(),
    ));
    config
        .genesis_parameters
        .extend(crate::pipeline_gas_policy::parameters_for_testing(
            iroha_config::parameters::actual::Gas {
                tech_account_id: payer.to_string(),
                accepted_assets: vec![fee_asset.canonical_address()],
                units_per_gas: vec![iroha_config::parameters::actual::GasRate {
                    asset: fee_asset.canonical_address(),
                    units_per_gas: 1,
                    twap_local_per_xor: Numeric::one(),
                    liquidity: iroha_config::parameters::actual::GasLiquidity::Tier2,
                    volatility: iroha_config::parameters::actual::GasVolatility::Stable,
                }],
            },
        ));
    let domain = DomainId::try_new("fees", "child-runtime-certified").unwrap();
    let relay = iroha_test_samples::ALICE_ID.clone();
    config.genesis_instructions.extend([
        Register::account(Account::new(relay.clone())).into(),
        Register::domain(Domain::new(domain.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            fee_asset.clone(),
            "Child runtime fee",
            AssetBalancePolicy::DataspaceRestricted,
            Some(domain),
        ))
        .into(),
    ]);
    let clock = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
    for account in [payer, relay, AccountId::new(clock.public_key().clone())] {
        config.genesis_instructions.push(
            Mint::asset_quantity(
                1_000_000_u32,
                AssetId::with_scope(
                    fee_asset.clone(),
                    account,
                    AssetBalanceScope::Dataspace(home),
                ),
            )
            .into(),
        );
    }
    let notice = RuntimeNoticeV1 {
        blocks: 2,
        ms: 0,
        approval_cushion_ms: 0,
    };
    config.genesis_parameters.push(Parameter::Custom(
        ChildRuntimeFoundationV1 {
            selection: crate::runtime::release_catalog()
                .unwrap()
                .child_foundation()
                .unwrap()
                .0
                .id()
                .unwrap(),
            policy: ChildRuntimeAdminPolicyV1 {
                revision: 1,
                admins: admins()
                    .iter()
                    .map(|key| key.public_key().clone())
                    .collect(),
                threshold: 2,
                notice: RuntimeUpgradePolicyV1 {
                    activation: notice,
                    disable: notice,
                },
                provenance: None,
            },
        }
        .to_custom_parameter()
        .unwrap(),
    ));
    config
}

/// Sign an action with the original administrators selected by this fixture's genesis.
pub(crate) fn action(
    chain: &CertifiedTestChain,
    action: ChildRuntimeAdminActionV1,
) -> InstructionBox {
    let view = chain.state().view();
    let authority = view.world().runtime_child_admin_state().as_ref().unwrap();
    let plan = ChildRuntimeAdminPlanV1 {
        root: authority.root,
        policy_id: authority.policy.id().unwrap(),
        policy_revision: authority.policy.revision,
        nonce: authority.next_nonce,
        valid_from: chain.height() + 1,
        valid_until: chain.height() + 20,
        action,
    };
    let signatures = admins()[..2]
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&plan, key).unwrap())
        .collect();
    AuthorizeChildRuntimeActionV1 {
        certificate: ChildRuntimeAdminCertificateV1 { plan, signatures },
    }
    .into()
}

fn submit(chain: &mut CertifiedTestChain, instruction: InstructionBox) -> bool {
    let time = 1_000 + (chain.height() + 1) * 100;
    let signed = chain.sign(&iroha_test_samples::ALICE_KEYPAIR, [instruction], time - 1);
    let results = chain.commit_at(time, vec![signed]);
    assert_eq!(results.len(), 1);
    results[0]
}

fn draft(chain: &CertifiedTestChain, height: u64, marker: u8) -> ChangeSetV1 {
    crate::runtime::draft_addition(
        chain.state().view().world(),
        chain.network_id(),
        crate::runtime::catalog::additions::ACCOUNT_SIGNING_KEYS.module,
        height,
        [marker; 32],
    )
    .unwrap()
}

#[test]
fn original_child_genesis_governs_paid_upgrade_and_replays_exact_certified_history() {
    // Match the maintained original-history fixtures' host stack; execution/codec pools and
    // protocol limits are the ordinary signed/configured ones throughout both chains.
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(child_upgrade_and_replay)
        .unwrap()
        .join()
        .unwrap();
}

fn child_upgrade_and_replay() {
    let mut chain = CertifiedTestChain::start(config()).expect("original child genesis applies");
    assert_eq!(chain.validators().len(), 4);
    let original_genesis = chain.genesis().encode_wire().unwrap();
    let foundation = chain.state().world_view().runtime_selection().id().unwrap();
    {
        let view = chain.state().view();
        let authority = view.world().runtime_child_admin_state().as_ref().unwrap();
        assert_eq!(authority.root.network, chain.network_id());
        assert_eq!(authority.root.instance, chain.instance().0);
        assert_eq!(authority.root.dataspace, HOME);
        assert_eq!(authority.next_nonce, 0);
    }
    let cancelled = draft(&chain, 8, 0x41);
    let cancelled_id = cancelled.id().unwrap();
    let schedule = action(
        &chain,
        ChildRuntimeAdminActionV1::Schedule {
            change_set: cancelled,
            provenance: Vec::new(),
        },
    );
    assert!(submit(&mut chain, schedule.clone()));
    let cancel = action(&chain, ChildRuntimeAdminActionV1::Cancel(cancelled_id));
    assert!(submit(&mut chain, cancel));
    assert!(
        !submit(&mut chain, schedule),
        "an old administrator nonce cannot be replayed"
    );
    let change = draft(&chain, 10, 0x42);
    let id = change.id().unwrap();
    let schedule = action(
        &chain,
        ChildRuntimeAdminActionV1::Schedule {
            change_set: change.clone(),
            provenance: Vec::new(),
        },
    );
    assert!(submit(&mut chain, schedule));
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let support: Vec<_> = fixture_keys()
        .iter()
        .map(|key| {
            crate::runtime::attestation::sign_support(
                chain.state().view().world(),
                &committee,
                chain.height(),
                id,
                key,
                [0x52; 32],
            )
            .unwrap()
        })
        .collect();
    let time = 1_000 + (chain.height() + 1) * 100;
    let transactions = support[..3]
        .iter()
        .cloned()
        .map(|attestation| {
            chain.sign(
                &iroha_test_samples::ALICE_KEYPAIR,
                [AttestChildRuntimeSupportV1 { attestation }.into()],
                time - 1,
            )
        })
        .collect();
    assert_eq!(chain.commit_at(time, transactions), [true, true, true]);
    assert_eq!(
        chain.state().world_view().runtime_selection().id().unwrap(),
        foundation
    );
    let mut rotated = chain
        .state()
        .world_view()
        .runtime_child_admin_state()
        .as_ref()
        .unwrap()
        .policy
        .clone();
    rotated.revision += 1;
    rotated.threshold = 3;
    let rotate = action(
        &chain,
        ChildRuntimeAdminActionV1::RotatePolicy(rotated.clone()),
    );
    assert!(submit(&mut chain, rotate));
    assert!(submit(
        &mut chain,
        AttestChildRuntimeSupportV1 {
            attestation: support[3].clone(),
        }
        .into()
    ));
    while chain.height() < change.effective_height {
        chain.commit_at(1_000 + (chain.height() + 1) * 100, Vec::new());
    }
    {
        let view = chain.state().view();
        assert_eq!(
            view.world()
                .runtime_change_sets()
                .get(&cancelled_id)
                .unwrap()
                .status,
            ChangeSetStatusV1::Cancelled(3)
        );
        assert_eq!(
            view.world().runtime_change_sets().get(&id).unwrap().status,
            ChangeSetStatusV1::Activated(10)
        );
        assert_eq!(
            view.world().runtime_selection().id().unwrap(),
            change.successor
        );
        let authority = view.world().runtime_child_admin_state().as_ref().unwrap();
        assert_eq!(authority.policy, rotated);
        assert_eq!(authority.next_nonce, 4);
    }
    let mut replay = CertifiedTestChain::start(config()).expect("same original child genesis");
    assert_eq!(replay.genesis().encode_wire().unwrap(), original_genesis);
    replay
        .replay_from(&chain)
        .expect("native replay preserves the original certified result");
    assert_eq!(
        replay.committed(replay.height()).result(),
        chain.committed(chain.height()).result()
    );
    assert_eq!(json_cell(&replay), json_cell(&chain));
}

fn json_cell(chain: &CertifiedTestChain) -> String {
    norito::json::to_json(&chain.state().world.runtime_child_admin_state).unwrap()
}
