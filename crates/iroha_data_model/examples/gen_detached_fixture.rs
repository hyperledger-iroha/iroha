//! Generates the canonical detached-contract transaction fixture used by SDK tests.

use std::{str::FromStr as _, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    metadata::Metadata,
    smart_contract::ContractAddress,
    transaction::{
        Executable,
        executable::{ContractArgumentRecord, ContractInvocation},
        signed::TransactionBuilder,
    },
};
use iroha_primitives::json::Json;
use iroha_version::codec::EncodeVersioned as _;

fn main() {
    let authority_keypair =
        KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519).expect("authority key");
    let placeholder_keypair =
        KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519).expect("placeholder key");
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let contract_address =
        ContractAddress::from_str("tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8")
            .expect("fixture contract address");
    let fee_sponsor = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";
    let expected_code_hash = Hash::new(b"detached-fixture-contract-code");
    let payload = format!(r#"{{"amount":"750","merchant_account_id":"{fee_sponsor}"}}"#,);
    let mut metadata = Metadata::default();
    for (key, value) in [
        ("contract_address", contract_address.to_string()),
        ("contract_code_hash", expected_code_hash.to_string()),
        ("contract_alias", "bisp::hbl.sbp".to_owned()),
        ("contract_entrypoint", "spend_to_merchant".to_owned()),
        ("gas_asset_id", "62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_owned()),
        ("fee_sponsor", fee_sponsor.to_owned()),
    ] {
        metadata.insert(key.parse().expect("metadata key"), Json::new(value));
    }
    metadata.insert(
        "contract_payload".parse().expect("metadata key"),
        Json::new(payload),
    );
    metadata.insert("gas_limit".parse().expect("metadata key"), 500_000_u64);
    let invocation = ContractInvocation {
        contract_address,
        expected_code_hash,
        entrypoint: "spend_to_merchant".to_owned(),
        arguments: Some(
            ContractArgumentRecord::try_new(vec![0x01]).expect("bounded contract arguments"),
        ),
    };
    let mut builder = TransactionBuilder::new(
        ChainId::from("swift-detached-contract-fixture"),
        authority.clone(),
    )
    .with_metadata(metadata)
    .with_executable(Executable::ContractCall(invocation));
    builder.set_creation_time(Duration::from_millis(4_102_444_800_000));
    builder.set_ttl(Duration::from_millis(120_000));
    let scaffold = builder
        .try_sign(placeholder_keypair.private_key())
        .expect("placeholder signature")
        .with_authority(authority.clone());
    let scaffold_bytes = scaffold.encode_versioned();
    let payload_hash = HashOf::new(scaffold.payload());
    let signature = Signature::try_new(authority_keypair.private_key(), payload_hash.as_ref())
        .expect("authority signature");
    let payload_bytes = norito::codec::encode_adaptive(scaffold.payload());
    let signed = TransactionBuilder::decode_payload(&payload_bytes)
        .expect("decode payload")
        .build_with_signature(signature.clone());
    signed.verify_signature().expect("final signature");

    println!("authority={authority}");
    println!("scaffold_b64={}", STANDARD.encode(scaffold_bytes));
    println!("signing_hash_hex={}", hex::encode(payload_hash.as_ref()));
    println!("signature_b64={}", STANDARD.encode(signature.payload()));
    println!(
        "scaffold_entrypoint_hash_hex={}",
        hex::encode(scaffold.hash_as_entrypoint().as_ref())
    );
    println!(
        "transaction_hash_hex={}",
        hex::encode(signed.hash().as_ref())
    );
    println!(
        "entrypoint_hash_hex={}",
        hex::encode(signed.hash_as_entrypoint().as_ref())
    );
    println!("signed_b64={}", STANDARD.encode(signed.encode_versioned()));
}
