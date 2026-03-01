#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Security regressions for offline allowance admission and escrow enforcement.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eyre::Result;
use integration_tests::sandbox::start_network_async_or_skip;
use iroha::{
    data_model::{
        ValidationFail,
        account::AccountId,
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        isi::{
            Mint, Register, SetKeyValue,
            offline::{ReclaimExpiredOfflineAllowance, RegisterOfflineAllowance},
        },
        metadata::Metadata,
        offline::{
            OFFLINE_ASSET_ENABLED_METADATA_KEY, OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY,
            OFFLINE_LINEAGE_EPOCH_KEY, OFFLINE_LINEAGE_SCOPE_KEY, OFFLINE_REJECTION_REASON_PREFIX,
            OfflineAllowanceCommitment, OfflineWalletCertificate, OfflineWalletPolicy,
        },
        prelude::*,
    },
    query::QueryError,
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::query::{
    error::QueryExecutionFail, offline::FindOfflineAllowanceByCertificateId,
};
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
    let mut metadata = Metadata::default();
    metadata.insert(
        OFFLINE_LINEAGE_SCOPE_KEY
            .parse()
            .expect("offline.lineage.scope metadata key should parse"),
        Json::new(format!("offline_allowance_security::{definition_id}")),
    );
    metadata.insert(
        OFFLINE_LINEAGE_EPOCH_KEY
            .parse()
            .expect("offline.lineage.epoch metadata key should parse"),
        Json::new("1".to_owned()),
    );
    metadata.insert(
        OFFLINE_BUILD_CLAIM_MIN_BUILD_NUMBER_KEY
            .parse()
            .expect("offline.build_claim.min_build_number metadata key should parse"),
        Json::new("1".to_owned()),
    );
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
        metadata,
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

#[tokio::test]
async fn register_topup_expire_then_reclaim_restores_controller_balance() -> Result<()> {
    init_instruction_registry();
    let definition_id: AssetDefinitionId = "offsecreclaime2e#wonderland".parse()?;
    let initial_balance = Numeric::new(100, 0);
    let allowance_amount = Numeric::new(50, 0);
    let controller_asset_id = AssetId::new(definition_id.clone(), ALICE_ID.clone());

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::new(
            definition_id.clone(),
            NumericSpec::integer(),
        )))
        .with_genesis_instruction(SetKeyValue::asset_definition(
            definition_id.clone(),
            OFFLINE_ASSET_ENABLED_METADATA_KEY
                .parse()
                .expect("offline.enabled metadata key should parse"),
            Json::new(true),
        ))
        .with_genesis_instruction(Mint::asset_numeric(
            initial_balance.clone(),
            controller_asset_id.clone(),
        ));
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(register_topup_expire_then_reclaim_restores_controller_balance),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let initial = client
        .query_single(FindAssetById::new(controller_asset_id.clone()))?
        .value()
        .clone();
    assert_eq!(initial, initial_balance);

    // Build the certificate after the network is ready so startup delays don't consume its TTL.
    let now_ms = now_millis();
    let mut certificate = signed_certificate_for(definition_id.clone(), now_ms, ALICE_ID.clone());
    certificate.allowance.amount = allowance_amount.clone();
    certificate.policy.max_balance = allowance_amount.clone();
    certificate.policy.max_tx_value = allowance_amount.clone();
    certificate.expires_at_ms = now_ms + 15_000;
    certificate.policy.expires_at_ms = certificate.expires_at_ms;
    certificate.operator_signature = Signature::new(
        ALICE_KEYPAIR.private_key(),
        &certificate
            .operator_signing_bytes()
            .expect("operator signing bytes"),
    );
    let certificate_id = certificate.certificate_id();
    let expires_at_ms = certificate.expires_at_ms;

    client.submit_blocking(RegisterOfflineAllowance { certificate })?;
    network.ensure_blocks(2).await?;

    let after_topup = client
        .query_single(FindAssetById::new(controller_asset_id.clone()))?
        .value()
        .clone();
    assert_eq!(after_topup, Numeric::new(50, 0));

    let mut records = client
        .query(FindOfflineAllowanceByCertificateId::new(
            certificate_id.clone(),
        ))
        .execute_all()?;
    let record = records
        .pop()
        .expect("offline allowance should exist after top-up");
    assert_eq!(record.remaining_amount, allowance_amount);

    if let Some(wait_ms) = expires_at_ms
        .checked_sub(now_millis())
        .map(|ms| ms.saturating_add(250))
        .filter(|ms| *ms > 0)
    {
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
    }

    loop {
        match client.submit_blocking(ReclaimExpiredOfflineAllowance { certificate_id }) {
            Ok(_) => break,
            Err(err) if err.to_string().contains("allowance_not_expired") => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(err) => return Err(err.into()),
        }
    }
    network.ensure_blocks(3).await?;

    let after_reclaim = client
        .query_single(FindAssetById::new(controller_asset_id))?
        .value()
        .clone();
    assert_eq!(after_reclaim, initial_balance);

    match client
        .query(FindOfflineAllowanceByCertificateId::new(certificate_id))
        .execute_all()
    {
        Ok(records) if records.is_empty() => {}
        Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))) => {}
        Ok(records) => panic!(
            "offline allowance should be removed after reclaim, found {} record(s)",
            records.len()
        ),
        Err(err) => panic!("unexpected query error after reclaim: {err}"),
    }

    Ok(())
}
