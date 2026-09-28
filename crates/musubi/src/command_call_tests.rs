//! Public call grammar and caller-owned artifact/schema binding.
use super::*;
fn fixture() -> (Vec<u8>, ContractAddress) {
    let artifact = ivm::kotodama::compiler::Compiler::new().compile_source(
        "seiyaku Example { kotoage fn write(int value) authorize(\"CanInvokeContractEntrypoint\") {} kotoage fn ping() {} view fn read() -> int { return 1; } }",
    ).expect("compile current artifact");
    let key = iroha::crypto::KeyPair::random();
    let authority = iroha_data_model::account::AccountId::new(key.public_key().clone());
    let network = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        .parse()
        .unwrap();
    let address = ContractAddress::derive(
        &network,
        &authority,
        1,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    (artifact, address)
}
#[test]
fn mutable_call_encodes_local_schema_and_omits_zero_argument_payload() {
    let (artifact, address) = fixture();
    let value = norito::json!({"value": "7"});
    let (intent, payload) =
        trusted_call_intent(&artifact, address.clone(), "write", value.clone()).unwrap();
    assert_eq!(payload, Some(value.clone()));
    assert_eq!(intent.invocation.contract_address, address);
    assert_eq!(
        intent.invocation.expected_code_hash,
        ivm::verify_contract_artifact(&artifact).unwrap().code_hash
    );
    assert!(intent.invocation.arguments.is_some());
    assert_eq!(
        intent
            .metadata
            .get(&"contract_payload".parse::<Name>().unwrap())
            .unwrap(),
        &Json::from_norito_value_ref(&value).unwrap()
    );
    let (_, other) =
        trusted_call_intent(&artifact, address.clone(), "ping", norito::json!({})).unwrap();
    assert!(other.is_none());
    assert!(
        trusted_call_intent(
            &artifact,
            address.clone(),
            "write",
            norito::json!({"wrong": "7"})
        )
        .is_err()
    );
    assert!(trusted_call_intent(&artifact, address.clone(), "ping", value).is_err());
    assert!(
        trusted_call_intent(&artifact, address.clone(), "read", norito::json!({})).is_err()
    );
    let mut changed = artifact;
    changed[0] ^= 1;
    assert!(
        trusted_call_intent(
            &changed,
            address,
            "write",
            norito::json!({"value": "7"})
        )
        .is_err()
    );
}
#[test]
fn mutable_call_cli_requires_new_intent_or_exact_resume() {
    assert!(
        Cli::try_parse_from([
            "musubi",
            "call",
            "--entrypoint",
            "write",
            "--args",
            "{\"value\":\"7\"}"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["musubi", "call", "--resume", "/private/call"]).is_ok());
    assert!(Cli::try_parse_from(["musubi", "call", "--cancel", "/private/call"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "musubi",
            "call",
            "--resume",
            "/private/call",
            "--cancel",
            "/private/call"
        ])
        .is_err()
    );
    assert!(Cli::try_parse_from(["musubi", "call"]).is_err());
    assert!(
        Cli::try_parse_from([
            "musubi",
            "call",
            "--resume",
            "/private/call",
            "--entrypoint",
            "other"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "musubi",
            "call",
            "--entrypoint",
            "write",
            "--gas-limit",
            "0"
        ])
        .is_err()
    );
}
