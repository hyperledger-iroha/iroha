use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::{AccessSetHints, ContractManifest};

#[test]
fn contract_manifest_roundtrip_norito() {
    let manifest = ContractManifest {
        code_hash: Some(Hash::new(b"code-hash")),
        abi_hash: Some(Hash::new(b"abi-hash")),
        compiler_fingerprint: Some("kotodama-0.1.0".to_string()),
        features_bitmap: Some(0b1010_0001),
        access_set_hints: Some(AccessSetHints {
            read_keys: vec![
                "account:sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
                    .to_string(),
            ],
            write_keys: vec!["asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM".to_string()],
        }),
        entrypoints: None,
        kotoba: None,
        provenance: None,
    };

    let bytes = norito::to_bytes(&manifest).expect("encode manifest");
    let decoded: ContractManifest = norito::decode_from_bytes(&bytes).expect("decode manifest");

    assert_eq!(decoded, manifest);
}
