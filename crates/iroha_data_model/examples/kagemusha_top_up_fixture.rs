//! Generate the structural SDK top-up fixture with native canonical identifiers.
//!
//! Run `cargo run -p iroha_data_model --example kagemusha_top_up_fixture` and save
//! stdout as `fixtures/offline/kagemusha_top_up_request_v1.nrt`. The repeated-byte
//! payer key is public test material. Dummy proof bytes grant no monetary authority.

use std::io::Write as _;

use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    account::AccountId,
    isi::kagemusha_v1::KagemushaTopUpRequestV1,
    kagemusha::{
        KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaMintAuthorizationV1,
        KagemushaPairedProofV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
    },
};

fn main() {
    std::io::stdout()
        .write_all(&canonical_fixture())
        .expect("write fixture");
}

fn canonical_fixture() -> Vec<u8> {
    let source: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_v1.json"
    )))
    .expect("shared peer fixture JSON");
    let bytes = |section: &str| {
        hex::decode(source[section]["norito_hex"].as_str().expect("fixture hex"))
            .expect("valid fixture hex")
    };
    let peer = KagemushaPaymentRequestV1::decode_canonical_exact(&bytes("payment_request"))
        .expect("native peer request");
    let payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(&bytes("payment"), &peer)
            .expect("native peer payment");
    let payer = KeyPair::from_private_key(
        PrivateKey::from_bytes(Algorithm::Ed25519, &[0x42; 32]).expect("public fixture seed"),
    )
    .expect("fixture payer");
    let request = KagemushaTopUpRequestV1 {
        version: 1,
        operation_id: [0x77; 32],
        issuance_commitment: [0; 32],
        credit_id: [0; 32],
        release_id: peer.release_id,
        suite_id: peer.hardware_credential.suite_id,
        vk_digest: [0x78; 32],
        network_id: peer.network_id,
        asset: peer.asset,
        asset_incarnation: peer.asset_incarnation,
        scale: peer.scale,
        amount: 40,
        liability_pool_id: peer.liability_pool_id,
        payer: AccountId::new(payer.public_key().clone()),
        recipient: peer.recipient,
        hardware_credential: peer.hardware_credential,
        recipient_credential_commitment: [0x7a; 32],
        credit_commitment: [0x7b; 32],
        recipient_one_time_key: peer.recipient_encryption_key,
        encrypted_credit: payment.encrypted_credit,
        artifact_manifest_digest: [0x79; 32],
        mint_authorization: None,
    }
    .seal_identifiers()
    .expect("canonical native issuance and credit IDs");
    let statement = request
        .mint_authorization_statement()
        .expect("mint statement");
    let semantic_digest = statement.canonical_digest().expect("statement digest");
    let request = request
        .attach_mint_authorization(KagemushaMintAuthorizationV1 {
            version: 1,
            statement,
            proof: KagemushaPairedProofV1 {
                version: 1,
                eq_protocol_digest: [0x80; 32],
                ep_protocol_digest: [0x81; 32],
                semantic_digest,
                guard_eq_credential_audit: [0x82; 32],
                guard_ep_credential_audit: [0x83; 32],
                eq_deferred_audit: [0x84; 32],
                ep_deferred_audit: [0x85; 32],
                eq_proof: vec![0x86; 128],
                ep_proof: vec![0x87; 128],
                eq_history: vec![0x88; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
                ep_history: vec![0x89; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            },
        })
        .expect("structurally valid native authorization");
    norito::encode_canonical(&request).expect("canonical fixture")
}

#[test]
fn shared_top_up_fixture_matches_native_sealed_request() {
    assert_eq!(
        canonical_fixture(),
        include_bytes!("../../../fixtures/offline/kagemusha_top_up_request_v1.nrt").as_slice()
    );
}
