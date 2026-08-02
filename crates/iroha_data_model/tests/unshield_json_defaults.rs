//! Exact-shape JSON tests for confidential unshield instructions.

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
fn unshield_json_has_only_proof_bound_fields() {
    let account = AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    );
    let asset = AssetDefinitionId::derive_from_components(
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
        ("public_amount", json_value(&"1")),
        ("inputs", json_value(&vec![vec![0u64; 32]])),
        ("proof", json_value(&proof)),
        ("root_hint", norito::json::Value::Null),
    ]);

    let parsed: Unshield = norito::json::from_value(payload).expect("parse unshield json");
    assert_eq!(parsed.public_amount().to_string(), "1");
    let encoded = norito::json::to_value(&parsed).expect("encode unshield json");
    let fields = encoded.as_object().expect("unshield is a JSON object");
    assert_eq!(fields.len(), 6);
    assert!(!fields.contains_key("outputs"));

    let mut canonical_wire = norito::to_bytes(&parsed).expect("encode canonical unshield wire");
    canonical_wire.push(0);
    assert!(
        norito::decode_from_bytes::<Unshield>(&canonical_wire).is_err(),
        "the exact six-field wire rejects trailing retired fields"
    );
}

#[test]
fn unshield_json_rejects_caller_supplied_outputs() {
    let account = AccountId::new(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("public key"),
    );
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("asset name"),
    );
    let proof = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), vec![0xAA]),
        VerifyingKeyId::new("halo2/ipa", "unshield_vk"),
    );
    let stale_payload = json_object([
        ("asset", json_value(&asset.to_string())),
        ("to", json_value(&account)),
        ("public_amount", json_value(&"1")),
        ("inputs", json_value(&vec![vec![0u64; 32]])),
        ("outputs", json_value(&vec![vec![1u64; 32]])),
        ("proof", json_value(&proof)),
        ("root_hint", norito::json::Value::Null),
    ]);

    let error = norito::json::from_value::<Unshield>(stale_payload)
        .expect_err("caller-supplied unshield outputs must be rejected");
    assert!(error.to_string().contains("unknown field `outputs`"));
}
