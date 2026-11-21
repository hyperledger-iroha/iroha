//! Four real validators govern a signed child foundation and recover its authority on restart.
//!
//! This exercises one independently signed child root. The parent identity is an explicit
//! fixture pin; this scenario makes no claim about parent registration or anchoring that child.
//! Every live transition uses paid ordinary transactions and native consensus; no selection,
//! administrator state, certificate result or support record is injected after genesis.
//! Qualification requires `IROHA_TEST_REQUIRE_NETWORK=1`; an ordinary developer sandbox skip
//! follows the shared harness policy and supplies no network qualification evidence.

use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{blocking::Client, client::TransactionWaitOptions};
use iroha_config::base::toml::WriteExt as _;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    Level, NetworkId,
    account::{Account, AccountId},
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetId},
    block::consensus::{RootFeePolicy, SumeragiGenesisContextParameters, SumeragiRootScope},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    domain::Domain,
    isi::{
        Grant, InstructionBox, Log, Mint, Register,
        child_runtime::{AttestChildRuntimeSupportV1, AuthorizeChildRuntimeActionV1},
    },
    parameter::{
        CustomParameter, Parameter, runtime_upgrade_policy::RuntimeUpgradePolicyV1,
        system::ConsensusMode,
    },
    runtime::{
        child_admin::{
            ChildAdminSignatureV1, ChildRootIdentityV1, ChildRuntimeAdminActionV1,
            ChildRuntimeAdminCertificateV1, ChildRuntimeAdminPlanV1, ChildRuntimeAdminPolicyV1,
            foundation::ChildRuntimeFoundationV1,
        },
        definition::SelectionId,
        lifecycle::{ChangeSetId, ChangeSetStatusV1, ChangeSetV1, RootScopeV1, RuntimeNoticeV1},
    },
    sns::{DATASPACE_ALIAS_SUFFIX_ID, NameSelectorV1},
    sumeragi_finality::{FinalityValidator, SumeragiFinalityVerifier},
    transaction::FeePaymentIntent,
};
use iroha_genesis::GenesisBuilder;
use iroha_model_base::{chain::ChainId, domain::DomainId, topology::DataSpaceId};
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_test_network::{Network, NetworkBuilder};
use iroha_test_samples::ALICE_ID;
use norito::json;
use toml::{Table, Value};

use crate::governed_activation::harness;

const LABEL: &str = "child-runtime-admin";
const HEIGHT_TIMEOUT: Duration = Duration::from_secs(240);

fn admins() -> Vec<KeyPair> {
    let mut keys: Vec<_> = (0x71..=0x73)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

fn policy() -> ChildRuntimeAdminPolicyV1 {
    let notice = RuntimeNoticeV1 {
        blocks: 2,
        ms: 0,
        approval_cushion_ms: 0,
    };
    ChildRuntimeAdminPolicyV1 {
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
    }
}

fn parent() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"independent child-admin test parent pin; parent anchoring is a separate gate",
    )))
}

fn manifest() -> [u8; 32] {
    NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, LABEL)
        .unwrap()
        .name_hash()
}

fn home() -> DataSpaceId {
    DataSpaceId::from_hash(&manifest())
}

// Public catalog authoring reproduces the exact fixed genesis recipe. It grants no live
// activation authority: Core's original signed-genesis owner validates this selection itself.
fn foundation() -> Result<SelectionId> {
    let catalog = iroha_core::runtime::release_catalog()?;
    let mut selected = catalog.baseline.clone();
    let mut published: BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    for module in ["iroha.runtime.lifecycle.v1", "iroha.runtime.child_admin.v1"] {
        let change = catalog.activation(module, &selected, &published, parent(), 1, [0; 32])?;
        let applied = change.apply(&selected, &published)?;
        published.extend(applied.published);
        selected = applied.selection;
    }
    Ok(selected.id()?)
}

fn builder() -> Result<NetworkBuilder> {
    let key = KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519);
    let owner = AccountId::new(key.public_key().clone());
    let chain: ChainId = LABEL.into();
    let fee_asset = iroha_config::parameters::defaults::nexus::fees::genesis_policy().fee_asset_id;
    let domain = DomainId::try_new("fees", LABEL)?;
    let staking =
        iroha_genesis::default_staking_policy(fee_asset.clone(), owner.clone(), owner.clone());
    let mut context = SumeragiGenesisContextParameters::recommended();
    context.root_scope = SumeragiRootScope::Dataspace {
        parent_network_id: parent(),
        dataspace_id: home(),
    };
    let mut parameters = vec![
        Parameter::Custom(
            ChildRuntimeFoundationV1 {
                selection: foundation()?,
                policy: policy(),
            }
            .to_custom_parameter()?,
        ),
        Parameter::Custom(
            RootFeePolicy {
                fee_asset_id: fee_asset.clone(),
                fee_sink_account_id: owner.clone(),
                sponsor_vault_custody_account_id: owner.clone(),
                successful_claim_fee_exempt_authorities: Default::default(),
                base_fee: Quantity::one(),
                per_byte_fee: Quantity::zero(),
                per_instruction_fee: Quantity::one(),
                per_gas_unit_fee: Quantity::one(),
            }
            .into_custom_parameter()?,
        ),
    ];
    for (name, value) in [
        ("ivm_gas_tech_account_id", norito::json!(owner.to_string())),
        (
            "ivm_gas_accepted_assets",
            norito::json!([fee_asset.canonical_address()]),
        ),
        (
            "ivm_gas_units_per_gas",
            norito::json!([{
                "asset": (fee_asset.canonical_address()), "units_per_gas": 1,
                "twap_local_per_xor": "1", "liquidity_profile": "tier2", "volatility_class": "stable"
            }]),
        ),
    ] {
        parameters.push(Parameter::Custom(CustomParameter::new(
            name.parse()?,
            Json::new(value),
        )));
    }
    let mut instructions: Vec<InstructionBox> = vec![
        Register::account(Account::new(ALICE_ID.clone())).into(),
        Register::domain(Domain::new(domain.clone())).into(),
        Register::asset_definition(AssetDefinition::numeric(
            fee_asset.clone(),
            "Child administration fee",
            AssetBalancePolicy::DataspaceRestricted,
            Some(domain),
        ))
        .into(),
        Grant::account_permission(
            iroha_executor_data_model::permission::query::CanReadAllLedgerData,
            ALICE_ID.clone(),
        )
        .into(),
        Grant::account_permission(
            iroha_executor_data_model::permission::query::CanReadRestrictedDataspace {
                dataspace: home(),
            },
            ALICE_ID.clone(),
        )
        .into(),
    ];
    for account in [owner, ALICE_ID.clone()] {
        instructions.push(
            Mint::asset_quantity(
                1_000_000_u32,
                AssetId::with_scope(
                    fee_asset.clone(),
                    account,
                    AssetBalanceScope::Dataspace(home()),
                ),
            )
            .into(),
        );
    }
    let signed_chain = chain.clone();
    let signed_key = key.clone();
    Ok(NetworkBuilder::new()
        .with_base_seed(LABEL)
        .with_peers(4)
        .with_permissioned_consensus()
        .with_auto_populated_trusted_peers()
        .with_genesis_keypair(key)
        .with_block_cadence(Duration::from_secs(1))
        .with_genesis_block(move |_, entries| {
            let genesis = GenesisBuilder::new_without_executor(
                signed_chain.clone(),
                ".",
                CryptoAdmissionPolicyV1::new(vec![
                    Algorithm::Ed25519,
                    Algorithm::Secp256k1,
                    Algorithm::BlsNormal,
                ])
                .unwrap(),
            )
            .with_staking_policy(staking.clone())
            .unwrap()
            .with_block_cadence_ms(NonZeroU64::new(1_000).unwrap())
            .set_topology(entries)
            .with_sumeragi_context_parameters(context);
            let genesis = parameters
                .clone()
                .into_iter()
                .fold(genesis, GenesisBuilder::append_parameter)
                .next_transaction();
            instructions
                .clone()
                .into_iter()
                .fold(genesis, GenesisBuilder::append_instruction)
                .build_raw()
                .unwrap()
                .with_consensus_mode(ConsensusMode::Permissioned.into())
                .with_consensus_meta()
                .unwrap()
                .build_and_sign(&signed_key)
                .unwrap()
        })
        .with_config_layer(move |layer| {
            layer
                .write("chain", chain.to_string())
                .write(["concurrency", "rayon_global_threads"], 2_i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                )
                .write(
                    ["nexus", "lane_catalog"],
                    vec![Value::Table(
                        Table::new()
                            .write("index", 0_i64)
                            .write("alias", "private-root")
                            .write("dataspace", LABEL)
                            .write("visibility", "restricted")
                            .write("storage", "full_replica"),
                    )],
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    vec![Value::Table(
                        Table::new()
                            .write("alias", LABEL)
                            .write("manifest_hash", hex::encode(manifest()))
                            .write("fault_tolerance", 1_i64),
                    )],
                )
                .write(["nexus", "routing_policy", "default_lane"], 0_i64)
                .write(["nexus", "routing_policy", "default_dataspace"], LABEL);
        }))
}

fn fee() -> FeePaymentIntent {
    // The ordinary client obtains and signs the exact quote for each child instruction.
    FeePaymentIntent::authority(Vec::new(), None)
}

fn submit(client: &Client, instruction: impl Into<InstructionBox>) -> Result<u64> {
    let hash = client.submit(instruction.into(), fee())?;
    client
        .wait_for_transaction_applied(hash, TransactionWaitOptions::default())?
        .block_height
        .ok_or_else(|| eyre!("applied child transaction has no committed height"))
}

fn reject(client: &Client, instruction: impl Into<InstructionBox>, reason: &str) -> Result<()> {
    let error = submit(client, instruction).expect_err("invalid child action must not apply");
    ensure!(
        format!("{error:?}").contains(reason),
        "unexpected refusal: {error:?}"
    );
    Ok(())
}

fn identity(network: &Network) -> Result<ChildRootIdentityV1> {
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&network.genesis().0)?;
    let scope = metadata.sumeragi_context.root_scope;
    ensure!(
        scope
            == SumeragiRootScope::Dataspace {
                parent_network_id: parent(),
                dataspace_id: home()
            }
    );
    Ok(ChildRootIdentityV1 {
        network: network.network_id(),
        parent_network: parent(),
        dataspace: home(),
        instance: scope
            .instance_id(
                &iroha_core::sumeragi::crypto::BlsCrypto::new(),
                network.network_id(),
                network.chain_id().as_str(),
            )?
            .0,
    })
}

fn certificate(
    network: &Network,
    policy: &ChildRuntimeAdminPolicyV1,
    nonce: u64,
    action: ChildRuntimeAdminActionV1,
    signatures: usize,
) -> Result<AuthorizeChildRuntimeActionV1> {
    let height = harness::tip(&network.client())?;
    let plan = ChildRuntimeAdminPlanV1 {
        root: identity(network)?,
        policy_id: policy.id()?,
        policy_revision: policy.revision,
        nonce,
        valid_from: height + 1,
        valid_until: height + 64,
        action,
    };
    let signatures = admins()[..signatures]
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&plan, key))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AuthorizeChildRuntimeActionV1 {
        certificate: ChildRuntimeAdminCertificateV1 { plan, signatures },
    })
}

fn draft(network: &Network, height: u64, marker: u8) -> Result<ChangeSetV1> {
    let mut agreed = None;
    for peer in network.validators() {
        let response = peer.client().client().post_runtime_draft_upgrade_json(
            "iroha.dataspace.restricted_read_delegation.v1",
            height,
            &hex::encode([marker; 32]),
        )?;
        let change: ChangeSetV1 = json::from_value(response["change_set"].clone())?;
        ensure!(
            change.network == network.network_id()
                && change.root == RootScopeV1::DataSpace(home())
                && change.effective_height == height
                && change.qualification == [marker; 32]
        );
        ensure!(response["id_hex"].as_str() == Some(hex::encode(change.id()?.0).as_str()));
        ensure!(change.predecessor.to_string() == harness::runtime_view(peer)?.selection_id_hex);
        if let Some(expected) = &agreed {
            ensure!(expected == &change, "validator drafts differ");
        } else {
            agreed = Some(change);
        }
    }
    agreed.ok_or_else(|| eyre!("child has no validators"))
}

fn assert_record(
    network: &Network,
    id: ChangeSetId,
    status: ChangeSetStatusV1,
    selection: SelectionId,
    supports: usize,
) -> Result<()> {
    for peer in network.validators() {
        let view = harness::runtime_view(peer)?;
        let record = view
            .record(id)
            .ok_or_else(|| eyre!("child record missing"))?;
        ensure!(
            view.selection_id_hex == selection.to_string()
                && record.status == status
                && record.attestations.len() == supports,
            "child validators disagree on authority transition"
        );
        ensure!(record.change_set.root == RootScopeV1::DataSpace(home()));
    }
    Ok(())
}

fn carry(network: &Network, client: &Client, target: u64) -> Result<()> {
    let deadline = Instant::now() + HEIGHT_TIMEOUT;
    while harness::tip(client)? < target {
        ensure!(
            Instant::now() < deadline,
            "child did not reach activation height"
        );
        submit(
            client,
            Log::new(Level::INFO, format!("child admin carrier toward {target}")),
        )?;
    }
    harness::await_height_on_every_peer(network, target)
}

fn support(
    network: &Network,
    client: &Client,
    id: ChangeSetId,
    selection: SelectionId,
    count: usize,
) -> Result<()> {
    for (index, peer) in network.validators().iter().take(count).enumerate() {
        let drafted = peer
            .client()
            .client()
            .post_runtime_attest_upgrade_json(&hex::encode(id.0))?;
        let instruction = harness::drafted_instruction(&drafted)?;
        let support = instruction
            .as_any()
            .downcast_ref::<AttestChildRuntimeSupportV1>()
            .ok_or_else(|| eyre!("child node drafted a universal support instruction"))?;
        ensure!(support.attestation.attestation.root == RootScopeV1::DataSpace(home()));
        let height = submit(client, instruction)?;
        harness::await_height_on_every_peer(network, height)?;
        assert_record(
            network,
            id,
            ChangeSetStatusV1::Scheduled,
            selection,
            index + 1,
        )?;
    }
    Ok(())
}

fn verify_history(network: &Network, through: u64) -> Result<Hash> {
    let genesis = network.genesis().0;
    let validators = iroha_genesis::signed_genesis_validator_pops(&genesis)?
        .into_iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key,
            proof_of_possession,
        })
        .collect();
    let original =
        SumeragiFinalityVerifier::new(&genesis, network.chain_id().as_ref(), validators)?;
    let mut common = None;
    for peer in network.validators() {
        let mut verifier = original.clone();
        let client = peer.client();
        let mut terminal = None;
        for height in 1..=through {
            let proof = client.client().get_sumeragi_finality_proof(
                NonZeroU64::new(height).unwrap(),
                verifier.root_scope(),
            )?;
            terminal = Some(verifier.verify(&proof)?.context_id());
        }
        let terminal = terminal.ok_or_else(|| eyre!("missing positive child history"))?;
        if let Some(expected) = common {
            ensure!(terminal == expected, "certified child results differ");
        }
        common = Some(terminal);
    }
    common.ok_or_else(|| eyre!("no child validators"))
}

#[test]
fn four_validators_authorize_support_activate_and_restart_child_runtime() -> Result<()> {
    let Some((network, runtime)) = sandbox::start_network_blocking_or_skip(builder()?, LABEL)?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks(1))?;
    ensure!(network.validators().len() == 4);
    let client = network.client();
    let original = foundation()?;
    let initial = policy();
    let cancelled = draft(&network, harness::tip(&client)? + 14, 0x41)?;
    let cancelled_id = cancelled.id()?;
    let schedule = certificate(
        &network,
        &initial,
        0,
        ChildRuntimeAdminActionV1::Schedule {
            change_set: cancelled,
            provenance: Vec::new(),
        },
        2,
    )?;
    let mut foreign = schedule.clone();
    foreign.certificate.plan.root.parent_network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"another parent")));
    foreign.certificate.signatures = admins()[..2]
        .iter()
        .map(|key| ChildAdminSignatureV1::sign(&foreign.certificate.plan, key))
        .collect::<Result<_, _>>()?;
    reject(&client, foreign, "action names another root")?;
    let height = submit(&client, schedule.clone())?;
    harness::await_height_on_every_peer(&network, height)?;
    assert_record(
        &network,
        cancelled_id,
        ChangeSetStatusV1::Scheduled,
        original,
        0,
    )?;
    let cancel = certificate(
        &network,
        &initial,
        1,
        ChildRuntimeAdminActionV1::Cancel(cancelled_id),
        2,
    )?;
    let cancelled_at = submit(&client, cancel)?;
    harness::await_height_on_every_peer(&network, cancelled_at)?;
    assert_record(
        &network,
        cancelled_id,
        ChangeSetStatusV1::Cancelled(cancelled_at),
        original,
        0,
    )?;
    reject(&client, schedule, "nonce is not the next consumable nonce")?;

    // Three validator signatures are a consensus quorum but cannot authorize an upgrade.
    let lapsed = draft(&network, harness::tip(&client)? + 8, 0x43)?;
    let lapsed_id = lapsed.id()?;
    let height = submit(
        &client,
        certificate(
            &network,
            &initial,
            2,
            ChildRuntimeAdminActionV1::Schedule {
                change_set: lapsed.clone(),
                provenance: Vec::new(),
            },
            2,
        )?,
    )?;
    harness::await_height_on_every_peer(&network, height)?;
    support(&network, &client, lapsed_id, original, 3)?;
    carry(&network, &client, lapsed.effective_height)?;
    assert_record(
        &network,
        lapsed_id,
        ChangeSetStatusV1::Lapsed(lapsed.effective_height),
        original,
        3,
    )?;

    let change = draft(&network, harness::tip(&client)? + 12, 0x42)?;
    let id = change.id()?;
    let action = ChildRuntimeAdminActionV1::Schedule {
        change_set: change.clone(),
        provenance: Vec::new(),
    };
    reject(
        &client,
        certificate(&network, &initial, 3, action.clone(), 1)?,
        "falls short of the committed threshold",
    )?;
    let height = submit(&client, certificate(&network, &initial, 3, action, 2)?)?;
    harness::await_height_on_every_peer(&network, height)?;
    support(&network, &client, id, original, 4)?;
    // Support is every actual committee member, not merely consensus's three-vote quorum.
    for peer in network.validators() {
        let view = harness::runtime_view(peer)?;
        let mut signed: Vec<_> = view
            .record(id)
            .unwrap()
            .attestations
            .iter()
            .map(|signed| signed.attestation.validator.clone())
            .collect();
        let mut committee = view.committee;
        signed.sort();
        committee.sort();
        ensure!(signed == committee && signed.len() == 4);
    }
    carry(&network, &client, change.effective_height)?;
    assert_record(
        &network,
        id,
        ChangeSetStatusV1::Activated(change.effective_height),
        change.successor,
        4,
    )?;

    let mut rotated = initial.clone();
    rotated.revision = 2;
    rotated.threshold = 3;
    let height = submit(
        &client,
        certificate(
            &network,
            &initial,
            4,
            ChildRuntimeAdminActionV1::RotatePolicy(rotated.clone()),
            2,
        )?,
    )?;
    harness::await_height_on_every_peer(&network, height)?;
    let certified = verify_history(&network, height)?;
    let restarted = &network.validators()[3];
    runtime.block_on(restarted.shutdown());
    let layers: Vec<_> = network.config_layers_for_peer(restarted).collect();
    runtime
        .block_on(restarted.start_checked(layers.iter(), None))
        .wrap_err("restart child from its original signed genesis and Kura")?;
    harness::await_height_on_every_peer(&network, height)?;
    ensure!(verify_history(&network, height)? == certified);
    assert_record(
        &network,
        id,
        ChangeSetStatusV1::Activated(change.effective_height),
        change.successor,
        4,
    )?;

    // The restarted seat must retain the nonce and rotated threshold, and all four must agree
    // on an ordinary fresh action under it. No snapshot or query DTO supplies that authority.
    let mut next = rotated.clone();
    next.revision = 3;
    let action = ChildRuntimeAdminActionV1::RotatePolicy(next);
    reject(
        &restarted.client(),
        certificate(&network, &rotated, 5, action.clone(), 2)?,
        "falls short of the committed threshold",
    )?;
    let valid = certificate(&network, &rotated, 5, action, 3)?;
    let height = submit(&restarted.client(), valid.clone())?;
    harness::await_height_on_every_peer(&network, height)?;
    reject(&restarted.client(), valid, "action names another policy")?;
    verify_history(&network, height)?;
    Ok(())
}
