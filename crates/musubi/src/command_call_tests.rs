//! Public call grammar and caller-owned artifact/schema binding.
use super::*;
fn fixture() -> (Vec<u8>, ContractAddress) {
    let artifact = ivm::kotodama::compiler::Compiler::new().compile_source(
        "seiyaku Example { kotoage fn write(int value) authorize(\"CanInvokeContractEntrypoint\") {} kotoage fn ping() authorize(\"CanInvokeContractEntrypoint\") {} view fn read() -> int { return 1; } }",
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
    assert!(trusted_call_intent(&artifact, address.clone(), "read", norito::json!({})).is_err());
    let mut changed = artifact;
    changed[0] ^= 1;
    assert!(
        trusted_call_intent(&changed, address, "write", norito::json!({"value": "7"})).is_err()
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

fn continuation_network(config: Option<PathBuf>) -> network::SelectedNetwork {
    network::SelectedNetwork {
        name: "other-taira".to_owned(),
        config,
        config_image: None,
        chain_discriminant: 369,
        network_id: None,
        fee_payment: None,
        contracts: BTreeMap::new(),
    }
}

#[test]
fn mutable_call_continuation_retains_quoted_project_network_and_selected_client() {
    let manifest = Path::new("/projects/coffee club/Musubi.toml");
    let journal = Path::new("/projects/coffee club/target/call/exact-journal");
    let network = continuation_network(Some(PathBuf::from("/runtime/owner's wallet/client.toml")));
    assert_eq!(
        deploy::contract_resume_command("call", manifest, &network, journal),
        r#"musubi --manifest-path '/projects/coffee club/Musubi.toml' call --network other-taira --config '/runtime/owner'"'"'s wallet/client.toml' --resume '/projects/coffee club/target/call/exact-journal'"#
    );
    let parsed = Cli::try_parse_from([
        "musubi",
        "--manifest-path",
        manifest.to_str().expect("manifest"),
        "call",
        "--network",
        &network.name,
        "--config",
        network
            .config
            .as_ref()
            .expect("client")
            .to_str()
            .expect("path"),
        "--resume",
        journal.to_str().expect("journal"),
    ])
    .expect("continuation uses exact call resume grammar");
    assert_eq!(parsed.manifest_path.as_deref(), Some(manifest));
    let Command::Call(args) = parsed.command else {
        panic!("call continuation");
    };
    assert_eq!(args.network.as_deref(), Some("other-taira"));
    assert_eq!(args.config, network.config);
    assert_eq!(args.resume.as_deref(), Some(journal));
}

#[test]
fn mutable_call_continuation_without_client_override_keeps_network_selection() {
    let manifest = Path::new("/projects/coffee club/Musubi.toml");
    let journal = Path::new("/projects/coffee club/target/call/exact-journal");
    let network = continuation_network(None);
    assert_eq!(
        deploy::contract_resume_command("call", manifest, &network, journal),
        "musubi --manifest-path '/projects/coffee club/Musubi.toml' call --network other-taira --resume '/projects/coffee club/target/call/exact-journal'"
    );
    let parsed = Cli::try_parse_from([
        "musubi",
        "--manifest-path",
        manifest.to_str().expect("manifest"),
        "call",
        "--network",
        &network.name,
        "--resume",
        journal.to_str().expect("journal"),
    ])
    .expect("configured-network continuation remains valid");
    let Command::Call(args) = parsed.command else {
        panic!("call continuation");
    };
    assert_eq!(args.network.as_deref(), Some("other-taira"));
    assert!(args.config.is_none());
    assert_eq!(args.resume.as_deref(), Some(journal));
}
