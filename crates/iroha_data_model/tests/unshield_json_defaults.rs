//! JSON compatibility tests for confidential unshield instructions.

use iroha_data_model::{
    account::AccountId,
    asset::AssetDefinitionId,
    isi::zk::Unshield,
    prelude::DomainId,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
};

fn json_value<T: norito::json::JsonSerialize + ?Sized>(value: &T) -> norito::json::Value {
    norito::json::to_value(value).expect("serialize json value")
}

fn json_object<const N: usize>(
    pairs: [(&'static str, norito::json::Value); N],
) -> norito::json::Value {
    norito::json::object(pairs).expect("serialize json object")
}

#[test]
fn unshield_json_defaults_missing_outputs_to_empty() {
    let account = AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    );
    let asset = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("asset name"),
    );
    let proof = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![0xAA]),
        VerifyingKeyId::new("halo2/ipa", "unshield_vk"),
    );
    let payload = json_object([
        ("asset", json_value(&asset.to_string())),
        ("to", json_value(&account)),
        ("public_amount", json_value(&1u64)),
        ("inputs", json_value(&vec![vec![0u64; 32]])),
        ("proof", json_value(&proof)),
        ("root_hint", norito::json::Value::Null),
    ]);

    let parsed: Unshield = norito::json::from_value(payload).expect("parse unshield json");
    assert!(parsed.outputs().is_empty());
    assert_eq!(*parsed.public_amount(), 1);
}
