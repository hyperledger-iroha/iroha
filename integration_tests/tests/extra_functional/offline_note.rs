#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Offline Note real-proof flow over a four-peer network.

use eyre::{OptionExt, Result};
use integration_tests::sandbox;
use iroha::{
    crypto::{Algorithm, Hash, KeyPair, Signature},
    data_model::{
        isi::{
            offline::AuditOfflineNote, offline::IssueOfflineNote, offline::RedeemOfflineNote,
            verifying_keys,
        },
        offline::{
            OFFLINE_ASSET_ENABLED_METADATA_KEY, OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            OfflineNoteAuditBundle, OfflineNoteAuditOutputClaim, OfflineNoteIssue,
            OfflineNoteIssuedClaim, OfflineNoteKeyCertificate, OfflineNoteRecursiveProof,
            OfflineNoteRedeem, offline_escrow_account_id,
        },
        prelude::*,
        proof::{ProofBox, VerifyingKeyId},
    },
};
use iroha_core::zk::{
    OFFLINE_NOTE_MAX_PROOF_BYTES, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, ZK_BACKEND_HALO2_IPA,
    derive_halo2_ipa_offline_note_proving_key_bytes, offline_note_recursive_vk_record,
    prove_offline_note_audit, prove_offline_note_redeem,
};
use iroha_primitives::json::Json;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID};
use tokio::task::spawn_blocking;

const OFFLINE_NOTE_VERIFIER_NAMESPACE: &str = "offline_note";
const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;

#[tokio::test]
async fn offline_note_issue_audit_redeem_real_proofs_on_four_peers() -> Result<()> {
    let context = stringify!(offline_note_issue_audit_redeem_real_proofs_on_four_peers);
    let verifier_id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
    let verifier_record = offline_note_recursive_vk_record(OFFLINE_NOTE_VERIFIER_NAMESPACE, 1)
        .map_err(|error| eyre::eyre!(error))?;
    let verifier_key = verifier_record
        .key
        .clone()
        .ok_or_eyre("Offline verifier record must embed its VK bytes")?;

    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "offline_xor".parse()?,
    );
    let alice_asset_id = AssetId::of(asset_definition_id.clone(), ALICE_ID.clone());
    let mut asset_definition =
        AssetDefinition::numeric(asset_definition_id.clone()).with_name("Offline XOR".to_owned());
    asset_definition.metadata.insert(
        OFFLINE_ASSET_ENABLED_METADATA_KEY
            .parse()
            .expect("offline metadata key parses"),
        Json::new(true),
    );

    let manage_offline_escrow = Permission::new("CanManageOfflineEscrow".into(), Json::new(()));
    let manage_verifying_keys = Permission::new("CanManageVerifyingKeys".into(), Json::new(()));
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(|layer| {
            layer
                .write(["zk", "halo2", "enabled"], true)
                .write(
                    ["zk", "halo2", "max_envelope_bytes"],
                    i64::from(OFFLINE_NOTE_MAX_PROOF_BYTES),
                )
                .write(
                    ["zk", "halo2", "max_proof_bytes"],
                    i64::from(OFFLINE_NOTE_MAX_PROOF_BYTES),
                )
                .write(
                    ["confidential", "verify_timeout_ms"],
                    PROOF_VERIFY_TIMEOUT_MS,
                );
        })
        .with_genesis_instruction(Register::asset_definition(asset_definition))
        .with_genesis_instruction(Mint::asset_numeric(
            Numeric::from(100_u64),
            alice_asset_id.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            manage_offline_escrow,
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            manage_verifying_keys,
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        ))
        .with_genesis_instruction(verifying_keys::RegisterVerifyingKey {
            id: verifier_id.clone(),
            record: verifier_record,
        });
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        return Ok(());
    };
    let client = network.client();
    let chain_id = network.chain_id();

    let result = spawn_blocking(move || -> Result<()> {
        let proving_key = derive_halo2_ipa_offline_note_proving_key_bytes(&verifier_key)
            .map_err(|error| eyre::eyre!(error))?;
        let input_certificate = signed_certificate(0xA1, "input-note-key")?;
        let output_certificate = signed_certificate(0xB2, "output-note-key")?;
        let issued_note_commitment = Hash::new(b"offline-issued-note");
        let output_note_commitment = Hash::new(b"offline-audited-output-note");
        let issued_nullifier = Hash::new(b"offline-issued-nullifier");
        let output_nullifier = Hash::new(b"offline-output-nullifier");
        let amount = Numeric::from(10_u64);

        let issue = OfflineNoteIssue {
            note_commitment: issued_note_commitment,
            key_certificate: input_certificate.clone(),
            asset: alice_asset_id.clone(),
            amount: amount.clone(),
        };
        client.submit_blocking(IssueOfflineNote::new(issue.clone()))?;
        assert_asset_balance(&client, &alice_asset_id, Numeric::from(90_u64))?;

        let input_claim = OfflineNoteIssuedClaim::from_issue(&issue)?;
        let output_claim = OfflineNoteAuditOutputClaim {
            note_commitment: output_note_commitment,
            key_certificate: output_certificate.clone(),
            asset: alice_asset_id.clone(),
            amount: amount.clone(),
        };
        let mut audit = OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-audit-token"),
            sender_key_certificate: input_certificate,
            input_nullifiers: vec![issued_nullifier],
            input_claims: vec![input_claim],
            output_commitments: vec![output_note_commitment],
            output_claims: vec![output_claim],
            recursive_proof: placeholder_recursive_proof(verifier_id.clone()),
        };
        let audit_inputs_hash = audit.public_inputs_hash()?;
        let audit_proof = prove_offline_note_audit(
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            &verifier_key,
            &audit,
            Some(&proving_key),
        )
        .map_err(|error| eyre::eyre!(error))?;
        audit.recursive_proof = OfflineNoteRecursiveProof {
            verifier_key_id: verifier_id.clone(),
            public_inputs_hash: audit_inputs_hash,
            proof: audit_proof,
        };
        client.submit_blocking(AuditOfflineNote::new(audit))?;
        assert_asset_balance(&client, &alice_asset_id, Numeric::from(90_u64))?;

        let mut redeem = OfflineNoteRedeem {
            source_note_commitment: output_note_commitment,
            input_nullifiers: vec![output_nullifier],
            sender_key_certificate: output_certificate,
            recipient: ALICE_ID.clone(),
            asset: alice_asset_id.clone(),
            amount,
            recursive_proof: placeholder_recursive_proof(verifier_id.clone()),
        };
        let redeem_inputs_hash = redeem.public_inputs_hash()?;
        let redeem_proof = prove_offline_note_redeem(
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            &verifier_key,
            &redeem,
            Some(&proving_key),
        )
        .map_err(|error| eyre::eyre!(error))?;
        redeem.recursive_proof = OfflineNoteRecursiveProof {
            verifier_key_id: verifier_id,
            public_inputs_hash: redeem_inputs_hash,
            proof: redeem_proof,
        };
        client.submit_blocking(RedeemOfflineNote::new(redeem.clone()))?;
        assert_asset_balance(&client, &alice_asset_id, Numeric::from(100_u64))?;

        let replay_error = client
            .submit_blocking(RedeemOfflineNote::new(redeem))
            .expect_err("replaying an Offline redemption must be rejected");
        let replay_message = replay_error.to_string();
        assert!(
            replay_message.contains("duplicate_redeem")
                || replay_message.contains("duplicate_nullifier"),
            "unexpected replay error: {replay_message}"
        );

        let escrow = offline_escrow_account_id(&chain_id, &asset_definition_id);
        let escrow_asset_id = AssetId::of(asset_definition_id, escrow);
        assert_asset_balance(&client, &escrow_asset_id, Numeric::zero())?;
        Ok(())
    })
    .await?;

    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}

fn signed_certificate(seed: u8, key_id: &str) -> Result<OfflineNoteKeyCertificate> {
    let note_key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    let (_algorithm, public_key) = note_key.public_key().to_bytes();
    let mut certificate = OfflineNoteKeyCertificate {
        version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
        platform: "integration-test".to_owned(),
        key_id: key_id.to_owned(),
        device_id: "four-peer-offline".to_owned(),
        account_id: ALICE_ID.clone(),
        public_key: public_key.to_vec(),
        assertion_scheme: "integration-test-hardware-one-use".to_owned(),
        assertion_key_algorithm: "ed25519-test".to_owned(),
        assertion_public_key: public_key.to_vec(),
        assertion_usage_count_limit: Some(1),
        one_use: true,
        issuer_signature: Signature::new(ALICE_KEYPAIR.private_key(), b"placeholder"),
    };
    let payload = certificate.signing_bytes()?;
    certificate.issuer_signature = Signature::new(ALICE_KEYPAIR.private_key(), &payload);
    Ok(certificate)
}

fn placeholder_recursive_proof(verifier_key_id: VerifyingKeyId) -> OfflineNoteRecursiveProof {
    OfflineNoteRecursiveProof {
        verifier_key_id,
        public_inputs_hash: Hash::new(b"offline-placeholder-public-inputs"),
        proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.into(), Vec::new()),
    }
}

fn assert_asset_balance(
    client: &iroha::client::Client,
    asset_id: &AssetId,
    expected: Numeric,
) -> Result<()> {
    let assets = client.query(FindAssets::new()).execute_all()?;
    let actual = assets
        .iter()
        .find(|asset| asset.id() == asset_id)
        .map(|asset| asset.value().clone())
        .unwrap_or_else(Numeric::zero);
    assert_eq!(actual, expected, "unexpected balance for {asset_id}");
    Ok(())
}
