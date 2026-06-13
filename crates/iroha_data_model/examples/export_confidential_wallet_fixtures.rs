//! Helper example that prints deterministic wallet fixture outputs for the
//! confidential shield/transfer/unshield flows. Run via:
//!
//! `cargo run --example export_confidential_wallet_fixtures`
//!
//! The resulting hashes/bytes can be copied into
//! `fixtures/confidential/wallet_flows_v1.json`.

use std::{error::Error, io, str::FromStr, time::Duration};

use iroha_crypto::KeyPair;
use iroha_data_model::{
    confidential::ConfidentialEncryptedPayload,
    isi::zk,
    prelude::*,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    transaction::SignedTransaction,
};
use norito::json::{self, Map, Number, Value};

fn main() -> Result<(), Box<dyn Error>> {
    let context = fixture_context()?;
    let chain_id_label = context.chain_id.to_string();
    let authority_label = context.authority.to_string();
    let asset_id_label = context.asset_id.to_string();

    let shield_case = build_shield_case(
        context.chain_id.clone(),
        context.authority.clone(),
        context.asset_id.clone(),
        context.creation_time,
        context.ttl,
        context.keypair.private_key(),
    )?;
    let transfer_case = build_zk_transfer_case(
        context.chain_id.clone(),
        context.authority.clone(),
        context.asset_id.clone(),
        context.creation_time,
        context.ttl,
        context.keypair.private_key(),
    )?;

    let unshield_case = build_unshield_case(
        context.chain_id,
        context.authority,
        context.asset_id,
        context.creation_time,
        context.ttl,
        context.keypair.private_key(),
    )?;

    let cases = vec![
        shield_case.to_json(
            "shield-basic",
            "zk::Shield",
            "Public-to-shielded deposit with deterministic note/payload sample.",
        ),
        transfer_case.to_json(
            "zk-transfer-basic",
            "zk::ZkTransfer",
            "Shielded-to-shielded transfer with two inputs/outputs and audit metadata.",
        ),
        unshield_case.to_json(
            "unshield-basic",
            "zk::Unshield",
            "Shielded-to-public withdrawal with deterministic root hint.",
        ),
    ];

    let mut document = Map::new();
    document.insert("format_version".into(), Value::Number(Number::from(1_u64)));
    document.insert(
        "description".into(),
        Value::String("Deterministic confidential wallet transaction fixtures (shield, internal transfer, unshield). Each entry captures the canonical signed transaction and hash so SDKs can assert parity.".into()),
    );
    document.insert("chain_id".into(), Value::String(chain_id_label));
    document.insert("authority_id".into(), Value::String(authority_label));
    document.insert("asset_definition_id".into(), Value::String(asset_id_label));
    document.insert("cases".into(), Value::Array(cases));
    let document_value = Value::Object(document);

    let document_json = json::to_string_pretty(&document_value)?;
    println!("{document_json}");
    Ok(())
}

struct FixtureContext {
    chain_id: ChainId,
    authority: AccountId,
    asset_id: AssetDefinitionId,
    creation_time: Duration,
    ttl: Duration,
    keypair: KeyPair,
}

fn fixture_context() -> Result<FixtureContext, Box<dyn Error>> {
    let chain_id = ChainId::from_str("00000000-0000-0000-0000-000000000000")?;
    let asset_id = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    let signing_key: iroha_crypto::PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53".parse()?;
    let keypair = KeyPair::from_private_key(signing_key)?;
    let authority_seed = AccountId::new(keypair.public_key().clone());
    let authority_literal = authority_seed.canonical_i105()?;
    let authority = AccountId::parse_encoded(&authority_literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)?;

    Ok(FixtureContext {
        chain_id,
        authority,
        asset_id,
        creation_time: Duration::from_millis(1_700_000_000_000),
        ttl: Duration::from_millis(45),
        keypair,
    })
}

struct CaseOutput {
    #[cfg(test)]
    signed: SignedTransaction,
    signed_hex: String,
    hash_hex: String,
}

fn build_shield_case(
    chain: ChainId,
    authority: AccountId,
    asset: AssetDefinitionId,
    creation_time: Duration,
    ttl: Duration,
    private_key: &iroha_crypto::PrivateKey,
) -> Result<CaseOutput, Box<dyn Error>> {
    let note_commitment = [0xAB; 32];
    let payload =
        ConfidentialEncryptedPayload::new([0x01; 32], [0x02; 24], vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let instruction = InstructionBox::from(zk::Shield::new(
        asset,
        authority.clone(),
        42,
        note_commitment,
        payload,
    ));

    let mut builder = TransactionBuilder::new(chain, authority).with_instructions([instruction]);
    builder.set_creation_time(creation_time);
    builder.set_ttl(ttl);
    let signed = builder.try_sign(private_key)?;

    Ok(case_output(&signed))
}

fn build_zk_transfer_case(
    chain: ChainId,
    authority: AccountId,
    asset: AssetDefinitionId,
    creation_time: Duration,
    ttl: Duration,
    private_key: &iroha_crypto::PrivateKey,
) -> Result<CaseOutput, Box<dyn Error>> {
    let inputs = vec![[0x10; 32], [0x11; 32]];
    let outputs = vec![[0x22; 32], [0x33; 32]];
    let proof = proof_attachment("halo2/ipa", "vk_transfer")?;
    let instruction = InstructionBox::from(zk::ZkTransfer::new(
        asset,
        inputs,
        outputs,
        proof,
        Some([0x44; 32]),
    ));

    let mut builder = TransactionBuilder::new(chain, authority).with_instructions([instruction]);
    builder.set_creation_time(creation_time);
    builder.set_ttl(ttl);
    let signed = builder.try_sign(private_key)?;

    Ok(case_output(&signed))
}

fn build_unshield_case(
    chain: ChainId,
    authority: AccountId,
    asset: AssetDefinitionId,
    creation_time: Duration,
    ttl: Duration,
    private_key: &iroha_crypto::PrivateKey,
) -> Result<CaseOutput, Box<dyn Error>> {
    let inputs = vec![[0x55; 32]];
    let proof = proof_attachment("halo2/ipa", "vk_unshield")?;
    let instruction = InstructionBox::from(zk::Unshield::new(
        asset,
        authority.clone(),
        1337,
        inputs,
        proof,
        None,
    ));

    let mut builder = TransactionBuilder::new(chain, authority).with_instructions([instruction]);
    builder.set_creation_time(creation_time);
    builder.set_ttl(ttl);
    let signed = builder.try_sign(private_key)?;

    Ok(case_output(&signed))
}

fn proof_attachment(backend: &str, vk_name: &str) -> Result<ProofAttachment, Box<dyn Error>> {
    if backend.is_empty() || backend.chars().any(char::is_whitespace) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "proof backend identifier must be non-empty and contain no whitespace",
        )
        .into());
    }
    let ident = iroha_schema::Ident::from_str(backend)?;
    let proof = ProofBox::new(ident.clone(), vec![0xEE; 48]);
    let vk_ref = VerifyingKeyId::new(ident.clone(), vk_name.to_string());
    Ok(ProofAttachment::new_ref(ident, proof, vk_ref))
}

fn case_output(tx: &SignedTransaction) -> CaseOutput {
    let bytes = norito::codec::Encode::encode(tx);
    let hash = tx.hash();
    CaseOutput {
        #[cfg(test)]
        signed: tx.clone(),
        signed_hex: hex::encode(bytes),
        hash_hex: hex::encode(hash.as_ref()),
    }
}

impl CaseOutput {
    fn to_json(&self, case_id: &str, flow: &str, note: &str) -> Value {
        let mut map = Map::new();
        map.insert("case_id".into(), Value::String(case_id.to_string()));
        map.insert("flow".into(), Value::String(flow.to_string()));
        map.insert("note".into(), Value::String(note.to_string()));
        map.insert(
            "signed_transaction_hex".into(),
            Value::String(self.signed_hex.clone()),
        );
        map.insert(
            "transaction_hash_hex".into(),
            Value::String(self.hash_hex.clone()),
        );
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidential_wallet_cases_use_checked_signing_and_verify() {
        let context = fixture_context().expect("fixture context");
        let cases = [
            build_shield_case(
                context.chain_id.clone(),
                context.authority.clone(),
                context.asset_id.clone(),
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("shield case should sign"),
            build_zk_transfer_case(
                context.chain_id.clone(),
                context.authority.clone(),
                context.asset_id.clone(),
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("zk transfer case should sign"),
            build_unshield_case(
                context.chain_id,
                context.authority,
                context.asset_id,
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("unshield case should sign"),
        ];

        for case in &cases {
            case.signed
                .verify_signature()
                .expect("checked confidential wallet fixture signature should verify");
            assert_eq!(case.hash_hex, hex::encode(case.signed.hash().as_ref()));
        }
    }

    #[test]
    fn confidential_wallet_cases_reject_tampered_signature_bytes() {
        let context = fixture_context().expect("fixture context");
        let cases = [
            build_shield_case(
                context.chain_id.clone(),
                context.authority.clone(),
                context.asset_id.clone(),
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("shield case should sign"),
            build_zk_transfer_case(
                context.chain_id.clone(),
                context.authority.clone(),
                context.asset_id.clone(),
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("zk transfer case should sign"),
            build_unshield_case(
                context.chain_id,
                context.authority,
                context.asset_id,
                context.creation_time,
                context.ttl,
                context.keypair.private_key(),
            )
            .expect("unshield case should sign"),
        ];

        for case in &cases {
            let mut tampered = case.signed.clone();
            tampered.set_signature(iroha_data_model::transaction::signed::TransactionSignature(
                iroha_crypto::SignatureOf::from_signature(iroha_crypto::Signature::from_bytes(
                    &[0_u8; 64],
                )),
            ));
            assert!(
                tampered.verify_signature().is_err(),
                "tampered confidential wallet fixture signature must fail"
            );
        }
    }

    #[test]
    fn proof_attachment_rejects_invalid_backend_identifier() {
        let err = proof_attachment("halo2 ipa", "vk_transfer")
            .expect_err("whitespace-bearing backend identifier must be rejected");
        let message = err.to_string().to_ascii_lowercase();
        assert!(message.contains("whitespace"), "unexpected error: {err}");
    }
}
