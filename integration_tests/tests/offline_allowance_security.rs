#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Security regressions for offline allowance admission and escrow enforcement.

use std::time::{SystemTime, UNIX_EPOCH};

use eyre::Result;
use integration_tests::sandbox::start_network_async_or_skip;
use iroha::data_model::{
    account::AccountId,
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    isi::{Register, offline::RegisterOfflineAllowance},
    metadata::Metadata,
    offline::{
        OFFLINE_REJECTION_REASON_PREFIX, OfflineAllowanceCommitment, OfflineWalletCertificate,
        OfflineWalletPolicy,
    },
    prelude::*,
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_primitives::numeric::NumericSpec;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};

fn now_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after epoch")
            .as_millis(),
    )
    .expect("millisecond timestamp should fit u64")
}

fn signed_certificate_for(
    definition_id: AssetDefinitionId,
    now_ms: u64,
    controller: AccountId,
) -> OfflineWalletCertificate {
    let spend_keys = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
    let mut certificate = OfflineWalletCertificate {
        controller: controller.clone(),
        operator: ALICE_ID.clone(),
        allowance: OfflineAllowanceCommitment {
            asset: AssetId::new(definition_id, controller),
            amount: Numeric::new(100, 0),
            commitment: vec![0xAB; 32],
        },
        spend_public_key: spend_keys.public_key().clone(),
        attestation_report: Vec::new(),
        issued_at_ms: now_ms.saturating_sub(1_000),
        expires_at_ms: now_ms + 3_600_000,
        policy: OfflineWalletPolicy {
            max_balance: Numeric::new(200, 0),
            max_tx_value: Numeric::new(100, 0),
            expires_at_ms: now_ms + 3_600_000,
        },
        operator_signature: Signature::from_bytes(&[0; 64]),
        metadata: Metadata::default(),
        verdict_id: None,
        attestation_nonce: None,
        refresh_at_ms: None,
    };
    certificate.operator_signature = Signature::new(
        ALICE_KEYPAIR.private_key(),
        &certificate
            .operator_signing_bytes()
            .expect("operator signing bytes"),
    );
    certificate
}

#[tokio::test]
async fn register_offline_allowance_rejects_non_controller_authority() -> Result<()> {
    init_instruction_registry();
    let now_ms = now_millis();
    let definition_id: AssetDefinitionId = "offsecunauth#wonderland".parse()?;
    let certificate = signed_certificate_for(definition_id.clone(), now_ms, ALICE_ID.clone());

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::new(
            definition_id,
            NumericSpec::integer(),
        )));
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(register_offline_allowance_rejects_non_controller_authority),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let bob_client = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
    let err = bob_client
        .submit_blocking(RegisterOfflineAllowance { certificate })
        .expect_err("non-controller authority must be rejected");
    let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}unauthorized_controller");
    assert!(
        err.to_string().contains(&expected),
        "unexpected error: {err}"
    );

    Ok(())
}

#[tokio::test]
async fn register_offline_allowance_rejects_missing_escrow_binding() -> Result<()> {
    init_instruction_registry();
    let now_ms = now_millis();
    let definition_id: AssetDefinitionId = "offsecescrow#wonderland".parse()?;
    let certificate = signed_certificate_for(definition_id.clone(), now_ms, ALICE_ID.clone());

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::new(
            definition_id,
            NumericSpec::integer(),
        )));
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(register_offline_allowance_rejects_missing_escrow_binding),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let err = network
        .client()
        .submit_blocking(RegisterOfflineAllowance { certificate })
        .expect_err("missing escrow binding must be rejected");
    let expected = format!("{OFFLINE_REJECTION_REASON_PREFIX}escrow_missing");
    assert!(
        err.to_string().contains(&expected),
        "unexpected error: {err}"
    );

    Ok(())
}
