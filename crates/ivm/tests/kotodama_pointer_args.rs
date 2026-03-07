use std::{collections::HashMap, str::FromStr};

use iroha_crypto::Hash as IrohaHash;
use iroha_data_model::prelude::*;
use ivm::{
    IVM, PointerType,
    kotodama::compiler::Compiler,
    mock_wsv::{MockWorldStateView, ScopedAccountId, WsvHost},
    validate_tlv_bytes,
};

fn parse_account_literal(literal: &str) -> AccountId {
    AccountId::parse_encoded(literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("account literal must be canonical IH58 or sora compressed")
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
    const ACCOUNT_A: &str = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    let src = r#"
        seiyaku PointerFFI {
            state Owners: Map<int, AccountId>;
            fn hajimari() {
                let default_owner = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
                let stored = get_or_insert_default(Owners, 7, default_owner);
                assert(stored == default_owner);
                let alt = account_id("6cmzPVPX8dTmJWnCc8X5MpcZLb7UjrvR5Y1VdRmfj9pbb93hFbJfpLb");
                let again = get_or_insert_default(Owners, 7, alt);
                assert(again == default_owner);
            }
        }
    "#;

    let bytecode = Compiler::new()
        .compile_source(src)
        .expect("compile pointer map contract");

    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&bytecode).expect("load program");
    let wsv = MockWorldStateView::new();
    let authority: ScopedAccountId = parse_account_literal(ACCOUNT_A);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&authority),
        HashMap::new(),
    );
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
    let expected: AccountId = parse_account_literal(ACCOUNT_A);
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
