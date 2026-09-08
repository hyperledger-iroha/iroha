#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii contract manifest endpoints: bytecode deploy wraps ISIs and GET reads the derived on-chain manifest.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha::data_model::prelude::*;
use iroha::data_model::{
    block::{
        consensus::{SumeragiCommittedLaneBlock, committed_lane_block_status_counts_as_progress},
        consensus_v2::recommended_data_availability_layout,
    },
    isi::smart_contract_code::{
        AcceptContractOwnership, ActivateContractInstance, DeactivateContractInstance,
        OfferContractOwnership, SetContractParliamentDelegation,
    },
    parameter::system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    smart_contract::{ContractAddress, ContractLifecycleOwnerV1},
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    governance::CanEnactGovernance,
    smart_contract::CanRegisterSmartContractCode,
};
use iroha_test_network::NetworkBuilder;
use reqwest::StatusCode;
use std::time::{Duration, Instant};
use std::{num::NonZeroU64, str::FromStr as _};
fn minimal_contract_artifact() -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "TestContract".to_owned(),
        compiler_fingerprint: "integration-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: Some("()".to_owned()),
            return_schema: Some(iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Unit],
            }),
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_types: Vec::new(),
        states: Vec::new(),
    };
    let mut out = meta.encode();
    out.extend_from_slice(&interface.encode_section());
    out.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    out
}
// The 200-entry probe uses about 3.9M gas locally. Reserve headroom for
// instance-scoped state paths and bounded scan precharges on real validators.
const CONTRACT_STATE_PROBE_GAS_LIMIT: u64 = 5_000_000;

fn contract_state_probe_artifact() -> Vec<u8> {
    let src = r#"
seiyaku ContractStateProbe {
  error enum ProbeError {
    NotInitialized = 1;
    WrongValue = 2
  }

  struct Receipt { int recipient; int amount; () memo; }

  state int Initialized;
  state int StoredValue;
  state int probe_readback;
  state () Marker;
  state Result<(), ProbeError> Outcome;
  state StateMap<int, int> Entries;
  const int PAGE_SIZE = 32 * 2;

  fn paginated_sum() -> int {
    var Option<StateCursor<int>> cursor = Option::none;
    var sum = 0;
    for iteration in range(4) {
      let page = Entries.page(after: cursor, limit: PAGE_SIZE);
      for (key, value) in page.items { sum += value; }
      cursor = page.next;
    }
    return sum;
  }

  kotoage fn main() -> int authorize("CanEnactGovernance") {
    return 0;
  }

  fn initialize_impl() {
    Initialized = 1;
    StoredValue = 7;
    probe_readback = 0;
    Marker = ();
    Outcome = Result::ok(());
  }

  hajimari() {
    initialize_impl();
  }

  kotoage fn verify() authorize("CanEnactGovernance") {
    require(Initialized == 1, ProbeError::NotInitialized);
    require(Marker == (), ProbeError::WrongValue);
    match Outcome {
      Result::ok(value) => { require(value == (), ProbeError::WrongValue); },
      Result::err(_) => { require(false, ProbeError::WrongValue); },
    };
    var List<int, 1> values = [StoredValue];
    match values.try_push(99) {
      Result::ok(_) => { require(false, ProbeError::WrongValue); },
      Result::err(failure) => {
        require(failure == ListError::CapacityExceeded, ProbeError::WrongValue);
      },
    };
    values.set(index: 0, value: StoredValue);
    let Receipt { amount, recipient: payee, memo: _ } =
      Receipt { amount: StoredValue, recipient: 2, memo: () };
    require(amount == 7 && payee == 2 && values.len() == 1, ProbeError::WrongValue);
    let rounded = decimal::from_int(amount).mul_div_round(
      multiplier: 2.0, divisor: 3.0, scale: 2, mode: Rounding::floor);
    require(rounded == 4.66, ProbeError::WrongValue);
    for index in range(200) { Entries[index] = index; }
    require(paginated_sum() == 19900, ProbeError::WrongValue);
    probe_readback = StoredValue;
  }

  view fn readback() -> int {
    require(Marker == (), ProbeError::WrongValue);
    match Outcome {
      Result::ok(value) => { require(value == (), ProbeError::WrongValue); },
      Result::err(_) => { require(false, ProbeError::WrongValue); },
    };
    require(paginated_sum() == 19900, ProbeError::WrongValue);
    return probe_readback;
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract-state probe program")
}
fn contract_probe_call_intent(
    artifact: &[u8],
    contract_address: &ContractAddress,
    contract_alias: &iroha_data_model::smart_contract::ContractAlias,
    entrypoint: &str,
) -> Result<iroha::client::ContractCallDraftIntent> {
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify contract probe artifact: {error}"))?;
    let descriptor = verified
        .contract_interface
        .entrypoints
        .iter()
        .find(|descriptor| descriptor.name == entrypoint)
        .ok_or_else(|| eyre!("contract probe entrypoint `{entrypoint}` is missing"))?;
    if !descriptor.params.is_empty() || descriptor.argument_schema.is_some() {
        return Err(eyre!(
            "contract probe `{entrypoint}` must have no arguments"
        ));
    }
    let mut metadata = Metadata::default();
    for (key, value) in [
        ("contract_address", contract_address.to_string()),
        ("contract_code_hash", verified.code_hash.to_string()),
        ("contract_alias", contract_alias.to_string()),
        ("contract_entrypoint", entrypoint.to_owned()),
    ] {
        metadata.insert(
            Name::from_str(key)?,
            iroha_primitives::json::Json::new(value),
        );
    }
    Ok(iroha::client::ContractCallDraftIntent {
        invocation: iroha_data_model::transaction::executable::ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: verified.code_hash,
            entrypoint: entrypoint.to_owned(),
            arguments: None,
        },
        metadata,
    })
}

fn contract_probe_call_observation(
    entrypoint: &str,
    transaction_hash: &str,
    unix_time_ms: u128,
) -> Result<String> {
    if !matches!(entrypoint, "hajimari" | "verify")
        || transaction_hash.len() != 64
        || !transaction_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || unix_time_ms == 0
    {
        return Err(eyre!(
            "contract probe observation requires an exact public stage/hash/timestamp"
        ));
    }
    Ok(format!(
        "KOTODAMA_CONTRACT_CALL stage={entrypoint} hash={transaction_hash} unix_time_ms={unix_time_ms}"
    ))
}

fn contract_probe_genesis_registration(artifact: &[u8]) -> Result<Vec<InstructionBox>> {
    use iroha_data_model::isi::smart_contract_code::{
        RegisterSmartContractBytes, RegisterSmartContractCode,
    };
    let registrar_key = &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    let registrar = AccountId::new(registrar_key.public_key().clone());
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify genesis contract probe artifact: {error}"))?;
    let manifest = verified
        .manifest
        .try_signed(registrar_key)
        .map_err(|error| eyre!("sign genesis contract probe manifest: {error}"))?;
    let permission: Permission = CanRegisterSmartContractCode.into();
    Ok(vec![
        Grant::account_permission(permission, registrar).into(),
        RegisterSmartContractBytes {
            code_hash: verified.code_hash,
            code: artifact.to_vec(),
        }
        .into(),
        RegisterSmartContractCode { manifest }.into(),
    ])
}

fn contract_probe_alias_management_permission() -> CanManageAccountAlias {
    CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Alias(
            iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                "contract_state_probe@universal"
                    .parse()
                    .expect("canonical contract probe alias permission"),
                iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
            ),
        ),
    }
}

fn contract_deployment_instructions(
    commit: iroha_data_model::isi::smart_contract_code::CommitContractDeployment,
    hajimari_grantee: Option<&AccountId>,
) -> Vec<InstructionBox> {
    let contract_address = commit.contract_address.clone();
    let mut instructions = vec![InstructionBox::from(commit)];
    if let Some(grantee) = hajimari_grantee {
        // Deployment stages the hook; its caller still needs an exact invocation grant.
        // Keep that grant after Commit in the same ordinary ordered transaction.
        let permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract_address,
                entrypoint: "hajimari".to_owned(),
            }
            .into();
        instructions.push(Grant::account_permission(permission, grantee.clone()).into());
    }
    instructions
}

#[test]
fn contract_v1_deployment_grants_only_the_exact_hajimari_invocation() {
    use iroha_data_model::isi::smart_contract_code::CommitContractDeployment;
    use iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint;

    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"contract-probe-deployment-grant",
    )));
    let address = |nonce| {
        ContractAddress::derive(
            &network,
            &iroha_test_samples::ALICE_ID,
            nonce,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("derive exact fixture address")
    };
    let contract_address = address(0);
    let commit = CommitContractDeployment {
        expected_deploy_nonce: 0,
        contract_address: contract_address.clone(),
        code_hash: Hash::new(b"exact-probe-artifact"),
        contract_alias: iroha_data_model::smart_contract::ContractAlias::from_components(
            "contract_state_probe",
            None,
            "universal",
        )
        .expect("probe alias"),
        lease_expiry_ms: None,
        expected_previous_contract_address: None,
    };
    let exact: Permission = CanInvokeContractEntrypoint {
        contract: contract_address.clone(),
        entrypoint: "hajimari".to_owned(),
    }
    .into();
    let expected: Vec<InstructionBox> = vec![
        commit.clone().into(),
        Grant::account_permission(exact.clone(), iroha_test_samples::ALICE_ID.clone()).into(),
    ];
    let actual =
        contract_deployment_instructions(commit.clone(), Some(&iroha_test_samples::ALICE_ID));
    assert_eq!(
        actual, expected,
        "Commit must precede the exact Alice hook grant"
    );
    assert_eq!(
        contract_deployment_instructions(commit.clone(), None),
        vec![InstructionBox::from(commit)],
        "other deployment fixtures retain their original instructions",
    );
    for (permission, grantee) in [
        (
            CanInvokeContractEntrypoint {
                contract: address(1),
                entrypoint: "hajimari".to_owned(),
            }
            .into(),
            iroha_test_samples::ALICE_ID.clone(),
        ),
        (
            CanInvokeContractEntrypoint {
                contract: contract_address,
                entrypoint: "verify".to_owned(),
            }
            .into(),
            iroha_test_samples::ALICE_ID.clone(),
        ),
        (exact, iroha_test_samples::BOB_ID.clone()),
        (
            Permission::new(
                "CanInvokeContractEntrypoint".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            iroha_test_samples::ALICE_ID.clone(),
        ),
    ] {
        let wrong: InstructionBox = Grant::account_permission(permission, grantee).into();
        assert_ne!(
            actual[1], wrong,
            "a different identity or unscoped token is not the hook grant"
        );
    }
    let mut reversed = expected;
    reversed.reverse();
    assert_ne!(
        actual, reversed,
        "grant order is part of the fixture contract"
    );
}

#[test]
fn contract_v1_alias_permission_is_exact() {
    let permission = contract_probe_alias_management_permission();
    let AccountAliasPermissionScope::Alias(alias) = permission.scope else {
        panic!("contract probe must not receive a domain- or dataspace-wide alias grant");
    };
    assert_eq!(alias.canonical_text(), "contract_state_probe@universal");
    assert_eq!(
        alias.dataspace_id,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL
    );
}

#[test]
fn contract_v1_genesis_registration_preserves_artifact_and_registrar() {
    use iroha_data_model::isi::smart_contract_code::{
        RegisterSmartContractBytes, RegisterSmartContractCode,
    };
    let artifact = contract_state_probe_artifact();
    let verified = ivm::verify_contract_artifact(&artifact).expect("verify probe artifact");
    let registrar_key = &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    let registrar = AccountId::new(registrar_key.public_key().clone());
    let permission: Permission = CanRegisterSmartContractCode.into();
    let expected: Vec<InstructionBox> = vec![
        Grant::account_permission(permission, registrar).into(),
        RegisterSmartContractBytes {
            code_hash: verified.code_hash,
            code: artifact.clone(),
        }
        .into(),
        RegisterSmartContractCode {
            manifest: verified
                .manifest
                .try_signed(registrar_key)
                .expect("sign manifest"),
        }
        .into(),
    ];
    let actual = contract_probe_genesis_registration(&artifact).expect("genesis registration");
    assert_eq!(
        actual, expected,
        "permission must precede exact bytes and signed manifest"
    );
    let registered = actual[2]
        .as_any()
        .downcast_ref::<RegisterSmartContractCode>()
        .expect("signed manifest instruction");
    let signed = registered.manifest();
    let provenance = signed.provenance.as_ref().expect("registrar provenance");
    provenance
        .signature
        .verify(
            registrar_key.public_key(),
            &signed.signature_payload_bytes(),
        )
        .expect("canonical manifest signature must verify");
    assert_eq!(signed.code_hash, Some(verified.code_hash));
    assert_eq!(signed.abi_hash, Some(verified.abi_hash));
    assert!(contract_probe_genesis_registration(b"not a contract artifact").is_err());
}

#[test]
fn contract_v1_four_validator_probe_compiles_final_syntax() {
    let artifact = contract_state_probe_artifact();
    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse V1 network probe");
    let interface = parsed.contract_interface.expect("signed V1 interface");
    assert!(interface.states.iter().any(|state| state.name == "Entries"));
    assert!(
        interface
            .error_types
            .iter()
            .any(|ty| ty.identity == "kotodama::ListError")
    );
    let address = ContractAddress::derive(
        &NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"contract-probe-test-network",
        ))),
        &iroha_test_samples::ALICE_ID,
        0,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .expect("derive probe address");
    let alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "contract_state_probe",
        None,
        "universal",
    )
    .expect("probe alias");
    for entrypoint in ["hajimari", "verify"] {
        let intent = contract_probe_call_intent(&artifact, &address, &alias, entrypoint)
            .expect("bind exact zero-argument probe call");
        assert_eq!(intent.invocation.contract_address, address);
        assert_eq!(intent.invocation.entrypoint, entrypoint);
        assert!(intent.invocation.arguments.is_none());
        assert_eq!(intent.metadata.iter().count(), 4);
        assert_eq!(
            intent
                .metadata
                .get(&Name::from_str("contract_alias").unwrap()),
            Some(&iroha_primitives::json::Json::new(alias.to_string())),
        );
        assert!(
            intent
                .metadata
                .get(&Name::from_str("contract_payload").unwrap())
                .is_none()
        );
    }
    assert!(contract_probe_call_intent(&artifact, &address, &alias, "missing").is_err());
    let hash = "ab".repeat(32);
    for entrypoint in ["hajimari", "verify"] {
        assert_eq!(
            contract_probe_call_observation(entrypoint, &hash, 123).unwrap(),
            format!("KOTODAMA_CONTRACT_CALL stage={entrypoint} hash={hash} unix_time_ms=123"),
        );
    }
    assert!(contract_probe_call_observation("other", &hash, 123).is_err());
    assert!(contract_probe_call_observation("verify", "abcd", 123).is_err());
    assert!(contract_probe_call_observation("verify", &"AB".repeat(32), 123).is_err());
    assert!(contract_probe_call_observation("verify", &hash, 0).is_err());
}
fn dynamic_access_counter_artifact() -> Vec<u8> {
    let src = r#"
seiyaku DynamicAccessCounter {
  state StateMap<int, int> Counters;

  fn bump_hidden(int key, int delta) {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_direct(int key, int delta) authorize("CanEnactGovernance") {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_via_helper(int key, int delta) authorize("CanEnactGovernance") {
    bump_hidden(key: key, delta: delta);
  }
}
"#;
    let artifact = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile dynamic-access counter program");
    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse dynamic-access metadata");
    let interface = parsed
        .contract_interface
        .expect("dynamic-access contract interface");
    for entrypoint_name in ["bump_direct", "bump_via_helper"] {
        let entrypoint = interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == entrypoint_name)
            .unwrap_or_else(|| panic!("missing `{entrypoint_name}` entrypoint"));
        assert!(
            entrypoint.write_keys.iter().any(|key| key == "state:*"),
            "`{entrypoint_name}` must transitively report the dynamic StateMap write: {entrypoint:?}"
        );
        assert_eq!(entrypoint.access_hints_complete, Some(false));
        assert!(
            entrypoint
                .access_hints_skipped
                .iter()
                .any(|reason| reason == "dynamic state path is not compiler-resolved")
        );
    }
    artifact
}
fn typed_core_query_pager_artifact() -> Vec<u8> {
    let source = r#"
seiyaku TypedCoreQueryPager {
  view fn accounts(int offset, int limit) -> QueryPage<AccountView> {
    ledger::query::accounts(offset: offset, limit: limit)
  }

  view fn assets(int offset, int limit) -> QueryPage<AssetView> {
    ledger::query::assets(offset: offset, limit: limit)
  }

  view fn asset_definitions(int offset, int limit) -> QueryPage<AssetDefinitionView> {
    ledger::query::asset_definitions(offset: offset, limit: limit)
  }

  view fn domains(int offset, int limit) -> QueryPage<DomainView> {
    ledger::query::domains(offset: offset, limit: limit)
  }

  view fn nfts(int offset, int limit) -> QueryPage<NftView> {
    ledger::query::nfts(offset: offset, limit: limit)
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile typed core-query pager program")
}
fn typed_core_query_page_payload_literals(offset: &str, limit: &str) -> norito::json::Value {
    norito::json::object([
        ("offset", norito::json::Value::from(offset.to_owned())),
        ("limit", norito::json::Value::from(limit.to_owned())),
    ])
    .expect("serialize typed core-query page arguments")
}
fn typed_core_query_page_payload(offset: i64, limit: i64) -> norito::json::Value {
    typed_core_query_page_payload_literals(&offset.to_string(), &limit.to_string())
}
async fn post_typed_core_query_page(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
    payload: norito::json::Value,
) -> Result<(StatusCode, norito::json::Value)> {
    let request = norito::json::object([
        (
            "authority",
            norito::json::Value::from(iroha_test_samples::ALICE_ID.to_string()),
        ),
        (
            "contract_address",
            norito::json::to_value(contract_address)?,
        ),
        (
            "entrypoint",
            norito::json::Value::from(entrypoint.to_owned()),
        ),
        ("payload", payload),
        ("gas_limit", norito::json::Value::from(1_000_000_u64)),
    ])?;
    let response = http
        .post(torii_url.join("v1/contracts/view")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_vec(&request)?)
        .send()
        .await?;
    let status = response.status();
    let body = response.bytes().await?;
    let body = norito::json::from_slice(&body)
        .map_err(|error| eyre!("contract view returned {status} with invalid JSON: {error}"))?;
    Ok((status, body))
}
async fn invoke_typed_core_query_page(
    client: iroha::client::Client,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
    offset: i64,
    limit: i64,
) -> Result<norito::json::Value> {
    let contract_address = contract_address.clone();
    let entrypoint = entrypoint.to_owned();
    let payload = typed_core_query_page_payload(offset, limit);
    let response = tokio::task::spawn_blocking(move || {
        client.post_contract_view_json(
            &iroha_test_samples::ALICE_ID,
            Some(&contract_address),
            None,
            &entrypoint,
            Some(&payload),
            1_000_000,
        )
    })
    .await??;
    response
        .get("result")
        .cloned()
        .ok_or_else(|| eyre!("contract view response is missing result: {response:?}"))
}
fn typed_query_page_parts(
    result: &norito::json::Value,
    view_name: &str,
) -> Result<(Vec<String>, Option<i64>)> {
    let result = result
        .as_object()
        .ok_or_else(|| eyre!("typed {view_name} page is not an object: {result:?}"))?;
    if result.len() != 2 || !result.contains_key("items") || !result.contains_key("next_offset") {
        return Err(eyre!(
            "typed {view_name} page must contain exactly items and next_offset: {result:?}"
        ));
    }
    let items = result
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("typed {view_name} page is missing its items list: {result:?}"))?;
    let ids = items
        .iter()
        .map(|item| {
            item.get("id")
                .and_then(norito::json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| eyre!("typed {view_name} is missing its id: {item:?}"))
        })
        .collect::<Result<Vec<_>>>()?;
    if ids.len() > ivm::core_query::QUERY_PAGE_CAPACITY_V1 {
        return Err(eyre!(
            "typed {view_name} page contains {} items; maximum is {}",
            ids.len(),
            ivm::core_query::QUERY_PAGE_CAPACITY_V1,
        ));
    }
    let next_offset = result
        .get("next_offset")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("typed {view_name} page is missing next_offset: {result:?}"))?;
    if next_offset.len() != 1 {
        return Err(eyre!(
            "typed {view_name} page contains a non-canonical next_offset: {result:?}"
        ));
    }
    let next_offset = if let Some(offset) = next_offset.get("some") {
        let raw = offset.as_str().ok_or_else(|| {
            eyre!("typed {view_name} page contains a non-canonical next_offset Int: {result:?}")
        })?;
        let parsed = raw.parse::<i64>().map_err(|error| {
            eyre!(
                "typed {view_name} page contains an out-of-range next_offset Int: \
                 {result:?}: {error}"
            )
        })?;
        if parsed.to_string() != raw {
            return Err(eyre!(
                "typed {view_name} page contains a non-canonical next_offset Int: {result:?}"
            ));
        }
        if parsed < 0 {
            return Err(eyre!(
                "typed {view_name} page contains a negative next_offset: {result:?}"
            ));
        }
        if ids.is_empty() {
            return Err(eyre!(
                "typed {view_name} page contains next_offset without making progress: {result:?}"
            ));
        }
        let next_offset_usize = usize::try_from(parsed).map_err(|_| {
            eyre!("typed {view_name} page contains an out-of-range next_offset: {result:?}")
        })?;
        if next_offset_usize < ids.len() {
            return Err(eyre!(
                "typed {view_name} page contains next_offset before its returned item count: \
                 {result:?}"
            ));
        }
        Some(parsed)
    } else if next_offset
        .get("none")
        .and_then(norito::json::Value::as_bool)
        == Some(true)
    {
        None
    } else {
        return Err(eyre!(
            "typed {view_name} page contains a non-canonical next_offset: {result:?}"
        ));
    };
    Ok((ids, next_offset))
}
#[test]
fn typed_query_page_parts_require_canonical_active_only_option_int() {
    for (source, expected) in [
        (
            r#"{"items":[{"id":"item"}],"next_offset":{"some":"3"}}"#,
            Some(3),
        ),
        (r#"{"items":[],"next_offset":{"none":true}}"#, None),
    ] {
        let page = norito::json::from_str(source).expect("parse canonical typed query page");
        assert_eq!(
            typed_query_page_parts(&page, "TestView")
                .expect("accept canonical active-only Option<Int>")
                .1,
            expected
        );
    }
    for source in [
        r#"{"items":[],"next_offset":{"some":3}}"#,
        r#"{"items":[],"next_offset":{"some":"03"}}"#,
        r#"{"items":[],"next_offset":{"some":"+3"}}"#,
        r#"{"items":[],"next_offset":{"some":"-0"}}"#,
        r#"{"items":[],"next_offset":{"some":"9223372036854775808"}}"#,
        r#"{"items":[],"next_offset":{"none":false}}"#,
        r#"{"items":[],"next_offset":{"some":"3","none":true}}"#,
        r#"{"items":[],"next_offset":{"unknown":true}}"#,
        r#"{"items":[],"next_offset":{"none":true},"cursor":null}"#,
        r#"{"items":[{"id":"item"}],"next_offset":{"some":"-3"}}"#,
        r#"{"items":[],"next_offset":{"some":"3"}}"#,
        r#"{"items":[{"id":"a"},{"id":"b"}],"next_offset":{"some":"1"}}"#,
    ] {
        let page = norito::json::from_str(source).expect("parse malformed typed query page");
        assert!(
            typed_query_page_parts(&page, "TestView").is_err(),
            "accepted non-canonical active-only Option<Int>: {source}"
        );
    }
    let oversized_items = (0..=ivm::core_query::QUERY_PAGE_CAPACITY_V1)
        .map(|index| format!(r#"{{"id":"item{index}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let oversized = norito::json::from_str(&format!(
        r#"{{"items":[{oversized_items}],"next_offset":{{"none":true}}}}"#
    ))
    .expect("parse oversized typed query page");
    assert!(
        typed_query_page_parts(&oversized, "TestView").is_err(),
        "accepted a typed query page above the V1 capacity"
    );
}
fn assert_typed_query_projection(
    result: &norito::json::Value,
    view_name: &str,
    expected_fields: &[&str],
) -> Result<()> {
    let items = result
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("typed {view_name} page is missing its items list: {result:?}"))?;
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| eyre!("typed {view_name} is not an object: {item:?}"))?;
        if object.len() != expected_fields.len()
            || !expected_fields
                .iter()
                .all(|field| object.contains_key(*field))
        {
            return Err(eyre!(
                "{view_name} must return only fields {expected_fields:?}: {item:?}"
            ));
        }
    }
    Ok(())
}
fn assert_canonical_query_order<T>(ids: &[T], entity_name: &str)
where
    T: Clone + Ord + std::fmt::Debug,
{
    let mut sorted = ids.to_vec();
    sorted.sort();
    assert_eq!(
        ids,
        sorted.as_slice(),
        "the ledger {entity_name} query must expose canonical ID order"
    );
}
fn signed_consensus_handshake(
    network: &sandbox::SerializedNetwork,
) -> Result<ConsensusHandshakeMetadata> {
    let mut handshakes = network
        .genesis_isi()
        .iter()
        .flatten()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .filter_map(|set_parameter| match set_parameter.inner() {
            Parameter::Custom(custom)
                if custom.id() == &consensus_metadata::handshake_meta_id() =>
            {
                Some(custom)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if handshakes.len() != 1 {
        return Err(eyre!(
            "genesis must contain exactly one signed consensus handshake; found {}",
            handshakes.len()
        ));
    }
    let custom = handshakes
        .pop()
        .expect("length checked before reading consensus handshake");
    norito::json::from_str(custom.payload().get())
        .map_err(|error| eyre!("decode signed consensus handshake metadata: {error}"))
}
fn lane_payload_contains_applied_transaction(
    proposal_height: u64,
    accepted_transaction_hashes: &[Hash],
    applied_height: u64,
    transaction_hash: &Hash,
) -> bool {
    // Payload planning precedes or coincides with execution in a certified global merge.
    // Its proposal coordinate need not equal the transaction's authoritative Applied height.
    proposal_height != 0
        && proposal_height <= applied_height
        && accepted_transaction_hashes.contains(transaction_hash)
}

#[test]
fn contract_v1_rbc_transaction_uses_distinct_proposal_and_applied_heights() {
    let transaction_hash = Hash::new(b"contract probe transaction");
    let other_hash = Hash::new(b"different contract probe transaction");
    let accepted = [transaction_hash];
    assert!(lane_payload_contains_applied_transaction(
        3,
        &accepted,
        4,
        &transaction_hash
    ));
    assert!(lane_payload_contains_applied_transaction(
        3,
        &accepted,
        3,
        &transaction_hash
    ));
    assert!(!lane_payload_contains_applied_transaction(
        5,
        &accepted,
        4,
        &transaction_hash
    ));
    assert!(!lane_payload_contains_applied_transaction(
        0,
        &accepted,
        4,
        &transaction_hash
    ));
    assert!(!lane_payload_contains_applied_transaction(
        3,
        &accepted,
        4,
        &other_hash
    ));
    assert!(!lane_payload_contains_applied_transaction(
        3,
        &[],
        4,
        &transaction_hash
    ));
}

// These tests replay a finite canonical prefix through the authenticated Blocks
// API. Each connection, read and close stays within the current attempt deadline.
const CONTRACT_RBC_CANONICAL_HEIGHT_LIMIT: u64 = 64;
const CONTRACT_RBC_FAILURE_LIMIT: usize = 8;
const CONTRACT_RBC_QUERY_ATTEMPT_LIMIT: usize = 4;
const CONTRACT_RBC_REPLAY_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
enum CanonicalContractRbcBinding {
    Ordinary(iroha_data_model::block::consensus::LaneBlockProposalV1),
    Autonomous(iroha_data_model::block::consensus::LaneBlockProposalV1),
}

#[derive(Clone, Debug, Default)]
struct CanonicalContractRbcCache {
    bindings: Option<Vec<CanonicalContractRbcBinding>>,
    attempts: usize,
    errors: Vec<String>,
}

impl CanonicalContractRbcCache {
    fn can_query(&self) -> bool {
        self.bindings.is_none() && self.attempts < CONTRACT_RBC_QUERY_ATTEMPT_LIMIT
    }

    fn record_query(
        &mut self,
        result: std::result::Result<Vec<CanonicalContractRbcBinding>, String>,
    ) -> Result<()> {
        if !self.can_query() {
            return Err(eyre!(
                "canonical RBC query attempted after success or budget exhaustion"
            ));
        }
        self.attempts += 1;
        match result {
            Ok(bindings) => self.bindings = Some(bindings),
            Err(error) => self.errors.push(error.chars().take(512).collect()),
        }
        Ok(())
    }
}

fn canonical_ordinary_contract_binding(
    ownership: &iroha_data_model::block::consensus::SumeragiLanePayloadOwnership,
) -> Result<CanonicalContractRbcBinding> {
    use iroha_data_model::block::consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1};
    ownership
        .validate_replay_material()
        .map_err(|error| eyre!("canonical ordinary ownership: {error}"))?;
    let descriptor = LaneBlockDescriptorV1 {
        lane_id: ownership.lane_id,
        dataspace_id: ownership.dataspace_id,
        lane_incarnation: ownership.lane_incarnation,
        proposal_height: ownership.proposal_height,
        previous_lane_block_height: ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
        lane_block_height: ownership.lane_block_height,
        lane_block_view: ownership.lane_block_view,
        subject_hash: ownership.subject_hash,
        payload_ownership_hash: ownership.payload_ownership_hash,
        rbc_instance_hash: ownership.rbc_instance_hash,
        accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&ownership.lane_block_descriptor_validator_set),
        validator_set: ownership.lane_block_descriptor_validator_set.clone(),
        validator_count: ownership.lane_block_descriptor_validator_count,
        min_quorum: ownership.lane_block_descriptor_min_quorum,
        qc_mode_tag: ownership.qc_mode_tag.clone(),
        descriptor_hash: ownership
            .lane_block_descriptor_hash
            .ok_or_else(|| eyre!("ordinary descriptor missing"))?,
    };
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        // A recovery hint is excluded from the public consensus hash preimage.
        // The caller separately checks the ownership's exact canonical header.
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    iroha_core::lane_consensus::validate_lane_block_proposal(&proposal)
        .map_err(|error| eyre!("canonical ordinary proposal: {error}"))?;
    Ok(CanonicalContractRbcBinding::Ordinary(proposal))
}

fn canonical_contract_replay_hashes(
    descriptor: &iroha_data_model::block::consensus::LaneBlockDescriptorV1,
) -> Result<(Hash, Hash, Hash)> {
    use iroha_data_model::block::consensus::SumeragiLanePayloadOwnership;
    let subject = SumeragiLanePayloadOwnership::compute_replay_subject_hash(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        descriptor.lane_block_view,
        &descriptor.accepted_candidate_indices,
        &descriptor.accepted_transaction_hashes,
        &descriptor.qc_mode_tag,
    )
    .map_err(|error| eyre!("canonical autonomous subject preimage: {error}"))?;
    let ownership = SumeragiLanePayloadOwnership::compute_replay_payload_ownership_hash(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        descriptor.lane_block_view,
        subject,
        &descriptor.accepted_candidate_indices,
        &descriptor.accepted_transaction_hashes,
        &descriptor.qc_mode_tag,
    )
    .map_err(|error| eyre!("canonical autonomous ownership preimage: {error}"))?;
    let rbc = SumeragiLanePayloadOwnership::compute_replay_rbc_instance_hash(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        descriptor.lane_block_view,
        subject,
        ownership,
    )
    .map_err(|error| eyre!("canonical autonomous RBC preimage: {error}"))?;
    Ok((subject, ownership, rbc))
}

fn canonical_autonomous_contract_binding(
    envelope: &iroha_data_model::block::execution_context::AutonomousLanePayloadEnvelopeV1,
    block_height: u64,
    network_id: iroha_data_model::NetworkId,
) -> Result<CanonicalContractRbcBinding> {
    use iroha_core::lane_consensus::{LaneExecutablePayloadV1, validate_lane_block_proposal};
    use iroha_data_model::merge::MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES;
    if envelope.version != iroha_data_model::block::AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1
        || envelope.canonical_payload.is_empty()
        || envelope.canonical_payload.len() > MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES
    {
        return Err(eyre!("canonical autonomous envelope version/byte bound"));
    }
    let payload: LaneExecutablePayloadV1 = norito::decode_canonical(&envelope.canonical_payload)
        .map_err(|error| eyre!("canonical autonomous payload codec: {error}"))?;
    if norito::encode_canonical(&payload)? != envelope.canonical_payload {
        return Err(eyre!("canonical autonomous payload roundtrip"));
    }
    // The current Core constructor and validator use internal payload version1
    // (LANE_EXECUTABLE_PAYLOAD_VERSION_V1), independently of envelope versioning.
    if payload.version != 1 {
        return Err(eyre!("canonical autonomous payload version"));
    }
    let proposal = &payload.origin_proposal;
    let descriptor = &proposal.descriptor;
    validate_lane_block_proposal(proposal)
        .map_err(|error| eyre!("canonical autonomous proposal: {error}"))?;
    if canonical_contract_replay_hashes(descriptor)?
        != (
            descriptor.subject_hash,
            descriptor.payload_ownership_hash,
            descriptor.rbc_instance_hash,
        )
    {
        return Err(eyre!("canonical autonomous replay commitments"));
    }
    let hashes = payload
        .entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    // The finalized queried block supplies canonical provenance. This helper
    // joins that exact body to diagnostics; it does not mint payload custody or
    // replace consensus signature/lifecycle validation with a test-side policy.
    if proposal.payload_block_hint.is_some()
        || descriptor.lane_block_view != 0
        || descriptor.proposal_height != block_height
        || envelope.network_id != network_id
        || payload.network_id != network_id
        || envelope.epoch != payload.epoch
        || envelope.lane_id != descriptor.lane_id
        || envelope.dataspace_id != descriptor.dataspace_id
        || envelope.lane_incarnation != descriptor.lane_incarnation
        || envelope.proposal_height != descriptor.proposal_height
        || envelope.lane_block_height != descriptor.lane_block_height
        || envelope.lane_block_view != descriptor.lane_block_view
        || envelope.proposal_hash != proposal.proposal_hash
        || envelope.descriptor_hash != descriptor.descriptor_hash
        || envelope.payload_hash != payload.payload_hash
        || envelope.producer != payload.producer
        || !descriptor.validator_set.contains(&payload.producer)
        || payload.producer_signature.is_empty()
        || hashes != payload.entrypoint_hashes
        || hashes != descriptor.accepted_transaction_hashes
    {
        return Err(eyre!(
            "canonical autonomous envelope/proposal/transaction binding"
        ));
    }
    Ok(CanonicalContractRbcBinding::Autonomous(
        payload.origin_proposal,
    ))
}

fn contract_rbc_remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| eyre!("canonical RBC replay deadline expired"))
}

fn contract_rbc_replay_end(head: u64, required_height: Option<u64>) -> Result<u64> {
    if required_height.is_some_and(|height| {
        !(2..=CONTRACT_RBC_CANONICAL_HEIGHT_LIMIT).contains(&height) || height > head
    }) {
        return Err(eyre!(
            "canonical RBC replay head does not cover required Applied height"
        ));
    }
    let end = head.min(CONTRACT_RBC_CANONICAL_HEIGHT_LIMIT);
    if end < 2 {
        return Err(eyre!("canonical RBC replay has no non-genesis prefix"));
    }
    Ok(end)
}

// The same finite stream consumer is exercised with fake rows below. Canonical
// provenance in the network case belongs to the authenticated Blocks API.
async fn receive_contract_rbc_prefix<S, T, F>(
    stream: &mut S,
    end: u64,
    deadline: Instant,
    mut identity: F,
) -> Result<Vec<T>>
where
    S: futures_util::Stream<Item = Result<T>> + Unpin,
    F: FnMut(&T) -> (u64, HashOf<BlockHeader>, Option<HashOf<BlockHeader>>),
{
    use futures_util::TryStreamExt as _;

    if !(2..=CONTRACT_RBC_CANONICAL_HEIGHT_LIMIT).contains(&end) {
        return Err(eyre!("canonical RBC replay end exceeds the fixture bound"));
    }
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact((end - 1) as usize)
        .map_err(|_| eyre!("cannot reserve bounded canonical RBC prefix"))?;
    let mut previous_hash = None;
    for expected_height in 2..=end {
        contract_rbc_remaining(deadline)?;
        let block =
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), stream.try_next())
                .await
                .map_err(|_| eyre!("canonical RBC replay timed out at height {expected_height}"))??
                .ok_or_else(|| {
                    eyre!("canonical RBC replay ended before height {expected_height}")
                })?;
        // An async timer cannot interrupt the synchronous Norito decoder.
        contract_rbc_remaining(deadline)?;
        let (height, hash, parent) = identity(&block);
        contract_rbc_remaining(deadline)?;
        if height != expected_height || parent.is_none() || (height > 2 && parent != previous_hash)
        {
            return Err(eyre!(
                "canonical RBC replay is not an ascending contiguous hash chain"
            ));
        }
        previous_hash = Some(hash);
        blocks.push(block);
    }
    // Receiving exactly the frozen end avoids waiting for a future block or
    // performing lookahead beyond the admitted canonical prefix.
    Ok(blocks)
}

async fn stream_contract_rbc_prefix(
    client: &iroha::client::Client,
    end: u64,
    deadline: Instant,
) -> Result<Vec<SignedBlock>> {
    let remaining = contract_rbc_remaining(deadline)?;
    // Reserve a bounded portion of this same attempt for the close handshake.
    let close_reserve = Duration::from_secs(1).min(remaining / 4);
    let receive_deadline = deadline - close_reserve;
    let mut stream = tokio::time::timeout_at(
        tokio::time::Instant::from_std(receive_deadline),
        client.listen_for_blocks_async(NonZeroU64::new(2).expect("nonzero replay start")),
    )
    .await
    .map_err(|_| eyre!("canonical RBC replay connection timed out"))??;
    let result = receive_contract_rbc_prefix(&mut stream, end, receive_deadline, |block| {
        (
            block.header().height().get(),
            block.hash(),
            block.header().prev_block_hash(),
        )
    })
    .await;
    finish_contract_rbc_replay(result, stream.close(), deadline).await
}

async fn finish_contract_rbc_replay<T>(
    result: Result<Vec<T>>,
    close: impl std::future::Future<Output = ()>,
    deadline: Instant,
) -> Result<Vec<T>> {
    let close = tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), close).await;
    match (result, close) {
        (Ok(blocks), Ok(())) => {
            contract_rbc_remaining(deadline)?;
            Ok(blocks)
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(_)) => Err(eyre!("canonical RBC replay close timed out")),
        (Err(error), Err(_)) => Err(eyre!("{error:#}; canonical RBC replay close timed out")),
    }
}

fn replay_canonical_contract_rbc_bindings(
    client: &iroha::client::Client,
    deadline: Instant,
    required_height: Option<u64>,
) -> Result<Vec<CanonicalContractRbcBinding>> {
    let mut client = client.clone();
    client.torii_request_timeout = contract_rbc_remaining(deadline)?;
    let head = client.get_status()?.blocks;
    contract_rbc_remaining(deadline)?;
    let end = contract_rbc_replay_end(head, required_height)?;
    let blocks = tokio::runtime::Handle::current()
        .block_on(stream_contract_rbc_prefix(&client, end, deadline))?;
    let mut bindings = Vec::new();
    for block in blocks {
        contract_rbc_remaining(deadline)?;
        let height = block.header().height().get();
        let Some(context) = block.execution_context() else {
            continue;
        };
        if context.lane_payload_ownerships.len() > 64 || context.autonomous_lane_payloads.len() > 64
        {
            return Err(eyre!("canonical RBC ownership count exceeds fixture bound"));
        }
        for ownership in &context.lane_payload_ownerships {
            if ownership.proposal_height != height
                || ownership.proposal_view != block.header().view_change_index()
            {
                return Err(eyre!(
                    "ordinary ownership is bound to another canonical block"
                ));
            }
            bindings.push(canonical_ordinary_contract_binding(ownership)?);
            contract_rbc_remaining(deadline)?;
        }
        for envelope in &context.autonomous_lane_payloads {
            bindings.push(canonical_autonomous_contract_binding(
                envelope,
                height,
                client.network_id,
            )?);
            contract_rbc_remaining(deadline)?;
        }
    }
    contract_rbc_remaining(deadline)?;
    Ok(bindings)
}

fn contract_rbc_binding_matches(
    binding: &CanonicalContractRbcBinding,
    record: &SumeragiCommittedLaneBlock,
    expected_validator_set: &[PeerId],
    required_applied_transaction: Option<(u64, &Hash)>,
) -> bool {
    let (CanonicalContractRbcBinding::Ordinary(proposal)
    | CanonicalContractRbcBinding::Autonomous(proposal)) = binding;
    let descriptor = &proposal.descriptor;
    iroha_core::lane_consensus::validate_lane_block_proposal(proposal).is_ok()
        && proposal.proposal_hash == record.proposal_hash
        && descriptor.lane_id == record.lane_id
        && descriptor.dataspace_id == record.dataspace_id
        && descriptor.lane_incarnation == record.lane_incarnation
        && descriptor.lane_block_height == record.lane_block_height
        && descriptor.lane_block_view == record.lane_block_view
        && descriptor.descriptor_hash == record.descriptor_hash
        && descriptor.subject_hash == record.subject_hash
        && descriptor.payload_ownership_hash == record.payload_ownership_hash
        && descriptor.rbc_instance_hash == record.rbc_instance_hash
        && descriptor.qc_mode_tag == record.qc_mode_tag
        && descriptor.validator_count == record.validator_count
        && descriptor.min_quorum == record.min_quorum
        && descriptor.validator_set.as_slice() == expected_validator_set
        && required_applied_transaction.is_none_or(|(height, hash)| {
            lane_payload_contains_applied_transaction(
                descriptor.proposal_height,
                &descriptor.accepted_transaction_hashes,
                height,
                hash,
            )
        })
}

fn shared_contract_rbc_record(
    observations: &[Vec<SumeragiCommittedLaneBlock>],
) -> Option<SumeragiCommittedLaneBlock> {
    if observations.len() != 4 {
        return None;
    }
    observations[0]
        .iter()
        .filter(|record| observations[1..].iter().all(|peer| peer.contains(record)))
        .max_by_key(|record| (record.lane_block_height, record.lane_block_view))
        .cloned()
}

fn contract_rbc_progress_failures(
    record: &SumeragiCommittedLaneBlock,
    after: Option<&SumeragiCommittedLaneBlock>,
    validator_count: u32,
    min_quorum: u32,
) -> Vec<&'static str> {
    let mut failures = Vec::new();
    if !after.is_none_or(|baseline| {
        record.lane_id == baseline.lane_id
            && record.dataspace_id == baseline.dataspace_id
            && record.lane_incarnation == baseline.lane_incarnation
            && (record.lane_block_height, record.lane_block_view)
                > (baseline.lane_block_height, baseline.lane_block_view)
    }) {
        failures.push("not_later_in_same_lane_incarnation");
    }
    if !record.executable_payload_available {
        failures.push("payload_unavailable");
    }
    if !committed_lane_block_status_counts_as_progress(
        &record.execution_status,
        record.executable_payload_available,
    ) {
        failures.push("execution_status");
    }
    if record.validator_count != validator_count || record.min_quorum != min_quorum {
        failures.push("committee_geometry");
    }
    if record.prepare_qc_signer_count != min_quorum || record.commit_qc_signer_count != min_quorum {
        failures.push("exact_quorum_signers");
    }
    let zero = Hash::prehashed([0; Hash::LENGTH]);
    if [
        record.descriptor_hash,
        record.proposal_hash,
        record.subject_hash,
        record.payload_ownership_hash,
        record.rbc_instance_hash,
    ]
    .contains(&zero)
    {
        failures.push("zero_identity");
    }
    failures
}

async fn wait_for_cross_peer_rbc_diagnostics(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
    after: Option<&SumeragiCommittedLaneBlock>,
    required_applied_transaction: Option<(u64, &Hash)>,
) -> Result<SumeragiCommittedLaneBlock> {
    let expected_validator_count = u32::try_from(network.peers().len())
        .map_err(|_| eyre!("peer count does not fit in u32"))?;
    let expected_min_quorum = u32::try_from(commit_quorum_from_len(network.peers().len()).max(1))
        .map_err(|_| eyre!("commit quorum does not fit in u32"))?;
    if required_applied_transaction
        .is_some_and(|(height, _)| height > CONTRACT_RBC_CANONICAL_HEIGHT_LIMIT)
    {
        return Err(eyre!(
            "required transaction exceeds the bounded canonical RBC prefix"
        ));
    }
    let mut expected_validator_set = network
        .peers()
        .iter()
        .map(|peer| peer.id())
        .collect::<Vec<_>>();
    expected_validator_set.sort();
    let deadline = Instant::now() + timeout;
    // Freeze only a successfully validated prefix, after an eligible certificate
    // exists. Failed reads remain visible and may retry within a four-attempt cap.
    let mut canonical = vec![CanonicalContractRbcCache::default(); network.peers().len()];
    loop {
        let tasks = network
            .peers()
            .iter()
            .enumerate()
            .map(|(index, peer)| {
                let mut client = peer.client();
                let can_query = canonical[index].can_query();
                let baseline = after.cloned();
                let validators = expected_validator_set.clone();
                let required = required_applied_transaction.map(|(height, hash)| (height, *hash));
                tokio::task::spawn_blocking(move || -> Result<_> {
                    let attempt_deadline = deadline.min(Instant::now() + CONTRACT_RBC_REPLAY_ATTEMPT_TIMEOUT);
                    client.torii_request_timeout = contract_rbc_remaining(attempt_deadline)?;
                    let diagnostics = client.get_sumeragi_diagnostics()?;
                    contract_rbc_remaining(attempt_deadline)?;
                    let queried = (can_query
                        && diagnostics.npos.is_some()
                        && diagnostics.committed_lane_blocks.iter().any(|record| {
                            contract_rbc_progress_failures(
                                record,
                                baseline.as_ref(),
                                expected_validator_count,
                                expected_min_quorum,
                            )
                            .is_empty()
                        }))
                    .then(|| {
                        let bindings = replay_canonical_contract_rbc_bindings(
                            &client,
                            attempt_deadline,
                            required.as_ref().map(|(height, _)| *height),
                        )
                        .map_err(|error| format!("{error:#}"))?;
                        let requested = required.as_ref().map(|(height, hash)| (*height, hash));
                        let joined = diagnostics.committed_lane_blocks.iter().any(|record| {
                            contract_rbc_progress_failures(record, baseline.as_ref(), expected_validator_count, expected_min_quorum).is_empty()
                                && bindings.iter().any(|binding| contract_rbc_binding_matches(binding, record, &validators, requested))
                        });
                        contract_rbc_remaining(attempt_deadline).map_err(|error| format!("{error:#}"))?;
                        if !joined {
                            return Err("canonical prefix does not yet contain an eligible certified transaction binding".to_owned());
                        }
                        Ok(bindings)
                    });
                    Ok((diagnostics, queried))
                })
            })
            .collect::<Vec<_>>();
        let mut observations = Vec::with_capacity(tasks.len());
        let mut errors = Vec::new();
        let mut predicate_failures = Vec::new();
        for (index, task) in tasks.into_iter().enumerate() {
            match task.await {
                Ok(Ok((diagnostics, queried))) if diagnostics.npos.is_some() => {
                    if let Some(result) = queried {
                        canonical[index].record_query(result)?;
                    }
                    let mut matches = Vec::new();
                    let mut rejected = Vec::new();
                    for record in diagnostics.committed_lane_blocks {
                        let mut failures = contract_rbc_progress_failures(
                            &record,
                            after,
                            expected_validator_count,
                            expected_min_quorum,
                        );
                        let binding_matches =
                            canonical[index].bindings.as_ref().is_some_and(|bindings| {
                                bindings.iter().any(|binding| {
                                    contract_rbc_binding_matches(
                                        binding,
                                        &record,
                                        &expected_validator_set,
                                        required_applied_transaction,
                                    )
                                })
                            });
                        if !binding_matches {
                            failures.push("canonical_proposal_committee_or_transaction_join");
                        }
                        if failures.is_empty() {
                            matches.push(record);
                        } else if rejected.len() < CONTRACT_RBC_FAILURE_LIMIT {
                            rejected.push(format!(
                                "lane={}/height={}/view={}/proposal={}: {:?}",
                                record.lane_id.as_u32(),
                                record.lane_block_height,
                                record.lane_block_view,
                                record.proposal_hash,
                                failures
                            ));
                        }
                    }
                    let (ordinary, autonomous) =
                        canonical[index]
                            .bindings
                            .as_ref()
                            .map_or((0, 0), |bindings| {
                                bindings
                                    .iter()
                                    .fold((0, 0), |(o, a), binding| match binding {
                                        CanonicalContractRbcBinding::Ordinary(_) => (o + 1, a),
                                        CanonicalContractRbcBinding::Autonomous(_) => (o, a + 1),
                                    })
                            });
                    predicate_failures.push(format!(
                        "peer {index}: canonical ordinary={ordinary} autonomous={autonomous}; queries={}/{}; query_errors={:?}; rejected={rejected:?}",
                        canonical[index].attempts, CONTRACT_RBC_QUERY_ATTEMPT_LIMIT, canonical[index].errors,
                    ));
                    observations.push(matches);
                }
                Ok(Ok(_)) => {
                    errors.push(format!(
                        "peer {index} diagnostics did not expose NPoS state"
                    ));
                    observations.push(Vec::new());
                }
                Ok(Err(error)) => {
                    errors.push(
                        format!("peer {index}: {error:#}")
                            .chars()
                            .take(512)
                            .collect(),
                    );
                    observations.push(Vec::new());
                }
                Err(error) => {
                    errors.push(format!("peer {index} diagnostics task failed: {error}"));
                    observations.push(Vec::new());
                }
            }
        }
        if Instant::now() < deadline {
            if let Some(shared) = shared_contract_rbc_record(&observations) {
                return Ok(shared);
            }
        }
        let evidence_counts = observations.iter().map(Vec::len).collect::<Vec<_>>();
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for identical four-peer certified RBC diagnostics; eligible_counts={evidence_counts:?}; errors={errors:?}; predicates={predicate_failures:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

// A structural query/diagnostic join fixture. Quorum and producer-signature
// execution remain covered by the real four-validator scenarios.
fn contract_rbc_autonomous_join_fixture() -> (
    iroha_data_model::block::AutonomousLanePayloadEnvelopeV1,
    SumeragiCommittedLaneBlock,
    Vec<PeerId>,
    Hash,
) {
    use iroha_core::lane_consensus::LaneExecutablePayloadV1;
    use iroha_data_model::{
        block::consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        nexus::{DataSpaceId, LaneId},
    };
    let mut validators = (1..=4)
        .map(|seed| {
            PeerId::new(
                KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
    validators.sort();
    let network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::new(b"RBC join fixture genesis")),
    );
    let signed = TransactionBuilder::new(
        network_id,
        iroha_test_samples::ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "RBC canonical join".to_owned())])
    .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    let entrypoint = iroha_data_model::transaction::TransactionEntrypoint::External(signed);
    let transaction_hash = Hash::from(entrypoint.hash());
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::new(0),
        dataspace_id: DataSpaceId::new(0),
        lane_incarnation: Hash::new(b"RBC join fixture incarnation"),
        proposal_height: 3,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::new(b"subject"),
        payload_ownership_hash: Hash::new(b"ownership"),
        rbc_instance_hash: Hash::new(b"rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![transaction_hash],
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validators),
        validator_set: validators.clone(),
        validator_count: 4,
        min_quorum: 3,
        qc_mode_tag: "npos".to_owned(),
        descriptor_hash: Hash::prehashed([0; 32]),
    };
    (
        descriptor.subject_hash,
        descriptor.payload_ownership_hash,
        descriptor.rbc_instance_hash,
    ) = canonical_contract_replay_hashes(&descriptor).expect("fixture replay commitments");
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; 32]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    iroha_core::lane_consensus::validate_lane_block_proposal(&proposal)
        .expect("valid structural proposal");
    let d = &proposal.descriptor;
    let record = SumeragiCommittedLaneBlock {
        lane_id: d.lane_id,
        dataspace_id: d.dataspace_id,
        lane_incarnation: d.lane_incarnation,
        lane_block_height: d.lane_block_height,
        lane_block_view: d.lane_block_view,
        descriptor_hash: d.descriptor_hash,
        proposal_hash: proposal.proposal_hash,
        execution_status: "state_applied_by_canonical_block".to_owned(),
        executable_payload_available: true,
        subject_hash: d.subject_hash,
        payload_ownership_hash: d.payload_ownership_hash,
        rbc_instance_hash: d.rbc_instance_hash,
        qc_mode_tag: d.qc_mode_tag.clone(),
        validator_count: 4,
        min_quorum: 3,
        prepare_qc_signer_count: 3,
        commit_qc_signer_count: 3,
    };
    let payload = LaneExecutablePayloadV1 {
        version: 1,
        network_id,
        epoch: 0,
        origin_proposal: proposal.clone(),
        entrypoint_hashes: vec![transaction_hash],
        entrypoints: vec![entrypoint],
        reservation_keys: Vec::new(),
        routing_plans: Vec::new(),
        native_amx_receipts: Vec::new(),
        payload_hash: Hash::new(b"structural fixture payload commitment"),
        producer: validators[0].clone(),
        producer_signature: vec![1; 96],
    };
    let envelope = iroha_data_model::block::AutonomousLanePayloadEnvelopeV1 {
        version: iroha_data_model::block::AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1,
        network_id,
        epoch: 0,
        lane_id: d.lane_id,
        dataspace_id: d.dataspace_id,
        lane_incarnation: d.lane_incarnation,
        proposal_height: d.proposal_height,
        lane_block_height: d.lane_block_height,
        lane_block_view: d.lane_block_view,
        proposal_hash: proposal.proposal_hash,
        descriptor_hash: d.descriptor_hash,
        payload_hash: payload.payload_hash,
        producer: payload.producer.clone(),
        canonical_payload: norito::encode_canonical(&payload).expect("canonical fixture payload"),
    };
    (envelope, record, validators, transaction_hash)
}

#[test]
fn contract_v1_rbc_autonomous_join_matches_without_ordinary_ownership() {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let (envelope, record, validators, hash) = contract_rbc_autonomous_join_fixture();
    let binding = canonical_autonomous_contract_binding(&envelope, 3, envelope.network_id)
        .expect("join canonical autonomous body");
    assert!(matches!(
        binding,
        CanonicalContractRbcBinding::Autonomous(_)
    ));
    assert!(contract_rbc_binding_matches(
        &binding,
        &record,
        &validators,
        Some((4, &hash))
    ));
    assert!(contract_rbc_progress_failures(&record, None, 4, 3).is_empty());
}

#[test]
fn contract_v1_rbc_autonomous_join_rejects_identity_committee_and_transaction_substitution() {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let (envelope, record, validators, hash) = contract_rbc_autonomous_join_fixture();
    let binding = canonical_autonomous_contract_binding(&envelope, 3, envelope.network_id).unwrap();
    for field in [
        "proposal",
        "descriptor",
        "incarnation",
        "subject",
        "ownership",
        "rbc",
        "view",
    ] {
        let mut wrong = record.clone();
        match field {
            "proposal" => wrong.proposal_hash = Hash::new(b"other"),
            "descriptor" => wrong.descriptor_hash = Hash::new(b"other"),
            "incarnation" => wrong.lane_incarnation = Hash::new(b"other"),
            "subject" => wrong.subject_hash = Hash::new(b"other"),
            "ownership" => wrong.payload_ownership_hash = Hash::new(b"other"),
            "rbc" => wrong.rbc_instance_hash = Hash::new(b"other"),
            "view" => wrong.lane_block_view += 1,
            _ => unreachable!(),
        }
        assert!(
            !contract_rbc_binding_matches(&binding, &wrong, &validators, Some((4, &hash))),
            "{field}"
        );
    }
    let mut wrong_roster = validators.clone();
    wrong_roster.swap(0, 1);
    assert!(!contract_rbc_binding_matches(
        &binding,
        &record,
        &wrong_roster,
        Some((4, &hash))
    ));
    assert!(!contract_rbc_binding_matches(
        &binding,
        &record,
        &validators,
        Some((2, &hash))
    ));
    assert!(!contract_rbc_binding_matches(
        &binding,
        &record,
        &validators,
        Some((4, &Hash::new(b"unrelated transaction")))
    ));
}

#[test]
fn contract_v1_rbc_autonomous_envelope_rejects_wrong_canonical_body_and_malformed_bytes() {
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let (envelope, _, _, _) = contract_rbc_autonomous_join_fixture();
    assert!(canonical_autonomous_contract_binding(&envelope, 4, envelope.network_id).is_err());
    let mut wrong = envelope.clone();
    wrong.proposal_hash = Hash::new(b"other");
    assert!(canonical_autonomous_contract_binding(&wrong, 3, envelope.network_id).is_err());
    // Rehashing the enclosing structures cannot substitute arbitrary DA
    // commitments for the canonical accepted-work preimages.
    let mut wrong_replay = envelope.clone();
    let mut payload: iroha_core::lane_consensus::LaneExecutablePayloadV1 =
        norito::decode_canonical(&wrong_replay.canonical_payload).unwrap();
    payload.origin_proposal.descriptor.subject_hash = Hash::new(b"wrong subject preimage");
    payload.origin_proposal.descriptor.descriptor_hash = payload
        .origin_proposal
        .descriptor
        .computed_descriptor_hash();
    payload.origin_proposal.proposal_hash = payload.origin_proposal.computed_proposal_hash();
    wrong_replay.descriptor_hash = payload.origin_proposal.descriptor.descriptor_hash;
    wrong_replay.proposal_hash = payload.origin_proposal.proposal_hash;
    wrong_replay.canonical_payload = norito::encode_canonical(&payload).unwrap();
    iroha_core::lane_consensus::validate_lane_block_proposal(&payload.origin_proposal)
        .expect("outer hashes remain internally consistent");
    assert!(
        canonical_autonomous_contract_binding(&wrong_replay, 3, envelope.network_id)
            .unwrap_err()
            .to_string()
            .contains("replay commitments")
    );
    for version in [0, 2] {
        let mut wrong_version = envelope.clone();
        let mut payload: iroha_core::lane_consensus::LaneExecutablePayloadV1 =
            norito::decode_canonical(&envelope.canonical_payload).unwrap();
        payload.version = version;
        wrong_version.canonical_payload = norito::encode_canonical(&payload).unwrap();
        assert!(
            canonical_autonomous_contract_binding(&wrong_version, 3, envelope.network_id)
                .unwrap_err()
                .to_string()
                .contains("payload version")
        );
    }
    let mut wrong_hashes = envelope.clone();
    let mut payload: iroha_core::lane_consensus::LaneExecutablePayloadV1 =
        norito::decode_canonical(&envelope.canonical_payload).unwrap();
    payload.entrypoint_hashes = vec![Hash::new(b"different entrypoint")];
    wrong_hashes.canonical_payload = norito::encode_canonical(&payload).unwrap();
    assert!(
        canonical_autonomous_contract_binding(&wrong_hashes, 3, envelope.network_id)
            .unwrap_err()
            .to_string()
            .contains("transaction binding")
    );
    let mut trailing = envelope.clone();
    trailing.canonical_payload.push(0);
    assert!(canonical_autonomous_contract_binding(&trailing, 3, envelope.network_id).is_err());
    let mut malformed = envelope.clone();
    malformed.canonical_payload = vec![1, 2, 3];
    assert!(canonical_autonomous_contract_binding(&malformed, 3, envelope.network_id).is_err());
}

#[test]
fn contract_v1_rbc_progress_rejects_missing_quorum_payload_and_baseline_only() {
    let (_, record, _, _) = contract_rbc_autonomous_join_fixture();
    assert_eq!(
        contract_rbc_progress_failures(&record, Some(&record), 4, 3),
        vec!["not_later_in_same_lane_incarnation"]
    );
    let mut wrong = record.clone();
    wrong.prepare_qc_signer_count = 2;
    assert!(contract_rbc_progress_failures(&wrong, None, 4, 3).contains(&"exact_quorum_signers"));
    wrong.prepare_qc_signer_count = 4;
    assert!(contract_rbc_progress_failures(&wrong, None, 4, 3).contains(&"exact_quorum_signers"));
    wrong = record.clone();
    wrong.executable_payload_available = false;
    assert!(contract_rbc_progress_failures(&wrong, None, 4, 3).contains(&"payload_unavailable"));
    wrong = record.clone();
    wrong.execution_status = "rejected_preflight".to_owned();
    assert!(contract_rbc_progress_failures(&wrong, None, 4, 3).contains(&"execution_status"));
    wrong = record.clone();
    wrong.rbc_instance_hash = Hash::prehashed([0; 32]);
    assert!(contract_rbc_progress_failures(&wrong, None, 4, 3).contains(&"zero_identity"));
    wrong = record.clone();
    wrong.lane_block_height += 1;
    assert!(contract_rbc_progress_failures(&wrong, Some(&record), 4, 3).is_empty());
    wrong.lane_incarnation = Hash::new(b"other incarnation");
    assert!(
        contract_rbc_progress_failures(&wrong, Some(&record), 4, 3)
            .contains(&"not_later_in_same_lane_incarnation")
    );
}

#[test]
fn contract_v1_rbc_ordinary_join_rejects_wrong_proposal_and_replay_material() {
    use iroha_data_model::block::consensus::SumeragiLanePayloadOwnership;
    iroha_data_model::isi::set_instruction_registry(
        iroha_data_model::instruction_registry::default(),
    );
    let (envelope, record, validators, hash) = contract_rbc_autonomous_join_fixture();
    let payload: iroha_core::lane_consensus::LaneExecutablePayloadV1 =
        norito::decode_canonical(&envelope.canonical_payload).unwrap();
    let d = payload.origin_proposal.descriptor;
    // This models an actual ordinary canonical ownership, not an autonomous
    // envelope reclassified by the query helper.
    let ownership = SumeragiLanePayloadOwnership {
        proposal_height: d.proposal_height,
        proposal_view: 0,
        lane_id: d.lane_id,
        dataspace_id: d.dataspace_id,
        lane_incarnation: d.lane_incarnation,
        lane_block_height: d.lane_block_height,
        lane_block_view: d.lane_block_view,
        subject_hash: d.subject_hash,
        qc_mode_tag: d.qc_mode_tag.clone(),
        accepted_candidate_indices: d.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: d.accepted_transaction_hashes.clone(),
        previous_lane_block_height: d.previous_lane_block_height,
        previous_lane_block_descriptor_hash: d.previous_lane_block_descriptor_hash,
        lane_block_descriptor_hash: Some(d.descriptor_hash),
        lane_block_descriptor_validator_set: d.validator_set,
        lane_block_descriptor_validator_count: d.validator_count,
        lane_block_descriptor_min_quorum: d.min_quorum,
        payload_ownership_hash: d.payload_ownership_hash,
        rbc_instance_hash: d.rbc_instance_hash,
    };
    let binding = canonical_ordinary_contract_binding(&ownership).expect("valid ordinary replay");
    assert!(matches!(binding, CanonicalContractRbcBinding::Ordinary(_)));
    assert!(contract_rbc_binding_matches(
        &binding,
        &record,
        &validators,
        Some((4, &hash))
    ));
    let mut wrong = record.clone();
    wrong.proposal_hash = Hash::new(b"unrelated ordinary proposal");
    assert!(!contract_rbc_binding_matches(
        &binding,
        &wrong,
        &validators,
        Some((4, &hash))
    ));
    let mut wrong_replay = ownership;
    wrong_replay.accepted_transaction_hashes = vec![Hash::new(b"different work")];
    assert!(
        canonical_ordinary_contract_binding(&wrong_replay)
            .unwrap_err()
            .to_string()
            .contains("ordinary ownership")
    );
}

#[test]
fn contract_v1_rbc_canonical_query_retries_retain_errors_and_stop_at_budget_or_success() {
    let mut cache = CanonicalContractRbcCache::default();
    cache
        .record_query(Err("transient first read".to_owned()))
        .unwrap();
    assert!(cache.can_query());
    assert!(cache.bindings.is_none());
    assert_eq!(cache.errors, vec!["transient first read"]);
    cache.record_query(Ok(Vec::new())).unwrap();
    assert_eq!(cache.attempts, 2);
    assert!(cache.bindings.is_some());
    assert!(!cache.can_query());
    assert_eq!(cache.errors, vec!["transient first read"]);
    assert!(cache.record_query(Err("must not run".to_owned())).is_err());
    assert_eq!(cache.attempts, 2);

    let mut exhausted = CanonicalContractRbcCache::default();
    for attempt in 0..CONTRACT_RBC_QUERY_ATTEMPT_LIMIT {
        assert!(exhausted.can_query());
        exhausted
            .record_query(Err(format!("failed attempt {attempt}")))
            .unwrap();
    }
    assert!(!exhausted.can_query());
    assert_eq!(exhausted.errors.len(), CONTRACT_RBC_QUERY_ATTEMPT_LIMIT);
    assert!(exhausted.record_query(Ok(Vec::new())).is_err());
    assert!(exhausted.bindings.is_none());
    assert_eq!(exhausted.attempts, CONTRACT_RBC_QUERY_ATTEMPT_LIMIT);
}

#[test]
fn contract_v1_rbc_four_peer_intersection_ignores_different_local_maxima() {
    let (_, common, _, _) = contract_rbc_autonomous_join_fixture();
    let mut observations = (0..4)
        .map(|index| {
            let mut later = common.clone();
            later.lane_block_height += index + 1;
            later.proposal_hash = Hash::new(index.to_le_bytes());
            vec![common.clone(), later]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        shared_contract_rbc_record(&observations),
        Some(common.clone())
    );
    observations[3].remove(0);
    assert_eq!(shared_contract_rbc_record(&observations), None);
    observations[3].push(common.clone());
    observations[3].last_mut().unwrap().commit_qc_signer_count = 2;
    assert_eq!(shared_contract_rbc_record(&observations), None);
    assert_eq!(shared_contract_rbc_record(&observations[..3]), None);
}

#[test]
fn contract_v1_rbc_stream_end_covers_applied_height_within_the_fixture_cap() {
    assert_eq!(contract_rbc_replay_end(3, Some(3)).unwrap(), 3);
    assert_eq!(contract_rbc_replay_end(100, Some(64)).unwrap(), 64);
    assert_eq!(contract_rbc_replay_end(2, None).unwrap(), 2);
    for (head, required) in [
        (0, None),
        (1, None),
        (2, Some(3)),
        (100, Some(65)),
        (3, Some(1)),
    ] {
        assert!(contract_rbc_replay_end(head, required).is_err());
    }
}

// Fake stream rows exercise only finite replay order and cleanup. They are not
// represented as signed blocks or consensus/provenance acceptance evidence.
type ContractRbcReplayRow = (u64, HashOf<BlockHeader>, Option<HashOf<BlockHeader>>);

fn contract_rbc_fake_replay_row(height: u64) -> ContractRbcReplayRow {
    let hash = |height: u64| {
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(height.to_le_bytes()))
    };
    (height, hash(height), Some(hash(height - 1)))
}

#[tokio::test]
async fn contract_v1_rbc_stream_reads_exact_end_without_lookahead() {
    use futures_util::{StreamExt as _, TryStreamExt as _};

    let polls = std::cell::Cell::new(0);
    let mut stream =
        futures_util::stream::iter((2..=65).map(|height| Ok(contract_rbc_fake_replay_row(height))))
            .inspect(|_| polls.set(polls.get() + 1));
    let blocks = receive_contract_rbc_prefix(
        &mut stream,
        64,
        Instant::now() + Duration::from_secs(10),
        |row| *row,
    )
    .await
    .unwrap();
    assert_eq!(blocks.len(), 63);
    assert_eq!(blocks.first().unwrap().0, 2);
    assert_eq!(blocks.last().unwrap().0, 64);
    assert_eq!(polls.get(), 63);
    assert_eq!(stream.try_next().await.unwrap().unwrap().0, 65);
}

#[tokio::test]
async fn contract_v1_rbc_stream_rejects_gap_duplicate_hash_truncation_and_frame_errors() {
    let two = contract_rbc_fake_replay_row(2);
    let three = contract_rbc_fake_replay_row(3);
    let four = contract_rbc_fake_replay_row(4);
    let mut wrong_parent = three;
    wrong_parent.2 = Some(four.1);
    let mut absent_parent = two;
    absent_parent.2 = None;
    let cases: Vec<Vec<Result<ContractRbcReplayRow>>> = vec![
        vec![Ok(three), Ok(two)],
        vec![Ok(two), Ok(two)],
        vec![Ok(two), Ok(four)],
        vec![Ok(two), Ok(wrong_parent)],
        vec![Ok(absent_parent), Ok(three)],
        vec![Ok(two)],
        vec![Ok(two), Err(eyre!("retained malformed frame error"))],
    ];
    for rows in cases {
        let mut stream = futures_util::stream::iter(rows);
        assert!(
            receive_contract_rbc_prefix(
                &mut stream,
                3,
                Instant::now() + Duration::from_secs(10),
                |row| *row,
            )
            .await
            .is_err()
        );
    }
    for end in [0, 1, 65] {
        let mut stream = futures_util::stream::pending::<Result<ContractRbcReplayRow>>();
        let error = receive_contract_rbc_prefix(
            &mut stream,
            end,
            Instant::now() + Duration::from_secs(10),
            |row| *row,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("fixture bound"));
    }
}

#[tokio::test]
async fn contract_v1_rbc_stream_pending_and_expired_deadlines_are_failures() {
    let mut stream = futures_util::stream::pending::<Result<ContractRbcReplayRow>>();
    let error = receive_contract_rbc_prefix(
        &mut stream,
        2,
        Instant::now() + Duration::from_millis(1),
        |row| *row,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("timed out") || error.to_string().contains("deadline expired")
    );
    let error = receive_contract_rbc_prefix(&mut stream, 2, Instant::now(), |row| *row)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("deadline expired"));
}

#[tokio::test]
async fn contract_v1_rbc_stream_close_is_bounded_and_retains_the_read_failure() {
    let error = finish_contract_rbc_replay::<ContractRbcReplayRow>(
        Err(eyre!("original replay failure")),
        std::future::pending(),
        Instant::now() + Duration::from_millis(1),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("original replay failure"));
    assert!(error.to_string().contains("close timed out"));
    let row = contract_rbc_fake_replay_row(2);
    assert_eq!(
        finish_contract_rbc_replay(
            Ok(vec![row]),
            async {},
            Instant::now() + Duration::from_secs(10)
        )
        .await
        .unwrap(),
        vec![row],
    );
    let error = finish_contract_rbc_replay(Ok(vec![row]), std::future::pending(), Instant::now())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("close timed out"));
}

fn dynamic_counter_args(key: i64, delta: i64) -> norito::json::Value {
    norito::json::object([
        ("key", norito::json::Value::from(key.to_string())),
        ("delta", norito::json::Value::from(delta.to_string())),
    ])
    .expect("serialize dynamic counter arguments")
}
fn dynamic_counter_call_intent(
    artifact: &[u8],
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    contract_alias: &iroha_data_model::smart_contract::ContractAlias,
    entrypoint: &str,
    payload: &norito::json::Value,
) -> iroha::client::ContractCallDraftIntent {
    use iroha_data_model::transaction::executable::{ContractArgumentRecord, ContractInvocation};

    let verified =
        ivm::verify_contract_artifact(artifact).expect("verify dynamic counter artifact");
    let schema = verified
        .contract_interface
        .entrypoints
        .iter()
        .find(|descriptor| descriptor.name == entrypoint)
        .and_then(|descriptor| descriptor.argument_schema.as_ref())
        .unwrap_or_else(|| panic!("missing argument schema for `{entrypoint}`"));
    let argument_bytes = ivm::encode_argument_record_from_json(
        schema,
        &iroha_primitives::json::Json::from(payload.clone()),
    )
    .expect("encode dynamic counter argument record");
    let arguments = ContractArgumentRecord::try_new(argument_bytes)
        .expect("bound dynamic counter argument record");

    let mut metadata = Metadata::default();
    for (key, value) in [
        (
            "contract_address",
            iroha_primitives::json::Json::new(contract_address.to_string()),
        ),
        (
            "contract_code_hash",
            iroha_primitives::json::Json::new(verified.code_hash.to_string()),
        ),
        (
            "contract_alias",
            iroha_primitives::json::Json::new(contract_alias.to_string()),
        ),
        (
            "contract_entrypoint",
            iroha_primitives::json::Json::new(entrypoint.to_owned()),
        ),
        (
            "contract_payload",
            iroha_primitives::json::Json::from(payload.clone()),
        ),
    ] {
        metadata.insert(
            Name::from_str(key).expect("static contract metadata key"),
            value,
        );
    }
    iroha::client::ContractCallDraftIntent {
        invocation: ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: verified.code_hash,
            entrypoint: entrypoint.to_owned(),
            arguments: Some(arguments),
        },
        metadata,
    }
}
async fn wait_for_approved_txs(
    client: &iroha::client::Client,
    baseline: u64,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_status = None;
    let mut last_error = None;
    while Instant::now() < deadline {
        match tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await
        .expect("poll status")
        {
            Ok(status) => {
                if status.txs_approved > baseline {
                    return Ok(());
                }
                last_status = Some(status);
                last_error = None;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(eyre!(
        "{stage}: timed out waiting for txs_approved to advance beyond {baseline}; last_status={last_status:?}; last_error={last_error:?}"
    ))
}
fn pipeline_status_kind(payload: &norito::json::Value) -> Option<&str> {
    let status = payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?;
    match status {
        norito::json::Value::String(kind) => Some(kind.as_str()),
        norito::json::Value::Object(map) => map.get("kind").and_then(norito::json::Value::as_str),
        _ => None,
    }
}
fn pipeline_status_block_height(payload: &norito::json::Value) -> Option<u64> {
    payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?
        .get("block_height")
        .and_then(norito::json::Value::as_u64)
}
async fn wait_for_tx_applied(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<u64> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_kind = String::from("unavailable");
    let mut last_payload = String::new();
    let mut last_error = String::new();
    loop {
        match http
            .get(status_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(response)
                if response.status() == reqwest::StatusCode::OK
                    || response.status() == reqwest::StatusCode::ACCEPTED =>
            {
                let status = response.status();
                let bytes = response.bytes().await?;
                if bytes.is_empty() {
                    last_kind = format!("http {status} with empty body");
                } else {
                    let payload: norito::json::Value = norito::json::from_slice(&bytes)?;
                    if let Some(kind) = pipeline_status_kind(&payload) {
                        last_kind = kind.to_string();
                        last_payload = format!("{payload:?}");
                        match kind {
                            "Applied" => {
                                if let Some(block_height) = pipeline_status_block_height(&payload) {
                                    return Ok(block_height);
                                }
                                last_kind = "Applied without block_height".to_owned();
                            }
                            "Rejected" => {
                                return Err(eyre!(
                                    "{stage}: tx `{tx_hash_hex}` rejected; payload={payload:?}"
                                ));
                            }
                            "Expired" => {
                                return Err(eyre!("{stage}: tx `{tx_hash_hex}` expired"));
                            }
                            _ => {}
                        }
                    } else {
                        last_kind = "missing status kind".to_string();
                        last_payload = format!("{payload:?}");
                    }
                }
            }
            Ok(response)
                if response.status() == reqwest::StatusCode::NO_CONTENT
                    || response.status() == reqwest::StatusCode::NOT_FOUND =>
            {
                last_kind = format!("http {}", response.status());
            }
            Ok(response) => {
                last_error = format!(
                    "http {} {}",
                    response.status(),
                    std::str::from_utf8(response.bytes().await?.as_ref()).unwrap_or("")
                );
            }
            Err(err) => {
                last_error = format!("{err}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx `{tx_hash_hex}` to reach Applied; last_kind={last_kind}, last_payload={last_payload}, last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
pub(super) fn deploy_contract_locally_signed(
    client: &iroha::client::Client,
    artifact: &[u8],
    contract_alias: iroha_data_model::smart_contract::ContractAlias,
) -> Result<(
    iroha_data_model::smart_contract::ContractAddress,
    String,
    String,
    HashOf<iroha_data_model::transaction::SignedTransaction>,
)> {
    deploy_contract_locally_signed_with_registration(client, artifact, contract_alias, false, None)
}

fn deploy_contract_locally_signed_with_registration(
    client: &iroha::client::Client,
    artifact: &[u8],
    contract_alias: iroha_data_model::smart_contract::ContractAlias,
    registered_in_genesis: bool,
    hajimari_grantee: Option<&AccountId>,
) -> Result<(
    iroha_data_model::smart_contract::ContractAddress,
    String,
    String,
    HashOf<iroha_data_model::transaction::SignedTransaction>,
)> {
    use iroha_data_model::isi::smart_contract_code::{
        CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
        SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
    };
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify contract artifact: {error}"))?;
    let authority: Account = client.query_single(FindAccountById::new(client.account.clone()))?;
    let nonce_key =
        Name::from_str(iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY)?;
    let deploy_nonce = authority
        .metadata()
        .get(&nonce_key)
        .map(|value| {
            value
                .try_into_any_norito::<u64>()
                .map_err(|_| eyre!("contract deployment nonce metadata is not a canonical u64"))
        })
        .transpose()?
        .unwrap_or(0);
    let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
        &client.network_id,
        &client.account,
        deploy_nonce,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .map_err(|error| eyre!("derive contract address: {error}"))?;
    let mut metadata = Metadata::default();
    for key in ["gov_contract_address", "contract_address"] {
        metadata.insert(
            Name::from_str(key)?,
            iroha_primitives::json::Json::new(contract_address.to_string()),
        );
    }
    if !registered_in_genesis {
        let manifest = verified
            .manifest
            .try_signed(&client.key_pair)
            .map_err(|error| eyre!("sign contract manifest locally: {error}"))?;
        let total_size = u64::try_from(artifact.len())?;
        let chunk_count = u32::try_from(artifact.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES))?;
        if chunk_count == 0 {
            return Err(eyre!("contract artifact must not be empty"));
        }
        for (index, chunk) in artifact.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
            let chunk_index = u32::try_from(index)?;
            let mut instructions = vec![InstructionBox::from(UploadSmartContractCodeChunk {
                code_hash: verified.code_hash,
                total_size,
                chunk_index,
                chunk_count,
                chunk: chunk.to_vec(),
            })];
            if chunk_index + 1 == chunk_count {
                instructions.push(InstructionBox::from(FinalizeSmartContractCodeUpload {
                    code_hash: verified.code_hash,
                    total_size,
                    chunk_count,
                }));
            }
            client.submit_all_blocking_with_metadata(
                instructions,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                metadata.clone(),
            )?;
        }
        client.submit_blocking_with_metadata(
            RegisterSmartContractCode { manifest },
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            metadata.clone(),
        )?;
    }
    let deployment_instructions = contract_deployment_instructions(
        CommitContractDeployment {
            expected_deploy_nonce: deploy_nonce,
            contract_address: contract_address.clone(),
            code_hash: verified.code_hash,
            contract_alias,
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        },
        hajimari_grantee,
    );
    let deployment_tx_hash = client.submit_all_blocking_with_metadata(
        deployment_instructions,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        metadata,
    )?;
    Ok((
        contract_address,
        hex::encode(verified.code_hash.as_ref()),
        hex::encode(verified.abi_hash.as_ref()),
        deployment_tx_hash,
    ))
}
async fn deploy_contract_artifact(
    client: &iroha::client::Client,
    http: &reqwest::Client,
    artifact: &[u8],
    alias_name: &str,
    stage: &str,
) -> Result<(iroha_data_model::smart_contract::ContractAddress, Hash, u64)> {
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        alias_name,
        None,
        "universal",
    )
    .map_err(|error| eyre!("{stage}: invalid contract alias: {error}"))?;
    let (contract_address, _, _, deployment_tx_hash) = tokio::task::spawn_blocking({
        let client = client.clone();
        let artifact = artifact.to_vec();
        move || deploy_contract_locally_signed(&client, &artifact, contract_alias)
    })
    .await
    .expect("deploy contract task")?;
    let deployment_block_height = wait_for_tx_applied(
        http,
        &client.torii_url,
        &hex::encode(deployment_tx_hash.as_ref()),
        Duration::from_secs(60),
        stage,
    )
    .await?;
    Ok((
        contract_address,
        Hash::from(deployment_tx_hash),
        deployment_block_height,
    ))
}
async fn contract_state_json_value(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    path: &str,
) -> Result<norito::json::Value> {
    let mut url = torii_url.join("v1/contracts/state")?;
    url.query_pairs_mut()
        .append_pair("contract_address", &contract_address.to_string())
        .append_pair("path", path)
        .append_pair("decode", "json");
    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(eyre!("contract state `{path}` returned {status}: {body}"));
    }
    let payload: norito::json::Value = norito::json::from_str(&body)?;
    let entry = payload
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .and_then(|entries| entries.first())
        .ok_or_else(|| eyre!("contract state `{path}` response missing entry: {payload:?}"))?;
    if entry.get("found").and_then(norito::json::Value::as_bool) != Some(true) {
        return Err(eyre!("contract state `{path}` was not found: {payload:?}"));
    }
    entry
        .get("value_json")
        .cloned()
        .ok_or_else(|| eyre!("contract state `{path}` was not decoded: {payload:?}"))
}

async fn contract_view_json_value(
    client: iroha::client::Client,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
) -> Result<norito::json::Value> {
    let contract_address = contract_address.clone();
    let entrypoint = entrypoint.to_owned();
    let response = tokio::task::spawn_blocking(move || {
        client.post_contract_view_json(
            &iroha_test_samples::ALICE_ID,
            Some(&contract_address),
            None,
            &entrypoint,
            None,
            CONTRACT_STATE_PROBE_GAS_LIMIT,
        )
    })
    .await??;
    response
        .get("result")
        .cloned()
        .ok_or_else(|| eyre!("contract view response is missing result: {response:?}"))
}

async fn wait_for_contract_applied_height(
    peer: &iroha_test_network::NetworkPeer,
    height: u64,
    timeout: Duration,
) -> Result<()> {
    let mut last_observation = "no applied-state status response".to_owned();
    tokio::time::timeout(timeout, async {
        loop {
            match peer.status().await {
                Ok(status) if status.blocks >= height => return,
                Ok(status) => last_observation = format!("applied height {}", status.blocks),
                Err(error) => last_observation = error.to_string(),
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .map_err(|_| eyre!("contract peer did not apply height {height}: {last_observation}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContractLifecycleSnapshot {
    version: u64,
    active: bool,
    origin: String,
    origin_account: String,
    owner: String,
    pending_owner: Option<String>,
    parliament_delegated: bool,
    active_code_hash_hex: Option<String>,
    revision: u64,
    emergency_hold_present: bool,
    emergency_hold_active: bool,
}

fn optional_json_string(
    value: Option<&norito::json::Value>,
    field: &str,
) -> Result<Option<String>> {
    match value {
        None => Err(eyre!("governed contract response is missing `{field}`")),
        Some(norito::json::Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| eyre!("governed contract field `{field}` is not a string or null")),
    }
}

fn contract_lifecycle_snapshot(
    response: &norito::json::Value,
) -> Result<ContractLifecycleSnapshot> {
    if response.get("found").and_then(norito::json::Value::as_bool) != Some(true) {
        return Err(eyre!(
            "governed contract response has no retained lifecycle: {response:?}"
        ));
    }
    let active = response
        .get("active")
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| eyre!("governed contract response has no `active` flag: {response:?}"))?;
    let emergency_hold_active = response
        .get("emergency_hold_active")
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| eyre!("governed contract response has no hold status: {response:?}"))?;
    let lifecycle = response
        .get("lifecycle")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("governed contract response has no lifecycle: {response:?}"))?;
    let string_field = |field: &str| -> Result<String> {
        lifecycle
            .get(field)
            .and_then(norito::json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| eyre!("governed contract lifecycle has no `{field}`: {response:?}"))
    };
    let parliament_delegated = lifecycle
        .get("parliament_delegated")
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| eyre!("governed contract lifecycle has no delegation flag: {response:?}"))?;
    let revision = lifecycle
        .get("revision")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("governed contract lifecycle has no revision: {response:?}"))?;
    let version = lifecycle
        .get("version")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("governed contract lifecycle has no version: {response:?}"))?;
    let emergency_hold_present = match lifecycle.get("emergency_hold") {
        None => {
            return Err(eyre!(
                "governed contract lifecycle has no emergency-hold field: {response:?}"
            ));
        }
        Some(norito::json::Value::Null) => false,
        Some(norito::json::Value::Object(_)) => true,
        Some(_) => {
            return Err(eyre!(
                "governed contract lifecycle has malformed emergency-hold state: {response:?}"
            ));
        }
    };
    Ok(ContractLifecycleSnapshot {
        version,
        active,
        origin: string_field("origin")?,
        origin_account: string_field("origin_account")?,
        owner: string_field("owner")?,
        pending_owner: optional_json_string(lifecycle.get("pending_owner"), "pending_owner")?,
        parliament_delegated,
        active_code_hash_hex: optional_json_string(
            lifecycle.get("active_code_hash_hex"),
            "active_code_hash_hex",
        )?,
        revision,
        emergency_hold_present,
        emergency_hold_active,
    })
}

async fn wait_for_contract_lifecycle_on_all_peers(
    network: &iroha_test_network::Network,
    contract_address: &ContractAddress,
    expected: &ContractLifecycleSnapshot,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut last_observed = Vec::new();
    loop {
        last_observed.clear();
        let mut converged = true;
        for (peer_index, peer) in network.peers().iter().enumerate() {
            let client = peer.client_for(
                &iroha_test_samples::ALICE_ID,
                iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
            );
            let contract_address = contract_address.clone();
            let observed = tokio::task::spawn_blocking(move || {
                client
                    .get_gov_contract_json(&contract_address)
                    .and_then(|response| contract_lifecycle_snapshot(&response))
            })
            .await
            .map_err(|error| eyre!("{stage}: peer {peer_index} lifecycle task failed: {error}"))?;
            match observed {
                Ok(snapshot) => {
                    converged &= &snapshot == expected;
                    last_observed.push(format!("peer {peer_index}: {snapshot:?}"));
                }
                Err(error) => {
                    converged = false;
                    last_observed.push(format!("peer {peer_index}: error={error:#}"));
                }
            }
        }
        if converged {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: four peers did not converge on {expected:?}; last observations: {}",
                last_observed.join("; ")
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn assert_submission_error_contains(error: &eyre::Report, expected: &str, stage: &str) {
    let message = format!("{error:#}");
    assert!(
        message.contains(expected),
        "{stage}: expected rejection containing `{expected}`, got `{message}`"
    );
}

#[tokio::test]
async fn contract_owner_lifecycle_cas_and_transfer_converge_on_four_peers() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(4))
        .with_npos_consensus()
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let context = stringify!(contract_owner_lifecycle_cas_and_transfer_converge_on_four_peers);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");
    let handshake = signed_consensus_handshake(&network)?;
    handshake
        .validate()
        .map_err(|error| eyre!("invalid signed consensus handshake: {error}"))?;
    assert_eq!(handshake.mode, SumeragiConsensusMode::Npos);
    assert_eq!(
        handshake.sumeragi_v2.da_layout,
        recommended_data_availability_layout(),
        "contract lifecycle gate requires the signed mandatory DA layout"
    );
    network.ensure_blocks(1).await?;

    let alice = network.peers()[0].client_for(
        &iroha_test_samples::ALICE_ID,
        iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
    );
    let bob = network.peers()[1].client_for(
        &iroha_test_samples::BOB_ID,
        iroha_test_samples::BOB_KEYPAIR.private_key().clone(),
    );
    let artifact = minimal_contract_artifact();
    let verified = ivm::verify_contract_artifact(&artifact)
        .map_err(|error| eyre!("verify lifecycle test artifact: {error}"))?;
    let code_hash = verified.code_hash;
    let code_hash_hex = hex::encode(code_hash.as_ref());
    let alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "owner_lifecycle_four_peer",
        None,
        "universal",
    )?;
    let (contract_address, _, _, _) = deploy_contract_locally_signed(&alice, &artifact, alias)?;

    let direct_active = ContractLifecycleSnapshot {
        version: 1,
        active: true,
        origin: "direct".to_owned(),
        origin_account: iroha_test_samples::ALICE_ID.to_string(),
        owner: iroha_test_samples::ALICE_ID.to_string(),
        pending_owner: None,
        parliament_delegated: false,
        active_code_hash_hex: Some(code_hash_hex.clone()),
        revision: 1,
        emergency_hold_present: false,
        emergency_hold_active: false,
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &direct_active,
        "direct deployment lifecycle",
    )
    .await?;

    let unauthorized = bob
        .submit_blocking(
            DeactivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                reason: Some("unauthorized takeover attempt".to_owned()),
            },
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("a non-owner must not deactivate the contract");
    assert_submission_error_contains(
        &unauthorized,
        "only the current account owner may deactivate",
        "non-owner deactivation",
    );
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &direct_active,
        "non-owner deactivation rollback",
    )
    .await?;

    alice.submit_blocking(
        DeactivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 1,
            reason: Some("owner maintenance".to_owned()),
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let inactive_revision_2 = ContractLifecycleSnapshot {
        active: false,
        active_code_hash_hex: None,
        revision: 2,
        ..direct_active.clone()
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &inactive_revision_2,
        "owner deactivation",
    )
    .await?;

    let stale_activation = alice
        .submit_blocking(
            ActivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 1,
                code_hash,
            },
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("a consumed lifecycle revision must reject activation");
    assert_submission_error_contains(
        &stale_activation,
        "stale contract lifecycle revision",
        "stale owner activation",
    );

    alice.submit_blocking(
        SetContractParliamentDelegation {
            contract_address: contract_address.clone(),
            expected_revision: 2,
            delegated: true,
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let delegated_revision_3 = ContractLifecycleSnapshot {
        parliament_delegated: true,
        revision: 3,
        ..inactive_revision_2.clone()
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &delegated_revision_3,
        "Parliament delegation",
    )
    .await?;

    alice.submit_blocking(
        SetContractParliamentDelegation {
            contract_address: contract_address.clone(),
            expected_revision: 3,
            delegated: false,
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let revoked_revision_4 = ContractLifecycleSnapshot {
        parliament_delegated: false,
        revision: 4,
        ..delegated_revision_3
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &revoked_revision_4,
        "Parliament delegation revocation",
    )
    .await?;

    alice.submit_blocking(
        OfferContractOwnership {
            contract_address: contract_address.clone(),
            expected_revision: 4,
            new_owner: ContractLifecycleOwnerV1::Account(iroha_test_samples::BOB_ID.clone()),
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let offered_revision_5 = ContractLifecycleSnapshot {
        pending_owner: Some(iroha_test_samples::BOB_ID.to_string()),
        revision: 5,
        ..revoked_revision_4
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &offered_revision_5,
        "two-party ownership offer",
    )
    .await?;

    let wrong_acceptor = alice
        .submit_blocking(
            AcceptContractOwnership {
                contract_address: contract_address.clone(),
                expected_revision: 5,
            },
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("the current owner cannot accept an offer made to another account");
    assert_submission_error_contains(
        &wrong_acceptor,
        "authority is not the pending account owner",
        "wrong ownership acceptor",
    );
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &offered_revision_5,
        "wrong ownership acceptor rollback",
    )
    .await?;

    bob.submit_blocking(
        AcceptContractOwnership {
            contract_address: contract_address.clone(),
            expected_revision: 5,
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let transferred_revision_6 = ContractLifecycleSnapshot {
        owner: iroha_test_samples::BOB_ID.to_string(),
        pending_owner: None,
        parliament_delegated: false,
        revision: 6,
        ..offered_revision_5
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &transferred_revision_6,
        "two-party ownership acceptance",
    )
    .await?;

    let former_owner = alice
        .submit_blocking(
            ActivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 6,
                code_hash,
            },
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("the former owner must not reactivate the transferred contract");
    assert_submission_error_contains(
        &former_owner,
        "only the current account owner may activate",
        "former-owner activation",
    );

    bob.submit_blocking(
        ActivateContractInstance {
            contract_address: contract_address.clone(),
            expected_revision: 6,
            code_hash,
        },
        FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let bob_active_revision_7 = ContractLifecycleSnapshot {
        active: true,
        active_code_hash_hex: Some(code_hash_hex),
        revision: 7,
        ..transferred_revision_6
    };
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &bob_active_revision_7,
        "new-owner activation",
    )
    .await?;

    let stale_deactivation = bob
        .submit_blocking(
            DeactivateContractInstance {
                contract_address: contract_address.clone(),
                expected_revision: 6,
                reason: Some("stale replay".to_owned()),
            },
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .expect_err("the activation revision must not be replayable");
    assert_submission_error_contains(
        &stale_deactivation,
        "stale contract lifecycle revision",
        "stale new-owner deactivation",
    );
    wait_for_contract_lifecycle_on_all_peers(
        &network,
        &contract_address,
        &bob_active_revision_7,
        "stale new-owner deactivation rollback",
    )
    .await
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn deploy_and_get_contract_manifest_via_torii() -> Result<()> {
    // Grant CanRegisterSmartContractCode to Alice in genesis so she can deploy contracts.
    let permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        // Keep pipeline timings short to ensure the deploy transaction is flushed promptly.
        .with_block_cadence(std::time::Duration::from_secs(4))
        .with_config_layer(|layer| {
            // Surface more detail if the pipeline stalls while registering the contract.
            layer.write(["logger", "level"], "TRACE").write(
                ["logger", "filter"],
                "iroha_core::sumeragi=trace,iroha_core::queue=trace,iroha_core::smartcontracts=trace,iroha_core::tx=trace",
            );
        })
        .with_genesis_instruction(Grant::account_permission(
            permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(deploy_and_get_contract_manifest_via_torii),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();
    // Wait for genesis to be committed before submitting additional transactions
    network.ensure_blocks(1).await?;
    let code_bytes = minimal_contract_artifact();
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "deploy_test",
        None,
        "universal",
    )
    .expect("contract alias");
    let (_, code_hash_hex, _, _) = tokio::task::spawn_blocking({
        let client = client.clone();
        move || deploy_contract_locally_signed(&client, &code_bytes, contract_alias)
    })
    .await
    .expect("locally signed contract deployment task")?;
    let http = integration_tests::http::client();
    // Poll status until we see the deploy transaction committed
    let deadline = Instant::now() + std::time::Duration::from_secs(120);
    let mut status = None;
    let mut last_status_error: Option<String> = None;
    while Instant::now() < deadline {
        match tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await
        .expect("poll status")
        {
            Ok(current) => {
                let non_empty = current.blocks_non_empty;
                status = Some(current);
                last_status_error = None;
                if non_empty >= 2 {
                    break;
                }
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let status = status.ok_or_else(|| {
        eyre!(
            "failed to fetch status before deadline{}",
            last_status_error
                .as_deref()
                .map(|err| format!("; last error: {err}"))
                .unwrap_or_default()
        )
    })?;
    if status.blocks_non_empty < 2 {
        return Err(eyre!(
            "expected blocks_non_empty>=2 after manifest registration, got {} (blocks {}, queue {}, approved {}, rejected {}; last status error: {})",
            status.blocks_non_empty,
            status.blocks,
            status.queue_size,
            status.txs_approved,
            status.txs_rejected,
            last_status_error.as_deref().unwrap_or("none")
        ));
    }
    // GET by code hash
    let get_url = client
        .torii_url
        .join(&format!("/v1/contracts/code/{code_hash_hex}"))
        .unwrap();
    let get_deadline = Instant::now() + std::time::Duration::from_secs(120);
    let mut got_txt = None;
    let mut last_get_error: Option<String> = None;
    while Instant::now() < get_deadline {
        let resp = http
            .get(get_url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status == StatusCode::NOT_FOUND {
            last_get_error = Some("manifest not found".to_owned());
        } else if !status.is_success() {
            return Err(eyre!(
                "GET /v1/contracts/code/{code_hash_hex} returned {status}: {body}"
            ));
        } else if body.trim().is_empty() {
            last_get_error = Some("empty response body".to_owned());
        } else {
            got_txt = Some(body);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let got_txt = got_txt.ok_or_else(|| {
        eyre!(
            "manifest GET did not return JSON before deadline{}",
            last_get_error
                .as_deref()
                .map(|err| format!("; last error: {err}"))
                .unwrap_or_default()
        )
    })?;
    let got: norito::json::Value = norito::json::from_str(&got_txt)?;
    // Validate manifest present and code_bytes absent
    let (got_manifest, got_bytes) = match &got {
        norito::json::Value::Object(m) => (
            m.get("manifest")
                .cloned()
                .unwrap_or(norito::json::Value::Null),
            m.get("code_bytes")
                .cloned()
                .unwrap_or(norito::json::Value::Null),
        ),
        _ => (norito::json::Value::Null, norito::json::Value::Null),
    };
    let got_code = match &got_manifest {
        norito::json::Value::Object(m) => m.get("code_hash").and_then(|v| v.as_str()),
        _ => None,
    };
    assert_eq!(got_code, Some(code_hash_hex.as_str()));
    assert!(got_bytes.is_null(), "code_bytes must be null/absent");
    Ok(())
}
#[tokio::test]
async fn dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let alice_enact_permission: Permission = CanEnactGovernance.into();
    let bob_enact_permission: Permission = CanEnactGovernance.into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(4))
        .with_config_layer(|layer| {
            layer
                .write(["pipeline", "dynamic_prepass"], true)
                .write(["pipeline", "parallel_overlay"], true)
                .write(["pipeline", "parallel_apply"], true)
                .write(["pipeline", "workers"], 2_i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            alice_enact_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            bob_enact_permission,
            iroha_test_samples::BOB_ID.clone(),
        ));
    let context = stringify!(dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");
    network.ensure_blocks(1).await?;
    let alice_client = network.peers()[0].client();
    let bob_client = network.peers()[1].client();
    let http = integration_tests::http::client();
    let artifact = dynamic_access_counter_artifact();
    let (contract_address, _, deploy_height) = deploy_contract_artifact(
        &alice_client,
        &http,
        &artifact,
        "dynamic_access_counter",
        "deploy dynamic-access counter",
    )
    .await?;
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "dynamic_access_counter",
        None,
        "universal",
    )
    .expect("dynamic counter alias");
    network.ensure_blocks(deploy_height).await?;
    let alice_submission = tokio::task::spawn_blocking({
        let client = alice_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 3);
        let intent = dynamic_counter_call_intent(
            &artifact,
            &contract_address,
            &contract_alias,
            "bump_direct",
            &payload,
        );
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::ALICE_ID.clone(),
                Some(iroha_test_samples::ALICE_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_direct",
                Some(&payload),
                None,
                None,
                None,
                &FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100_000)),
                &intent,
            )
        }
    });
    let bob_submission = tokio::task::spawn_blocking({
        let client = bob_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 5);
        let intent = dynamic_counter_call_intent(
            &artifact,
            &contract_address,
            &contract_alias,
            "bump_via_helper",
            &payload,
        );
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::BOB_ID.clone(),
                Some(iroha_test_samples::BOB_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_via_helper",
                Some(&payload),
                None,
                None,
                None,
                &FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100_000)),
                &intent,
            )
        }
    });
    let (alice_response, bob_response) = tokio::join!(alice_submission, bob_submission);
    let alice_response = alice_response.expect("submit direct bump task")?;
    let bob_response = bob_response.expect("submit helper bump task")?;
    let alice_tx_hash = alice_response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("direct bump response missing tx_hash_hex: {alice_response:?}"))?
        .to_owned();
    let bob_tx_hash = bob_response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("helper bump response missing tx_hash_hex: {bob_response:?}"))?
        .to_owned();
    let (alice_block_height, bob_block_height) = tokio::try_join!(
        wait_for_tx_applied(
            &http,
            &alice_client.torii_url,
            &alice_tx_hash,
            Duration::from_secs(60),
            "direct dynamic bump",
        ),
        wait_for_tx_applied(
            &http,
            &bob_client.torii_url,
            &bob_tx_hash,
            Duration::from_secs(60),
            "helper-hidden dynamic bump",
        ),
    )?;
    assert_eq!(
        alice_block_height, bob_block_height,
        "concurrent conflicting calls must be observed in the same committed block"
    );
    network.ensure_blocks(alice_block_height).await?;
    let mut peer_values = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        let peer_client = peer.client();
        peer_values.push(
            contract_state_json_value(
                &http,
                &peer_client.torii_url,
                &contract_address,
                "Counters/7",
            )
            .await?,
        );
    }
    let expected = norito::json::Value::from("8");
    assert!(
        peer_values.iter().all(|value| value == &expected),
        "conflicting dynamic calls lost an update or peers diverged: {peer_values:?}"
    );
    assert!(
        peer_values.windows(2).all(|pair| pair[0] == pair[1]),
        "contract state differs across voting peers: {peer_values:?}"
    );
    Ok(())
}
#[tokio::test]
async fn typed_core_query_pagination_is_deterministic_on_four_peers() -> Result<()> {
    let seeded_accounts = (0..6)
        .map(|index| {
            KeyPair::try_from_seed(
                format!("typed-core-query-account-{index}").into_bytes(),
                Algorithm::Ed25519,
            )
            .expect("derive deterministic typed-query account keypair")
            .public_key()
            .clone()
        })
        .map(AccountId::new)
        .collect::<Vec<_>>();
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let manage_alias_permission: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(4))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["nexus", "lane_count"], 1i64)
                // Contract views share the public contract-route limiter with
                // deployments. This gate deliberately walks every typed page
                // on every peer in a tight sequence, so give the semantic test
                // a deterministic budget instead of depending on token refill
                // timing between otherwise single-pass page requests.
                .write(["torii", "deploy_rate_per_origin_per_sec"], 10_000i64)
                .write(["torii", "deploy_burst_per_origin"], 10_000i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            manage_alias_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    for account_id in &seeded_accounts {
        builder =
            builder.with_genesis_instruction(Register::account(Account::new(account_id.clone())));
    }
    for index in 0_u64..6 {
        let domain_id = DomainId::try_new(format!("typed-query-{index}"), "universal")?;
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            format!("coin{index}").parse()?,
        );
        let asset_id = AssetId::new(
            asset_definition_id.clone(),
            iroha_test_samples::ALICE_ID.clone(),
        );
        let nft_id = NftId::new(domain_id.clone(), format!("item{index}").parse()?);
        builder = builder
            .with_genesis_instruction(Register::domain(Domain::new(domain_id)))
            .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
                asset_definition_id.clone(),
                format!("Typed query asset {index}"),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )))
            .with_genesis_instruction(Mint::asset_quantity(index + 1, asset_id))
            .with_genesis_instruction(Register::nft(Nft::new(nft_id, Metadata::default())));
    }
    let context = stringify!(typed_core_query_pagination_is_deterministic_on_four_peers);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");
    let handshake = signed_consensus_handshake(&network)?;
    handshake
        .validate()
        .map_err(|error| eyre!("invalid signed consensus handshake: {error}"))?;
    assert_eq!(
        handshake.mode,
        SumeragiConsensusMode::Npos,
        "typed-query pagination gate requires the Sora NPoS profile"
    );
    assert_eq!(
        handshake.sumeragi_v2.da_layout,
        recommended_data_availability_layout(),
        "typed-query pagination gate requires the signed mandatory DA layout"
    );
    network.ensure_blocks(1).await?;
    let rbc_baseline =
        wait_for_cross_peer_rbc_diagnostics(&network, Duration::from_secs(120), None, None).await?;
    let deploy_client = network.peers()[0].client();
    let http = integration_tests::http::client();
    let (contract_address, deployment_tx_hash, deploy_height) = deploy_contract_artifact(
        &deploy_client,
        &http,
        &typed_core_query_pager_artifact(),
        "typed_core_query_pager",
        "deploy typed core-query pager",
    )
    .await?;
    network.ensure_blocks(deploy_height).await?;
    wait_for_cross_peer_rbc_diagnostics(
        &network,
        Duration::from_secs(120),
        Some(&rbc_baseline),
        Some((deploy_height, &deployment_tx_hash)),
    )
    .await?;
    let (account_ids, asset_ids, asset_definition_ids, domain_ids, nft_ids) =
        tokio::task::spawn_blocking({
            let client = deploy_client.clone();
            move || -> Result<_> {
                let account_ids = client
                    .query(FindAccounts)
                    .execute_all()?
                    .into_iter()
                    .map(|account| account.id().clone())
                    .collect::<Vec<_>>();
                let asset_ids = client
                    .query(FindAssets::new())
                    .execute_all()?
                    .into_iter()
                    .map(|asset| asset.id().clone())
                    .collect::<Vec<_>>();
                let asset_definition_ids = client
                    .query(FindAssetsDefinitions::new())
                    .execute_all()?
                    .into_iter()
                    .map(|definition| definition.id().clone())
                    .collect::<Vec<_>>();
                let domain_ids = client
                    .query(FindDomains::new())
                    .execute_all()?
                    .into_iter()
                    .map(|domain| domain.id().clone())
                    .collect::<Vec<_>>();
                let nft_ids = client
                    .query(FindNfts::new())
                    .execute_all()?
                    .into_iter()
                    .map(|nft| nft.id().clone())
                    .collect::<Vec<_>>();
                Ok((
                    account_ids,
                    asset_ids,
                    asset_definition_ids,
                    domain_ids,
                    nft_ids,
                ))
            }
        })
        .await??;
    assert_canonical_query_order(&account_ids, "account");
    assert_canonical_query_order(&asset_ids, "asset");
    assert_canonical_query_order(&asset_definition_ids, "asset-definition");
    assert_canonical_query_order(&domain_ids, "domain");
    assert_canonical_query_order(&nft_ids, "NFT");
    let families: [(&str, &str, Vec<String>, &[&str]); 5] = [
        (
            "accounts",
            "AccountView",
            account_ids.into_iter().map(|id| id.to_string()).collect(),
            &["id", "metadata"],
        ),
        (
            "assets",
            "AssetView",
            asset_ids.into_iter().map(|id| id.to_string()).collect(),
            &["id", "amount"],
        ),
        (
            "asset_definitions",
            "AssetDefinitionView",
            asset_definition_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            &[
                "id",
                "name",
                "description",
                "owned_by",
                "total_quantity",
                "metadata",
            ],
        ),
        (
            "domains",
            "DomainView",
            domain_ids.into_iter().map(|id| id.to_string()).collect(),
            &["id", "owned_by", "metadata"],
        ),
        (
            "nfts",
            "NftView",
            nft_ids.into_iter().map(|id| id.to_string()).collect(),
            &["id", "owned_by", "content"],
        ),
    ];
    for (entrypoint, _, expected_ids, _) in &families {
        assert!(
            (6..=64).contains(&expected_ids.len()),
            "the {entrypoint} fixture must fit one bounded page and contain two partial pages; \
             found {} entities",
            expected_ids.len()
        );
    }
    const CURSOR_PAGE_LIMIT: i64 = 3;
    let mut peer_results = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        let mut family_pages = Vec::with_capacity(families.len());
        for (entrypoint, view_name, expected_ids, _) in &families {
            let expected_cursor_pages = expected_ids
                .len()
                .div_ceil(usize::try_from(CURSOR_PAGE_LIMIT).expect("positive page limit"));
            let mut pages = Vec::with_capacity(expected_cursor_pages + 1);
            let mut offset = 0_i64;
            loop {
                if pages.len() >= expected_cursor_pages {
                    return Err(eyre!(
                        "{entrypoint} cursor walk did not terminate within \
                         {expected_cursor_pages} pages"
                    ));
                }
                let page = invoke_typed_core_query_page(
                    peer.client(),
                    &contract_address,
                    entrypoint,
                    offset,
                    CURSOR_PAGE_LIMIT,
                )
                .await?;
                let (_, next_offset) = typed_query_page_parts(&page, view_name)?;
                pages.push(page);
                let Some(next_offset) = next_offset else {
                    break;
                };
                if next_offset <= offset {
                    return Err(eyre!(
                        "{entrypoint} cursor did not advance strictly: \
                         current={offset}, next={next_offset}"
                    ));
                }
                offset = next_offset;
            }
            pages.push(
                invoke_typed_core_query_page(peer.client(), &contract_address, entrypoint, 0, 64)
                    .await?,
            );
            family_pages.push(pages);
        }
        peer_results.push(family_pages);
    }
    assert!(
        peer_results.windows(2).all(|pair| pair[0] == pair[1]),
        "typed page projections for the five core entity families differ across voting peers: \
         {peer_results:?}"
    );
    let canonical_families = peer_results
        .first()
        .ok_or_else(|| eyre!("four-peer fixture returned no peer results"))?;
    for (family_index, (entrypoint, view_name, expected_ids, expected_fields)) in
        families.iter().enumerate()
    {
        let pages = &canonical_families[family_index];
        let (all_page, cursor_pages) = pages
            .split_last()
            .ok_or_else(|| eyre!("{entrypoint} returned no typed pages"))?;
        let mut walked_ids = Vec::with_capacity(expected_ids.len());
        let mut expected_offset = 0_usize;
        for (page_index, page) in cursor_pages.iter().enumerate() {
            let (page_ids, next_offset) = typed_query_page_parts(page, view_name)?;
            let expected_end = expected_offset
                .saturating_add(
                    usize::try_from(CURSOR_PAGE_LIMIT).expect("positive cursor page limit"),
                )
                .min(expected_ids.len());
            assert_eq!(
                page_ids.as_slice(),
                &expected_ids[expected_offset..expected_end],
                "{entrypoint} cursor page {page_index} must preserve canonical ID order"
            );
            let expected_next = (expected_end < expected_ids.len())
                .then(|| i64::try_from(expected_end).expect("fixture length fits in i64"));
            assert_eq!(
                next_offset, expected_next,
                "{entrypoint} cursor page {page_index} must return its exact continuation"
            );
            assert_typed_query_projection(page, view_name, expected_fields)?;
            walked_ids.extend(page_ids);
            expected_offset = expected_end;
        }
        assert_eq!(
            walked_ids.as_slice(),
            expected_ids.as_slice(),
            "{entrypoint} cursor walk must return every canonical ID exactly once"
        );
        let (all_ids, all_next) = typed_query_page_parts(all_page, view_name)?;
        assert_eq!(
            &all_ids, expected_ids,
            "{entrypoint} maximum bounded page must include the complete fixture"
        );
        assert_eq!(
            all_next, None,
            "{entrypoint} final bounded page must return Option::none"
        );
        assert_typed_query_projection(all_page, view_name, expected_fields)?;
    }
    const INVALID_PAGINATION_BOUNDS: [(&str, &str, &str, &str, &str); 8] = [
        (
            "negative offset",
            "-1",
            "1",
            "DecodeError",
            "instruction decode error",
        ),
        (
            "negative limit",
            "0",
            "-1",
            "AssertionFailed",
            "assertion failed (constraint violation)",
        ),
        (
            "offset-plus-limit overflow",
            "9223372036854775807",
            "1",
            "DecodeError",
            "instruction decode error",
        ),
        (
            "zero limit",
            "0",
            "0",
            "DecodeError",
            "instruction decode error",
        ),
        (
            "limit above the maximum",
            "0",
            "65",
            "DecodeError",
            "instruction decode error",
        ),
        (
            "offset above the signed host range",
            "9223372036854775808",
            "1",
            "AssertionFailed",
            "assertion failed (constraint violation)",
        ),
        (
            "offset above the unsigned host range",
            "18446744073709551616",
            "1",
            "AssertionFailed",
            "assertion failed (constraint violation)",
        ),
        (
            "limit above the unsigned host range",
            "0",
            "18446744073709551616",
            "AssertionFailed",
            "assertion failed (constraint violation)",
        ),
    ];
    for (entrypoint, _, _, _) in &families {
        let entrypoint = *entrypoint;
        for &(bound_class, offset, limit, expected_trap, expected_message) in
            &INVALID_PAGINATION_BOUNDS
        {
            let mut peer_rejections = Vec::with_capacity(network.peers().len());
            for peer in network.peers() {
                let peer_client = peer.client();
                let torii_url = peer_client.torii_url.clone();
                let (status, body) = post_typed_core_query_page(
                    &http,
                    &torii_url,
                    &contract_address,
                    entrypoint,
                    typed_core_query_page_payload_literals(offset, limit),
                )
                .await?;
                assert_eq!(
                    status,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "{entrypoint} {bound_class} must be a semantic rejection on every peer: \
                     offset={offset}, limit={limit}, body={body:?}"
                );
                assert_eq!(
                    body.get("ok").and_then(norito::json::Value::as_bool),
                    Some(false),
                    "{entrypoint} {bound_class} rejection must set ok=false: {body:?}"
                );
                let actual_entrypoint = body
                    .get("entrypoint")
                    .and_then(norito::json::Value::as_str)
                    .ok_or_else(|| {
                        eyre!("{entrypoint} {bound_class} rejection has no entrypoint: {body:?}")
                    })?
                    .to_owned();
                assert_eq!(actual_entrypoint, entrypoint);
                let actual_error = body
                    .get("error")
                    .and_then(norito::json::Value::as_str)
                    .ok_or_else(|| {
                        eyre!("{entrypoint} {bound_class} rejection has no error: {body:?}")
                    })?
                    .to_owned();
                assert_eq!(
                    actual_error,
                    format!("contract view execution failed: {expected_message}"),
                    "{entrypoint} {bound_class} returned the wrong semantic fault"
                );
                let diagnostic = body
                    .get("vm_diagnostic")
                    .and_then(norito::json::Value::as_object)
                    .ok_or_else(|| {
                        eyre!("{entrypoint} {bound_class} rejection has no VM diagnostic: {body:?}")
                    })?;
                let actual_trap = diagnostic
                    .get("trap_kind")
                    .and_then(norito::json::Value::as_str)
                    .ok_or_else(|| {
                        eyre!("{entrypoint} {bound_class} rejection has no trap kind: {body:?}")
                    })?
                    .to_owned();
                let actual_message = diagnostic
                    .get("message")
                    .and_then(norito::json::Value::as_str)
                    .ok_or_else(|| {
                        eyre!(
                            "{entrypoint} {bound_class} rejection has no diagnostic message: \
                             {body:?}"
                        )
                    })?
                    .to_owned();
                assert_eq!(actual_trap, expected_trap);
                assert_eq!(actual_message, expected_message);
                peer_rejections.push((
                    status.as_u16(),
                    actual_entrypoint,
                    actual_error,
                    actual_trap,
                    actual_message,
                ));
            }
            assert!(
                peer_rejections.windows(2).all(|pair| pair[0] == pair[1]),
                "{entrypoint} {bound_class} semantic rejection differs across voting peers: \
                 {peer_rejections:?}"
            );
        }
    }
    Ok(())
}
#[test]
fn contract_v1_executes_and_survives_four_peer_da_rbc_restart() -> Result<()> {
    run_contract_v1_four_peer_da_rbc_restart(
        false,
        stringify!(contract_v1_executes_and_survives_four_peer_da_rbc_restart),
    )
}

#[test]
fn contract_v1_genesis_registered_artifact_executes_and_survives_four_peer_da_rbc_restart()
-> Result<()> {
    run_contract_v1_four_peer_da_rbc_restart(
        true,
        stringify!(
            contract_v1_genesis_registered_artifact_executes_and_survives_four_peer_da_rbc_restart
        ),
    )
}

fn run_contract_v1_four_peer_da_rbc_restart(
    registered_in_genesis: bool,
    context: &'static str,
) -> Result<()> {
    // Genesis construction needs the same native test stack as the other
    // four-validator corridors. This does not change the IVM stack limit.
    const TEST_STACK_BYTES: usize = 64 * 1024 * 1024;
    let worker = std::thread::Builder::new()
        .name(context.to_owned())
        .stack_size(TEST_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(TEST_STACK_BYTES)
                .enable_all()
                .build()
                .expect("build four-validator contract runtime")
                .block_on(contract_v1_four_peer_da_rbc_restart_impl(
                    registered_in_genesis,
                    context,
                ))
        })
        .expect("spawn four-validator contract test");
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn contract_v1_four_peer_da_rbc_restart_impl(
    registered_in_genesis: bool,
    context: &'static str,
) -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let enact_permission: Permission = CanEnactGovernance.into();
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        // Leave one full signed cadence between full-mesh admission and the
        // first genesis QC wave on shared runners. This matches the existing
        // four-validator multi-QC integration envelope.
        .with_block_cadence(Duration::from_secs(8))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            // Nexus is selected by the canonical lane configuration; V1 has
            // no runtime `nexus.enabled` switch.
            layer
                .write(["nexus", "lane_count"], 1i64)
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                )
                .write(["logger", "level"], "INFO")
                .write(
                    ["logger", "filter"],
                    "iroha_core::sumeragi=trace,iroha_p2p=debug",
                )
                // This gate exercises consensus and mandatory DA/RBC, not
                // SoraNet admission-puzzle cost. Keep PoW enabled at the
                // production difficulty while bounding full-mesh startup work.
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1_i64,
                );
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            enact_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(contract_probe_alias_management_permission()),
            iroha_test_samples::ALICE_ID.clone(),
        ));
    // Genesis can register an artifact without an instance address. Its registrar signs the
    // manifest normally; deployment still waits for the final genesis-derived network identity.
    // This companion leaves the separate upload/register/Commit acceptance gate intact.
    let pre_registered_artifact = if registered_in_genesis {
        let artifact = contract_state_probe_artifact();
        builder = builder
            .with_genesis_keypair(iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone());
        for instruction in contract_probe_genesis_registration(&artifact)? {
            builder = builder.with_genesis_instruction(instruction);
        }
        Some(artifact)
    } else {
        None
    };
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");
    let handshake = signed_consensus_handshake(&network)?;
    handshake
        .validate()
        .map_err(|error| eyre!("invalid signed consensus handshake: {error}"))?;
    assert_eq!(
        handshake.mode,
        SumeragiConsensusMode::Npos,
        "contract V1 restart gate requires the Sora NPoS profile"
    );
    assert_eq!(
        handshake.sumeragi_v2.da_layout,
        recommended_data_availability_layout(),
        "contract V1 restart gate requires the signed mandatory DA layout"
    );
    let client = network.client();
    let http = integration_tests::http::client();
    network.ensure_blocks(1).await?;
    let code_bytes = pre_registered_artifact.unwrap_or_else(contract_state_probe_artifact);
    if registered_in_genesis {
        let verified = ivm::verify_contract_artifact(&code_bytes)
            .map_err(|error| eyre!("verify pre-registered probe: {error}"))?;
        let expected = verified
            .manifest
            .try_signed(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR)?;
        for peer in network.peers() {
            let actual = peer.client().query_single(
                iroha_data_model::query::smart_contract::FindContractManifestByCodeHash::new(
                    verified.code_hash,
                ),
            )?;
            assert_eq!(
                actual, expected,
                "genesis must retain the exact signed manifest on every validator"
            );
        }
    }
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "contract_state_probe",
        None,
        "universal",
    )
    .expect("contract alias");
    let (contract_address, _, _, deployment_tx_hash) = tokio::task::spawn_blocking({
        let client = client.clone();
        let code_bytes = code_bytes.clone();
        let contract_alias = contract_alias.clone();
        move || {
            deploy_contract_locally_signed_with_registration(
                &client,
                &code_bytes,
                contract_alias,
                registered_in_genesis,
                Some(&iroha_test_samples::ALICE_ID),
            )
        }
    })
    .await
    .expect("locally signed contract deployment task")?;
    let deployment_height = wait_for_tx_applied(
        &http,
        &client.torii_url,
        &hex::encode(deployment_tx_hash.as_ref()),
        Duration::from_secs(60),
        "contract V1 deployment",
    )
    .await?;
    network.ensure_blocks(deployment_height).await?;
    let deployment_hash = Hash::from(deployment_tx_hash);
    // Genesis has a global QC but bootstraps the lane catalog without a lane-block
    // certificate. The applied deployment is the first normal work whose certified
    // lane payload can establish this test's cross-peer RBC baseline.
    let deployment_rbc = wait_for_cross_peer_rbc_diagnostics(
        &network,
        Duration::from_secs(120),
        None,
        Some((deployment_height, &deployment_hash)),
    )
    .await?;
    // CommitContractDeployment already activates the address, binds its alias and stages
    // hajimari; its transaction also grants Alice the exact hook invocation token.
    // Pin the address/code in a trusted intent, then sign the SDK's exact quoted
    // transaction locally. No payload is sent for either zero-parameter hook.
    let mut verification_height = deployment_height;
    for (entrypoint, gas_limit) in [
        ("hajimari", 10_000),
        ("verify", CONTRACT_STATE_PROBE_GAS_LIMIT),
    ] {
        let intent = contract_probe_call_intent(
            &code_bytes,
            &contract_address,
            &contract_alias,
            entrypoint,
        )?;
        let response = tokio::task::spawn_blocking({
            let client = client.clone();
            let alias = contract_alias.clone();
            move || {
                client.post_contract_call_json(
                    &iroha_test_samples::ALICE_ID,
                    Some(iroha_test_samples::ALICE_KEYPAIR.private_key()),
                    None,
                    Some(&alias),
                    entrypoint,
                    None,
                    None,
                    None,
                    None,
                    &FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(gas_limit)),
                    &intent,
                )
            }
        })
        .await??;
        if response
            .get("submitted")
            .and_then(norito::json::Value::as_bool)
            != Some(true)
        {
            return Err(eyre!(
                "{entrypoint} was not locally signed and submitted: {response:?}"
            ));
        }
        let tx_hash = response
            .get("tx_hash_hex")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| {
                eyre!("{entrypoint} response is missing its exact transaction hash: {response:?}")
            })?;
        // Emit only the locally signed public identity returned by the SDK. This lets an
        // external observer follow calls without transaction bodies or binary call-site guesses.
        println!(
            "{}",
            contract_probe_call_observation(
                entrypoint,
                tx_hash,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
            )?,
        );
        verification_height = wait_for_tx_applied(
            &http,
            &client.torii_url,
            tx_hash,
            Duration::from_secs(60),
            entrypoint,
        )
        .await?;
    }
    // Applied is local to the submitting peer. Compare state only after every
    // validator has applied the exact verification transaction height.
    network.ensure_blocks(verification_height).await?;
    let expected_result = norito::json::Value::from("7".to_owned());
    for (index, peer) in network.peers().iter().enumerate() {
        wait_for_contract_applied_height(peer, verification_height, network.sync_timeout()).await?;
        let decoded =
            contract_view_json_value(peer.client(), &contract_address, "readback").await?;
        assert_eq!(
            decoded, expected_result,
            "peer {index} decoded a non-canonical contract V1 result"
        );
        let stored = contract_state_json_value(
            &http,
            &peer.client().torii_url,
            &contract_address,
            "probe_readback",
        )
        .await?;
        assert_eq!(
            stored, expected_result,
            "peer {index} read a divergent contract V1 state value"
        );
    }

    wait_for_cross_peer_rbc_diagnostics(
        &network,
        Duration::from_secs(120),
        Some(&deployment_rbc),
        None,
    )
    .await?;
    let restart_index = network.peers().len() - 1;
    let restart_peer = network.peers()[restart_index].clone();
    let config_layers = network.config_layers().collect::<Vec<_>>();
    let recovery_height = network
        .peers()
        .iter()
        .take(restart_index)
        .map(|peer| peer.client().get_status().map(|status| status.blocks))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .ok_or_else(|| eyre!("contract V1 restart gate has no healthy peer height"))?;
    assert!(
        restart_peer.shutdown_if_started().await,
        "selected contract V1 peer was not running before restart"
    );
    tokio::time::timeout(
        network.peer_startup_timeout(),
        restart_peer.start_checked(config_layers.iter(), None),
    )
    .await
    .map_err(|_| eyre!("contract V1 peer restart exceeded the startup timeout"))??;
    tokio::time::timeout(
        network.sync_timeout(),
        restart_peer.once_block(recovery_height),
    )
    .await
    .map_err(|_| {
        eyre!(
            "restarted contract V1 peer did not recover height {recovery_height} within {:?}",
            network.sync_timeout()
        )
    })?;
    // A durable block journal can satisfy once_block before replay has rebuilt world state.
    // The authoritative status endpoint must confirm applied recovery before contract reads.
    wait_for_contract_applied_height(&restart_peer, recovery_height, network.sync_timeout())
        .await?;
    let restarted_result =
        contract_view_json_value(restart_peer.client(), &contract_address, "readback").await?;
    assert_eq!(
        restarted_result, expected_result,
        "restarted peer decoded a different contract V1 result"
    );
    let restarted_state = contract_state_json_value(
        &http,
        &restart_peer.client().torii_url,
        &contract_address,
        "probe_readback",
    )
    .await?;
    assert_eq!(
        restarted_state, expected_result,
        "restarted peer did not recover the contract V1 state value"
    );
    Ok(())
}
