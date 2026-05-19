/// Generates Norito-encoded fixtures for Kotlin SDK parity tests.
///
/// Each subcommand outputs the wire payload hex on the first line,
/// followed by input parameters the Kotlin encoder needs.
///
/// When the Rust data model changes, this binary produces different
/// bytes, causing the Kotlin parity tests to fail until the
/// corresponding encoder is updated.
use std::env;

use iroha_crypto::{PublicKey, default_bfv_programmed_hidden_program};
use iroha_crypto::{RamLfeBackend, RamLfeVerificationMode};
use iroha_data_model::account::{AccountId, NewAccount, OpaqueAccountId};
use iroha_data_model::asset::{AssetDefinitionId, AssetId};
use iroha_data_model::domain::DomainId;
use iroha_data_model::identifier::{
    IdentifierPolicyId, IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
};
use iroha_data_model::isi::identifier::ClaimIdentifier;
use iroha_data_model::isi::register::{Register, RegisterBox};
use iroha_data_model::isi::transfer::{Transfer, TransferBox};
use iroha_data_model::name::Name;
use iroha_data_model::nexus::UniversalAccountId;
use iroha_data_model::prelude::Numeric;
use iroha_data_model::ram_lfe::{
    RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeOutputOpeningPayload,
    RamLfeProgramId, RamLfeReceiptAttestation,
};

/// Well-known public key shared with the Kotlin parity tests.
const PARITY_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

fn parity_account_id() -> AccountId {
    let pk: PublicKey = PARITY_PUBLIC_KEY.parse().expect("parse public key");
    AccountId::new(pk)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!(
            "Usage: {} <register-account|transfer-asset|claim-identifier|hidden-ram-fhe-program>",
            args[0]
        );
        std::process::exit(1);
    }
    match args[1].as_str() {
        "register-account" => emit_register_account(),
        "transfer-asset" => emit_transfer_asset(),
        "claim-identifier" => emit_claim_identifier(),
        "hidden-ram-fhe-program" => emit_hidden_ram_fhe_program(),
        other => {
            eprintln!("Unknown fixture: {other}");
            std::process::exit(1);
        }
    }
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
    let account_id = parity_account_id();
    let domain = DomainId::try_new("wonderland", "universal").expect("domain id");
    let name: Name = "rose".parse().unwrap();
    let asset_def_id = AssetDefinitionId::new(domain, name);
    let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
    let amount = Numeric::new(100_i64, 0);
    let destination = account_id.clone();

    let transfer = Transfer::asset_numeric(asset_id, amount, destination);
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
            signature: iroha_crypto::Signature::from_bytes(&signature_bytes),
        },
        opaque_id,
        receipt_hash: dummy_hash,
        uaid,
        account_id: account_id.clone(),
    };
    let signature = iroha_crypto::Signature::from_bytes(&signature_bytes);

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
