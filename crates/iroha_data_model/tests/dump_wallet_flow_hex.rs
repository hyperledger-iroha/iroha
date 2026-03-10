//! Temporary helper test to dump canonical wallet instruction encodings.

use iroha_crypto::KeyPair;
use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    confidential::ConfidentialEncryptedPayload,
    domain::DomainId,
    isi::zk::{Shield, Unshield, ZkTransfer},
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
};
use norito::codec::encode_adaptive;

fn ident(label: &str) -> iroha_schema::Ident {
    label.into()
}

fn make_proof(bytes: Vec<u8>, vk_name: &str) -> ProofAttachment {
    let proof = ProofBox::new(ident("halo2/ipa"), bytes);
    ProofAttachment::new_ref(
        ident("halo2/ipa"),
        proof,
        VerifyingKeyId::new("halo2/ipa", vk_name),
    )
}

#[test]
fn dump_wallet_flow_hex() {
    let asset: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let domain: DomainId = "wonderland".parse().unwrap();
    let alice = AccountId::new(KeyPair::random().public_key().clone());
    let bob = AccountId::new(KeyPair::random().public_key().clone());

    let payload =
        ConfidentialEncryptedPayload::new([0x01; 32], [0x02; 24], vec![0xde, 0xad, 0xbe, 0xef]);
    let shield = Shield::new(asset.clone(), alice.clone(), 42, [0xab; 32], payload);
    let shield_hex = hex::encode(encode_adaptive(&shield));
    println!("shield hex: {shield_hex}");

    let transfer_inputs = vec![[0x10; 32], [0x20; 32]];
    let transfer_outputs = vec![[0x30; 32], [0x40; 32]];
    let transfer = ZkTransfer::new(
        asset.clone(),
        transfer_inputs,
        transfer_outputs,
        make_proof(vec![0x55; 48], "vk_transfer"),
        Some([0x50; 32]),
    );
    let transfer_hex = hex::encode(encode_adaptive(&transfer));
    println!("transfer hex: {transfer_hex}");

    let unshield_inputs = vec![[0x60; 32]];
    let unshield = Unshield::new(
        asset,
        bob,
        7,
        unshield_inputs,
        make_proof(vec![0xee; 48], "vk_unshield"),
        Some([0xaa; 32]),
    );
    let unshield_hex = hex::encode(encode_adaptive(&unshield));
    println!("unshield hex: {unshield_hex}");
}
