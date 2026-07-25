/// Generates Norito-encoded fixtures for Kotlin SDK parity tests.
///
/// Each subcommand outputs the wire payload hex on the first line,
/// followed by input parameters the Kotlin encoder needs.
///
/// When the Rust data model changes, this binary produces different
/// bytes, causing the Kotlin parity tests to fail until the
/// corresponding encoder is updated.
use std::env;

use iroha_crypto::{Hash, PublicKey, default_bfv_programmed_hidden_program, sha256};
use iroha_crypto::{RamLfeBackend, RamLfeVerificationMode};
use iroha_data_model::account::{
    AccountId, NewAccount, OpaqueAccountId, address::ChainDiscriminantGuard,
};
use iroha_data_model::asset::{AssetBalanceScope, AssetDefinitionId, AssetId};
use iroha_data_model::domain::DomainId;
use iroha_data_model::identifier::{
    IdentifierPolicyId, IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
};
use iroha_data_model::isi::identifier::ClaimIdentifier;
use iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation;
use iroha_data_model::isi::register::{Register, RegisterBox};
use iroha_data_model::isi::transfer::{Transfer, TransferBox};
use iroha_data_model::name::Name;
use iroha_data_model::nexus::{DataSpaceId, UniversalAccountId};
use iroha_data_model::offline::{
    KagemushaDevicePublicKeyV2, OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
    OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_SCHEME,
    OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM, OfflineAndroidKeyMintChallenge,
    OfflineDeviceAttestationRegistration,
};
use iroha_data_model::prelude::Quantity;
use iroha_data_model::ram_lfe::{
    RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeOutputOpeningPayload,
    RamLfeProgramId, RamLfeReceiptAttestation,
};

/// Well-known public key shared with the Kotlin parity tests.
const PARITY_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;

fn parity_account_id() -> AccountId {
    let pk: PublicKey = PARITY_PUBLIC_KEY.parse().expect("parse public key");
    AccountId::new(pk)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!(
            "Usage: {} <register-account|transfer-asset|transfer-asset-scoped|claim-identifier|hidden-ram-fhe-program|offline-device-attestation>",
            args[0]
        );
        std::process::exit(1);
    }
    match args[1].as_str() {
        "register-account" => emit_register_account(),
        "transfer-asset" => emit_transfer_asset(),
        "transfer-asset-scoped" => emit_transfer_asset_scoped(),
        "claim-identifier" => emit_claim_identifier(),
        "hidden-ram-fhe-program" => emit_hidden_ram_fhe_program(),
        "offline-device-attestation" => emit_offline_device_attestation(),
        other => {
            eprintln!("Unknown fixture: {other}");
            std::process::exit(1);
        }
    }
}

fn emit_offline_device_attestation() {
    // Synthetic unit-test bytes only. This fixture proves canonical wire parity and is not
    // physical-device attestation or release evidence.
    const P256_GENERATOR: &str = concat!(
        "04",
        "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
        "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
    );
    let account_id = parity_account_id();
    let assertion_public_key = hex::decode(P256_GENERATOR).expect("P-256 generator hex");
    let signing_certificate_sha256 = sha256(b"abi20-unit-test-signing-certificate").to_vec();
    let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&assertion_public_key)
        .expect("canonical P-256 device authority");
    let recent_block_hash = Hash::new(b"abi20-unit-test-block");
    let challenge = OfflineAndroidKeyMintChallenge {
        version: 1,
        device_id: "abi20-android-unit-test-device".to_owned(),
        account_id: account_id.clone(),
        asset_definition_id: None,
        ios_team_id: None,
        ios_bundle_id: None,
        ios_environment: None,
        android_package_name: Some("org.hyperledger.iroha.abi20.fixture".to_owned()),
        android_signing_certificate_sha256: Some(signing_certificate_sha256.clone()),
        public_key: public_key.clone(),
        assertion_scheme: OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_SCHEME.to_owned(),
        assertion_key_algorithm: OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM
            .to_owned(),
        assertion_usage_count_limit: Some(1),
        one_use: true,
        recent_block_height: 42,
        recent_block_hash,
        expires_at_ms: 2_000_000_000_000,
    };
    let challenge_hash = challenge
        .canonical_challenge_hash()
        .expect("encode Android KeyMint challenge");
    let attestation_report = b"abi20-unit-test-not-physical-attestation-evidence".to_vec();
    let attestation_report_hash = Hash::new(&attestation_report);
    let mut evidence = b"offline-device-attestation-evidence-v1".to_vec();
    evidence.extend_from_slice(attestation_report_hash.as_ref());
    let evidence_hash = Hash::new(&evidence);
    let registration = OfflineDeviceAttestationRegistration {
        version: 1,
        platform: OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM.to_owned(),
        key_id: hex::encode(sha256(&assertion_public_key)),
        device_id: challenge.device_id,
        account_id: account_id.clone(),
        asset_definition_id: None,
        ios_team_id: None,
        ios_bundle_id: None,
        ios_environment: None,
        android_package_name: challenge.android_package_name,
        android_signing_certificate_sha256: Some(signing_certificate_sha256),
        public_key,
        assertion_scheme: challenge.assertion_scheme,
        assertion_key_algorithm: challenge.assertion_key_algorithm,
        assertion_public_key,
        assertion_usage_count_limit: Some(1),
        one_use: true,
        challenge_hash,
        attestation_report_hash,
        attestation_report,
        evidence_hash,
        evidence,
        recent_block_height: 42,
        recent_block_hash,
        expires_at_ms: 2_000_000_000_000,
    };
    let registration_archive = norito::to_bytes(&registration).expect("encode registration");
    let registration_id = Hash::new(&registration_archive);
    let instruction_archive =
        norito::to_bytes(&RegisterOfflineDeviceAttestation::new(registration))
            .expect("encode registration instruction");

    println!("{}", hex::encode(registration_archive));
    println!("{}", hex::encode(instruction_archive));
    println!("{}", hex::encode(challenge_hash.as_ref()));
    println!("{account_id}");
    println!("{}", hex::encode(registration_id.as_ref()));
}

fn emit_hidden_ram_fhe_program() {
    let program = default_bfv_programmed_hidden_program();
    let encoded = norito::to_bytes(&program).expect("encode HiddenRamFheProgram");
    println!("{}", hex::encode(encoded));
}

fn emit_register_account() {
    let account_id = parity_account_id();
    let new_account = NewAccount::new(account_id);
    let register_box = RegisterBox::Account(Register::account(new_account));
    let encoded = norito::to_bytes(&register_box).expect("encode RegisterBox");
    println!("{}", hex::encode(encoded));
}

fn emit_transfer_asset() {
    let _chain_discriminant = ChainDiscriminantGuard::enter(TAIRA_CHAIN_DISCRIMINANT);
    let account_id = parity_account_id();
    let domain = DomainId::try_new("wonderland", "universal").expect("domain id");
    let name: Name = "rose".parse().unwrap();
    let asset_def_id = AssetDefinitionId::new(domain, name);
    let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
    let amount = Quantity::from(100_u64);
    let destination = account_id.clone();

    let transfer = Transfer::asset_quantity(asset_id, amount, destination);
    let transfer_box: TransferBox = transfer.into();
    let encoded = norito::to_bytes(&transfer_box).expect("encode TransferBox");

    // Line 1: wire payload hex
    println!("{}", hex::encode(encoded));
    // Line 2: asset ID string (<base58-def>#<i105-account>)
    println!("{}#{}", asset_def_id, account_id);
    // Line 3: amount
    println!("100");
    // Line 4: destination account I105
    println!("{}", account_id);
}

fn emit_transfer_asset_scoped() {
    let _chain_discriminant = ChainDiscriminantGuard::enter(TAIRA_CHAIN_DISCRIMINANT);
    let account_id = parity_account_id();
    let domain = DomainId::try_new("wonderland", "universal").expect("domain id");
    let name: Name = "rose".parse().unwrap();
    let asset_def_id = AssetDefinitionId::new(domain, name);
    let asset_id = AssetId::with_scope(
        asset_def_id.clone(),
        account_id.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(42)),
    );
    let amount = Quantity::from(100_u64);
    let destination = account_id.clone();

    let transfer = Transfer::asset_quantity(asset_id, amount, destination);
    let transfer_box: TransferBox = transfer.into();
    let encoded = norito::to_bytes(&transfer_box).expect("encode scoped TransferBox");

    println!("{}", hex::encode(encoded));
    println!("{}#{}#dataspace:42", asset_def_id, account_id);
    println!("100");
    println!("{}", account_id);
}

fn emit_claim_identifier() {
    let account_id = parity_account_id();
    let policy_id = IdentifierPolicyId::new("phone".parse().unwrap(), "e164".parse().unwrap());
    let program_id: RamLfeProgramId = "parity_test".parse().unwrap();
    let dummy_hash = iroha_crypto::Hash::new([0xAB; 32]);
    let execution = RamLfeExecutionReceiptPayload {
        program_id: program_id.clone(),
        program_digest: dummy_hash,
        backend: RamLfeBackend::HkdfSha3_512PrfV1,
        verification_mode: RamLfeVerificationMode::Signed,
        input_ciphertext_hash: dummy_hash,
        output_ciphertext_hash: dummy_hash,
        parameter_digest: dummy_hash,
        evaluation_key_digest: dummy_hash,
        output_hash: dummy_hash,
        associated_data_hash: dummy_hash,
        executed_at_ms: 1_735_000_000_000,
        expires_at_ms: None,
    };
    let opaque_id = OpaqueAccountId::from_hash(dummy_hash);
    let uaid = UniversalAccountId::from_hash(dummy_hash);
    // Deterministic signature bytes (64 bytes of 0xCD).
    let signature_bytes = [0xCD_u8; 64];

    let receipt_payload = IdentifierResolutionReceiptPayload {
        policy_id,
        execution,
        opening: RamLfeOutputOpening {
            payload: RamLfeOutputOpeningPayload {
                program_id,
                input_ciphertext_hash: dummy_hash,
                output_ciphertext_hash: dummy_hash,
                parameter_digest: dummy_hash,
                evaluation_key_digest: dummy_hash,
                opened_output_hash: dummy_hash,
                opened_at_ms: 1_735_000_000_000,
                expires_at_ms: None,
            },
            signature: iroha_crypto::Signature::try_from_bytes(&signature_bytes)
                .expect("Kotlin fixture opening signature is non-empty and nonzero"),
        },
        opaque_id,
        receipt_hash: dummy_hash,
        uaid,
        account_id: account_id.clone(),
    };
    let signature = iroha_crypto::Signature::try_from_bytes(&signature_bytes)
        .expect("Kotlin fixture receipt signature is non-empty and nonzero");

    let receipt = IdentifierResolutionReceipt {
        payload: receipt_payload,
        attestation: RamLfeReceiptAttestation::Signed(signature),
    };

    let claim = ClaimIdentifier {
        account: account_id.clone(),
        receipt,
    };
    let encoded = norito::to_bytes(&claim).expect("encode ClaimIdentifier");

    // Line 1: full wire payload hex
    println!("{}", hex::encode(encoded));
    // Line 2: account I105
    println!("{}", account_id);
    // Line 3: signature bytes hex
    println!("{}", hex::encode(signature_bytes));
    // Line 4: canonical hash hex used by receipt fields
    println!("{}", hex::encode(&dummy_hash.as_ref()[..]));
}
