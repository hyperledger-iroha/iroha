//! Taira-shaped local-node lifecycle coverage for the canonical typed ZK-ACE
//! privacy transfer.
#![cfg(feature = "zk-stark")]

use std::{
    num::NonZeroU32,
    thread::sleep,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        asset::AssetDefinition,
        isi::{
            Grant, InstructionBox, Log, Mint, Register,
            privacy::{
                RegisterPrivacyProtocolActivationV1, RegisterPrivacyZkAcePolicyV1,
                RevokePrivacyZkAcePolicyV1, RotatePrivacyZkAcePolicyV1,
            },
        },
        metadata::Metadata,
        permission::Permission,
        prelude::{
            AssetDefinitionId, AssetId, DomainId, FindAssets, Identifiable, Quantity,
            QueryBuilderExt,
        },
        privacy::{
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1, PrivacyActiveLifecycleV1,
            PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyProposedLifecycleV1,
            PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyZkAcePolicyLifecycleV1,
            PrivacyZkAcePolicyRecordV1,
        },
        query::block::prelude::FindBlocks,
        transaction::{FeePaymentIntent, SignedTransaction},
    },
};
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1, privacy_profiles::compiled_privacy_profile_v1,
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use zk_ace_prover::{
    SignedZkAcePrivacyTransferV1, ZkAcePrivacyActionTransactionContextV1, ZkAcePrivacyTransferV1,
    ZkAcePrivacyWitnessV1, build_signed_zk_ace_privacy_transfer_v1,
};

const TEST_NAME: &str = "canonical_zk_ace_privacy_transfer_taira_localnet";
const PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const BALANCE_TIMEOUT: Duration = Duration::from_secs(45);

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

fn require_test_network_feature(feature: &str) -> Result<()> {
    let enabled = std::env::var("TEST_NETWORK_IROHAD_FEATURES")
        .ok()
        .is_some_and(|value| {
            value
                .split([',', ' ', '\t', '\n'])
                .any(|item| item.trim() == feature)
        });
    ensure!(
        enabled,
        "{TEST_NAME}: TEST_NETWORK_IROHAD_FEATURES must include `{feature}`"
    );
    Ok(())
}

fn asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("default domain"),
        "zkace_typed".parse().expect("asset name"),
    )
}

fn submit_instruction(
    client: &Client,
    instruction: impl Into<InstructionBox>,
    context: &str,
) -> Result<()> {
    client
        .submit_blocking(instruction, no_fee())
        .wrap_err_with(|| context.to_owned())?;
    Ok(())
}

fn submit_signed(client: &Client, transaction: &SignedTransaction, context: &str) -> Result<()> {
    client
        .submit_transaction_blocking(transaction)
        .wrap_err_with(|| context.to_owned())?;
    Ok(())
}

fn error_chain_contains(error: &eyre::Report, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    error
        .chain()
        .any(|cause| cause.to_string().to_ascii_lowercase().contains(&needle))
}

fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    let blocks = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err("query canonical genesis block")?;
    let genesis = blocks
        .iter()
        .filter(|block| block.header().prev_block_hash().is_none())
        .collect::<Vec<_>>();
    ensure!(
        genesis.len() == 1,
        "expected exactly one genesis block, got {}",
        genesis.len()
    );
    let hash = *genesis[0].header().hash().as_ref();
    ensure!(hash != [0; 32], "canonical genesis hash is zero");
    Ok(hash)
}

fn next_incoming_height(client: &Client) -> Result<u64> {
    client
        .get_privacy_capabilities()
        .wrap_err("query privacy height")?
        .committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("privacy height overflow"))
}

fn advance_to_height(client: &Client, target: u64) -> Result<()> {
    loop {
        let current = client
            .get_privacy_capabilities()
            .wrap_err("query activation advance height")?
            .committed_height;
        if current >= target {
            ensure!(
                current == target,
                "activation advance overshot {target} at {current}"
            );
            return Ok(());
        }
        submit_instruction(
            client,
            Log::new(
                Level::INFO,
                format!("ZK-ACE activation block {}", current + 1),
            ),
            "advance ZK-ACE activation height",
        )?;
    }
}

fn wait_for_balance(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    account_id: &iroha::data_model::account::AccountId,
    expected: u64,
) -> Result<()> {
    let deadline = Instant::now() + BALANCE_TIMEOUT;
    let expected = Quantity::from(expected);
    let mut last = None;
    while Instant::now() < deadline {
        if let Ok(assets) = client.query(FindAssets::new()).execute_all() {
            last = assets
                .iter()
                .find(|asset| {
                    asset.id().definition() == asset_definition_id
                        && asset.id().account() == account_id
                })
                .map(|asset| asset.value().clone());
            if last.as_ref() == Some(&expected) {
                return Ok(());
            }
        }
        sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "balance did not converge for {account_id}; expected {expected:?}, observed {last:?}"
    ))
}

fn witness(seed: u8) -> ZkAcePrivacyWitnessV1 {
    ZkAcePrivacyWitnessV1::try_new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
    )
    .expect("valid localnet witness")
}

fn policy(
    witness: &ZkAcePrivacyWitnessV1,
    epoch: u64,
    lifecycle: PrivacyZkAcePolicyLifecycleV1,
) -> PrivacyZkAcePolicyRecordV1 {
    PrivacyZkAcePolicyRecordV1::new(
        PrivacyPolicyIdV1::new([0x41; 32]),
        witness.identity_commitment_v1(),
        PrivacyPolicyDigestV1::new([0x42; 32]),
        epoch,
        asset_definition_id(),
        vec![ALICE_ID.clone()],
        lifecycle,
    )
    .expect("valid governed policy")
}

fn build_transfer(
    client: &Client,
    policy: PrivacyZkAcePolicyRecordV1,
    witness: ZkAcePrivacyWitnessV1,
    genesis_hash: [u8; 32],
    nonce: u32,
    amount: u128,
) -> Result<SignedZkAcePrivacyTransferV1> {
    let transfer =
        ZkAcePrivacyTransferV1::try_new(policy, ALICE_ID.clone(), BOB_ID.clone(), amount)
            .wrap_err("construct governed ZK-ACE transfer")?;
    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before Unix epoch")?;
    build_signed_zk_ace_privacy_transfer_v1(
        ZkAcePrivacyActionTransactionContextV1 {
            chain_id: client.chain.clone(),
            authority: ALICE_ID.clone(),
            creation_time,
            time_to_live: Some(Duration::from_secs(3_600)),
            nonce: NonZeroU32::new(nonce),
            fee_payment: no_fee(),
            metadata: Metadata::default(),
        },
        transfer,
        witness,
        genesis_hash,
        ALICE_KEYPAIR.private_key(),
    )
    .wrap_err("build canonical signed ZK-ACE privacy transfer")
}

#[test]
fn canonical_zk_ace_privacy_transfer_taira_localnet() -> Result<()> {
    require_test_network_feature("zk-stark")?;
    init_instruction_registry();

    let asset_definition_id = asset_definition_id();
    let alice_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        ))
        .with_genesis_instruction(Mint::asset_quantity(100_u64, alice_asset))
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });
    let Some((network, _runtime)) = sandbox::start_network_blocking_or_skip(builder, TEST_NAME)?
    else {
        return Ok(());
    };
    let mut client = network.client();
    client.add_transaction_nonce = true;
    let genesis_hash = canonical_genesis_hash(&client)?;
    let compiled =
        compiled_privacy_profile_v1(PROTOCOL).wrap_err("load compiled ZK-ACE profile")?;

    submit_instruction(
        &client,
        Grant::account_permission(Permission::from(CanEnactGovernance), ALICE_ID.clone()),
        "grant privacy governance",
    )?;
    let proposed_at_height = next_incoming_height(&client)?;
    let activate_at_height = proposed_at_height
        .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
        .ok_or_else(|| eyre!("activation height overflow"))?;
    let proposed = compiled.activation_record(PrivacyProtocolLifecycleV1::Proposed(
        PrivacyProposedLifecycleV1 {
            proposed_at_height,
            activate_at_height,
        },
    ));
    submit_instruction(
        &client,
        RegisterPrivacyProtocolActivationV1::new(proposed),
        "register exact compiled ZK-ACE activation",
    )?;
    advance_to_height(&client, activate_at_height)?;
    let active = compiled.activation_record(PrivacyProtocolLifecycleV1::Active(
        PrivacyActiveLifecycleV1 {
            proposed_at_height,
            activated_at_height: activate_at_height,
            state_since_height: activate_at_height,
        },
    ));
    let row = client
        .get_privacy_capabilities()
        .wrap_err("query active ZK-ACE capability")?
        .protocols
        .into_iter()
        .find(|row| row.protocol_id == PROTOCOL)
        .ok_or_else(|| eyre!("ZK-ACE capability row missing"))?;
    ensure!(row.activation == Some(active), "ZK-ACE activation drifted");

    let first_witness = witness(0x11);
    let initial_policy = policy(
        &first_witness,
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        PrivacyZkAcePolicyLifecycleV1::Active,
    );
    submit_instruction(
        &client,
        RegisterPrivacyZkAcePolicyV1::new(initial_policy.clone()),
        "register canonical ZK-ACE policy",
    )?;

    let accepted = build_transfer(
        &client,
        initial_policy.clone(),
        first_witness,
        genesis_hash,
        1,
        19,
    )?;
    let stale_witness = witness(0x11);
    let stale = build_transfer(
        &client,
        initial_policy.clone(),
        stale_witness,
        genesis_hash,
        2,
        7,
    )?;
    submit_signed(
        &client,
        accepted.signed_transaction(),
        "submit canonical ZK-ACE transfer",
    )?;
    wait_for_balance(&client, &asset_definition_id, &ALICE_ID, 81)?;
    wait_for_balance(&client, &asset_definition_id, &BOB_ID, 19)?;

    let replay_error = client
        .submit_transaction_blocking(accepted.signed_transaction())
        .expect_err("exact transaction replay must reject");
    let replay_report = eyre!(replay_error);
    ensure!(
        ["already committed", "already enqueued", "already present"]
            .iter()
            .any(|needle| error_chain_contains(&replay_report, needle)),
        "exact replay rejected for the wrong reason: {replay_report:?}"
    );

    let rotated_witness = witness(0x31);
    let rotated_policy = policy(
        &rotated_witness,
        initial_policy.authorization_epoch + 1,
        PrivacyZkAcePolicyLifecycleV1::Active,
    );
    submit_instruction(
        &client,
        RotatePrivacyZkAcePolicyV1::new(initial_policy.record_digest, rotated_policy.clone()),
        "rotate canonical ZK-ACE policy",
    )?;
    let stale_error = client
        .submit_transaction_blocking(stale.signed_transaction())
        .expect_err("pre-rotation proof must reject after policy drift");
    let stale_report = eyre!(stale_error);
    ensure!(
        error_chain_contains(
            &stale_report,
            "does not exactly match authoritative policy state"
        ),
        "policy-drift proof rejected for the wrong reason: {stale_report:?}"
    );
    wait_for_balance(&client, &asset_definition_id, &ALICE_ID, 81)?;
    wait_for_balance(&client, &asset_definition_id, &BOB_ID, 19)?;

    let revoked_policy = PrivacyZkAcePolicyRecordV1::new(
        rotated_policy.policy_id,
        rotated_policy.identity_commitment,
        rotated_policy.policy_digest,
        rotated_policy.authorization_epoch + 1,
        rotated_policy.asset_definition_id.clone(),
        rotated_policy.source_allowlist.clone(),
        PrivacyZkAcePolicyLifecycleV1::Revoked,
    )
    .expect("valid revoked successor");
    submit_instruction(
        &client,
        RevokePrivacyZkAcePolicyV1::new(rotated_policy.record_digest, revoked_policy.clone()),
        "revoke canonical ZK-ACE policy",
    )?;
    ensure!(
        ZkAcePrivacyTransferV1::try_new(revoked_policy, ALICE_ID.clone(), BOB_ID.clone(), 1,)
            .is_err(),
        "client builder admitted a revoked policy"
    );

    Ok(())
}
