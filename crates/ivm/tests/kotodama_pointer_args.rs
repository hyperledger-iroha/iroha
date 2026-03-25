use std::{collections::HashMap, str::FromStr};

use iroha_crypto::Hash as IrohaHash;
use iroha_data_model::prelude::*;
use ivm::{
    IVM, PointerType,
    kotodama::compiler::Compiler,
    mock_wsv::{MockWorldStateView, WsvHost},
    validate_tlv_bytes,
};

fn account_from_public_key(public_key: &str) -> AccountId {
    AccountId::new(public_key.parse().expect("public key must be valid"))
}

fn resolve_state_value(host: &WsvHost, base: &Name, key: i64) -> Option<Vec<u8>> {
    let expected_path = format!("{}/{}", base.as_ref(), key);
    if let Some(bytes) = host.wsv.sc_get(&expected_path) {
        return Some(bytes.to_vec());
    }
    // Namespace sentinel (0x01 + seven zero bytes) used by durable map helpers.
    let namespaced_path = format!("{}\0\0\0\0\0\0\0{}", char::from(0x01), expected_path);
    if let Some(bytes) = host.wsv.sc_get(&namespaced_path) {
        return Some(bytes.to_vec());
    }
    None
}

#[test]
fn pointer_map_default_roundtrip() {
    const AUTHORITY_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    let src = r#"
        seiyaku PointerFFI {
            state Owners: Map<int, AccountId>;
            fn hajimari() {
                Owners[7] = authority();
            }
        }
    "#;

    let bytecode = Compiler::new()
        .compile_source(src)
        .expect("compile pointer map contract");

    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&bytecode).expect("load program");
    let wsv = MockWorldStateView::new();
    let authority = account_from_public_key(AUTHORITY_PUBLIC_KEY);
    let host = WsvHost::new_with_subject(wsv, authority, HashMap::new());
    vm.set_host(host);
    vm.run().expect("execute hajimari");

    let host_ref = vm.host_mut_any().expect("host access");
    let host = host_ref.downcast_ref::<WsvHost>().expect("wsv host");
    let base = Name::from_str("Owners").expect("valid state name");
    let stored = resolve_state_value(host, &base, 7).expect("state entry present");

    // Expect NoritoBytes TLV wrapping the AccountId pointer TLV.
    let outer = validate_tlv_bytes(&stored).expect("outer TLV");
    assert_eq!(outer.type_id, PointerType::NoritoBytes);
    let inner = validate_tlv_bytes(outer.payload).expect("inner TLV");
    assert_eq!(inner.type_id, PointerType::AccountId);

    let decoded_account: AccountId =
        norito::decode_from_bytes(inner.payload).expect("decode account id");
    let expected: AccountId = account_from_public_key(AUTHORITY_PUBLIC_KEY);
    assert_eq!(decoded_account, expected);

    // Ensure payload hash matches expected data (sanity check).
    let hash: [u8; 32] = IrohaHash::new(inner.payload).into();
    let hash_offset = 7 + inner.payload.len();
    assert!(
        outer.payload.len() >= hash_offset + hash.len(),
        "outer TLV payload must contain embedded hash",
    );
    let stored_hash = &outer.payload[hash_offset..hash_offset + hash.len()];
    assert_eq!(stored_hash, hash.as_ref());
}

#[test]
fn pointer_asset_state_storage_wraps_inner_pointer() {
    const ASSET_DEFINITION: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    let src = format!(
        r#"
        seiyaku PointerAssetStorage {{
            state Assets: Map<int, AssetDefinitionId>;

            fn main() {{
                Assets[7] = asset_definition("{asset_definition}");
            }}
        }}
    "#,
        asset_definition = ASSET_DEFINITION,
    );

    let bytecode = Compiler::new()
        .compile_source(&src)
        .expect("compile asset storage contract");
    let asset: AssetDefinitionId = ASSET_DEFINITION.parse().expect("asset definition literal");
    let authority = account_from_public_key(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let host = WsvHost::new_with_subject(MockWorldStateView::new(), authority, HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");
    vm.run().expect("store asset pointer");

    let host_ref = vm.host_mut_any().expect("host access");
    let host = host_ref.downcast_ref::<WsvHost>().expect("wsv host");
    let base = Name::from_str("Assets").expect("valid state name");
    let stored = resolve_state_value(host, &base, 7).expect("state entry present");

    let outer = validate_tlv_bytes(&stored).expect("outer TLV");
    assert_eq!(outer.type_id, PointerType::NoritoBytes);
    let inner = validate_tlv_bytes(outer.payload).expect("inner TLV");
    assert_eq!(inner.type_id, PointerType::AssetDefinitionId);

    let decoded_asset: AssetDefinitionId =
        norito::decode_from_bytes(inner.payload).expect("decode asset definition");
    assert_eq!(decoded_asset, asset);
}
