//! Captured identifier frames, enum tags and canonical reconstruction contracts.

use crate::frame_identity_test_support::record;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{account, asset, domain, id::IdBox, nexus, nft, peer, permission, rwa};
use iroha_primitives::json::Json;
use norito::codec::Encode as _;
use norito::json::Value;

fn variants() -> Vec<IdBox> {
    let domain = domain::DomainId::try_new("vault", "sora").expect("qualified fixture domain");
    let key =
        KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519).expect("public fixture seed");
    let account = account::AccountId::new(key.public_key().clone());
    let definition =
        asset::AssetDefinitionId::derive_from_components(domain.clone(), "usd".parse().unwrap());
    vec![
        IdBox::DomainId(domain.clone()),
        IdBox::AccountId(account.clone()),
        IdBox::AssetDefinitionId(definition.clone()),
        IdBox::AssetId(asset::AssetId::new(definition, account)),
        IdBox::NftId(nft::NftId::new(domain.clone(), "title".parse().unwrap())),
        IdBox::RwaId(rwa::RwaId::generated(
            domain,
            Hash::new(b"public-id-box-rwa-fixture"),
        )),
        IdBox::PeerId(peer::PeerId::new(key.public_key().clone())),
        IdBox::LaneId(nexus::LaneId::new(7)),
        IdBox::TriggerId("settlement".parse().unwrap()),
        IdBox::RoleId("auditor".parse().unwrap()),
        IdBox::Permission(permission::Permission::new(
            "CanAudit".into(),
            Json::new(norito::json!({"scope": "ledger", "enabled": true})),
        )),
        IdBox::CustomParameterId("window".parse().unwrap()),
        IdBox::RepoAgreementId("daily".parse().unwrap()),
    ]
}

fn records() -> Vec<Value> {
    let values = variants();
    assert_eq!(values.len(), 13);
    let names = [
        "domain",
        "account",
        "asset_definition",
        "asset",
        "nft",
        "rwa",
        "peer",
        "lane",
        "trigger",
        "role",
        "permission",
        "custom_parameter",
        "repo_agreement",
    ];
    let mut rows = Vec::new();
    for (tag, (name, value)) in names.into_iter().zip(&values).enumerate() {
        // These are the protocol's explicit u32 enum tags, independent of the carrier's derive.
        assert_eq!(
            &value.encode()[..4],
            &u32::try_from(tag).unwrap().to_le_bytes(),
            "{name}: tag"
        );
        record(&mut rows, name, value);
    }
    record(&mut rows, "option_none", &None::<IdBox>);
    record(&mut rows, "option_permission", &Some(values[10].clone()));
    record(&mut rows, "vec_empty", &Vec::<IdBox>::new());
    record(&mut rows, "vec_all_variants", &values);
    assert_eq!(rows.len(), 17);
    rows
}

#[test]
fn all_identifier_variants_preserve_captured_frames() {
    let expected: Value = norito::json::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/id_box_identity_frames.json"
    )))
    .expect("immutable pre-declaration IdBox capture");
    assert_eq!(
        expected.get("rows").and_then(Value::as_array).unwrap(),
        &records()
    );
}
