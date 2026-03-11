//! Example: Build Open/Approve with permissions and sign-in proof, and print
//! deterministic hashes and the canonical Approve signature preimage.
//!
//! Run:
//!   cargo run -p `iroha_torii_shared` --example `permissions_preimage`

use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{account::AccountId, domain::DomainId};
use iroha_torii_shared::{connect as proto, connect_sdk as sdk};

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn main() {
    // Sample session parameters
    let sid = [0x11u8; 32];
    let app_pk = [0x22u8; 32];
    let wallet_pk = [0x33u8; 32];
    let _domain: DomainId = "wonderland".parse().expect("domain parses");
    let keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
    let account_id = AccountId::new(keypair.public_key().clone()).to_string();

    // Request permissions in Open (app → wallet)
    let req_perms = proto::PermissionsV1 {
        methods: vec!["SIGN_REQUEST_RAW".into(), "SIGN_REQUEST_TX".into()],
        events: vec!["DISPLAY_REQUEST".into()],
        resources: None,
    };

    // Wallet narrows/accepts permissions in Approve (wallet → app)
    let acc_perms = proto::PermissionsV1 {
        methods: vec!["SIGN_REQUEST_TX".into()],
        events: vec![],
        resources: None,
    };

    // Optional sign-in proof (akin to SIWE)
    let proof = proto::SignInProofV1 {
        domain: "example.org".into(),
        uri: "https://example.org".into(),
        statement: "Sign in to DemoApp".into(),
        issued_at: "2025-01-01T00:00:00Z".into(),
        nonce: "abc123".into(),
    };

    // Build Open control
    let _open = proto::ConnectControlV1::Open {
        app_pk,
        app_meta: Some(proto::AppMeta {
            name: "DemoApp".into(),
            url: Some("https://example.org".into()),
            icon_hash: None,
        }),
        constraints: proto::Constraints {
            chain_id: "testnet".into(),
        },
        permissions: Some(req_perms.clone()),
    };

    // Build Approve control
    let _approve = proto::ConnectControlV1::Approve {
        wallet_pk,
        account_id: account_id.clone(),
        permissions: Some(acc_perms.clone()),
        proof: Some(proof.clone()),
        sig_wallet: proto::WalletSignatureV1::new(
            Algorithm::Ed25519,
            Signature::from_bytes(&[0u8; 64]),
        ),
    };

    // Compute deterministic hashes and signature preimage
    let perms_hash = sdk::hash_permissions_v1(&acc_perms);
    let proof_hash = sdk::hash_signin_proof_v1(&proof);
    let preimage = sdk::build_approve_preimage(
        &sid,
        &app_pk,
        &wallet_pk,
        &account_id,
        Some(&acc_perms),
        Some(&proof),
    );

    println!(
        "Permissions (accepted): methods={:?} events={:?}",
        acc_perms.methods, acc_perms.events
    );
    println!("permissions_hash_b2_256 = {}", hex(&perms_hash));
    println!(
        "proof: domain={} uri={} statement=\"{}\" issued_at={} nonce={}",
        proof.domain, proof.uri, proof.statement, proof.issued_at, proof.nonce
    );
    println!("proof_hash_b2_256 = {}", hex(&proof_hash));
    println!("approve_preimage_len = {}", preimage.len());
    println!("approve_preimage_hex = {}", hex(&preimage));
}
