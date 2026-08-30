//! Taira-shaped local-node capability and native-builder coverage for ZK-ACE.
#![cfg(feature = "zk-stark")]
use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        asset::AssetBalanceScope,
        metadata::Metadata,
        prelude::{AssetDefinitionId, DomainId},
        privacy::{
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1, PrivacyCompiledProfileResultV1,
            PrivacyCompiledProfileSnapshotV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1,
            PrivacyProtocolIdV1, PrivacyZkAcePolicyLifecycleV1, PrivacyZkAcePolicyRecordV1,
        },
        transaction::FeePaymentIntent,
    },
};
use iroha_core::privacy_profiles::{
    compiled_privacy_profile_snapshot_result_v1, compiled_privacy_profile_v1,
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use std::{
    num::NonZeroU32,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zk_ace_prover::{
    ZkAcePrivacyActionTransactionContextV1, ZkAcePrivacyTransferV1, ZkAcePrivacyWitnessV1,
    build_signed_zk_ace_privacy_transfer_v1,
};

const TEST_NAME: &str = "zk_ace_privacy_transfer_builds_for_taira_localnet";
const PROTOCOL: PrivacyProtocolIdV1 = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;

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
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("default domain"),
        "zkace_typed".parse().expect("asset name"),
    )
}

fn canonical_genesis_hash(client: &Client) -> Result<[u8; 32]> {
    // NetworkId is the exact canonical genesis-header hash, not an operator label. Reading the
    // typed client identity avoids the retired unbounded FindBlocks path while preserving the
    // same lineage binding used by transaction validation.
    let hash = *client.network_id.as_bytes();
    ensure!(hash != [0; 32], "canonical genesis hash is zero");
    Ok(hash)
}

fn witness(seed: u8) -> ZkAcePrivacyWitnessV1 {
    ZkAcePrivacyWitnessV1::try_new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
    )
    .expect("valid localnet witness")
}

fn policy(witness: &ZkAcePrivacyWitnessV1) -> PrivacyZkAcePolicyRecordV1 {
    PrivacyZkAcePolicyRecordV1::new(
        PrivacyPolicyIdV1::new([0x41; 32]),
        witness.identity_commitment_v1(),
        PrivacyPolicyDigestV1::new([0x42; 32]),
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        asset_definition_id(),
        vec![ALICE_ID.clone()],
        PrivacyZkAcePolicyLifecycleV1::Active,
    )
    .expect("valid governed policy")
}

#[test]
fn zk_ace_privacy_transfer_builds_for_taira_localnet() -> Result<()> {
    require_test_network_feature("zk-stark")?;
    init_instruction_registry();

    let compiled = compiled_privacy_profile_v1(PROTOCOL)
        .map_err(|error| eyre!("load available ZK-ACE compiled profile: {error:?}"))?;
    let compiled_snapshot = PrivacyCompiledProfileSnapshotV1::from(compiled);
    ensure!(
        compiled_privacy_profile_snapshot_result_v1(PROTOCOL)
            == PrivacyCompiledProfileResultV1::Available(compiled_snapshot),
        "local ZK-ACE capability result is not the exact available profile"
    );

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        // Canonical Taira is a four-validator NPoS network. Permissioned consensus suppresses
        // the test builder's stake-validator genesis bootstrap while the public lane remains
        // stake-elected, so routed reads correctly fail closed with an empty authority pool.
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer.write(["zk", "stark", "enabled"], true);
        });
    let Some((network, _runtime)) = sandbox::start_network_blocking_or_skip(builder, TEST_NAME)?
    else {
        return Ok(());
    };
    let mut client = network.client();
    client.add_transaction_nonce = true;

    let row = client
        .get_privacy_capabilities()
        .wrap_err("query available ZK-ACE capability")?
        .protocols
        .into_iter()
        .find(|row| row.protocol_id == PROTOCOL)
        .ok_or_else(|| eyre!("ZK-ACE capability row missing"))?;
    ensure!(
        row.compiled_profile == PrivacyCompiledProfileResultV1::Available(compiled_snapshot),
        "network exposed a different ZK-ACE compiled profile: {:?}",
        row.compiled_profile
    );
    ensure!(
        row.activation.is_none(),
        "fresh ZK-ACE localnet unexpectedly has an activation: {:?}",
        row.activation
    );

    let witness = witness(0x11);
    let transfer = ZkAcePrivacyTransferV1::try_new(
        policy(&witness),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Global,
        19,
    )
    .wrap_err("construct governed ZK-ACE transfer")?;
    let creation_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before Unix epoch")?;
    let genesis_hash = canonical_genesis_hash(&client)?;
    let signed = build_signed_zk_ace_privacy_transfer_v1(
        ZkAcePrivacyActionTransactionContextV1 {
            network_id: client.network_id,
            authority: ALICE_ID.clone(),
            creation_time,
            time_to_live: Some(Duration::from_secs(3_600)),
            nonce: NonZeroU32::new(1),
            fee_payment: no_fee(),
            metadata: Metadata::default(),
        },
        transfer,
        witness,
        genesis_hash,
        ALICE_KEYPAIR.private_key(),
    )
    .wrap_err("build signed native ZK-ACE transfer")?;
    ensure!(
        signed.transaction_hash() != [0; 32]
            && signed.transaction_intent_digest() != [0; 32]
            && signed.statement_digest() != [0; 32]
            && signed.proof_envelope_hash() != [0; 32]
            && signed.statement_bytes() > 0
            && signed.proof_bytes() > 0
            && signed.encoded_proof_envelope_bytes() > 0
            && signed.effect().amount == 19
            && signed.effect().source == ALICE_ID.clone()
            && signed.effect().destination == BOB_ID.clone(),
        "signed ZK-ACE builder output is incomplete or bound to the wrong effect"
    );
    Ok(())
}
