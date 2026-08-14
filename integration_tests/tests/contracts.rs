#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii contract manifest endpoints: bytecode deploy wraps ISIs and GET reads the derived on-chain manifest.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha::data_model::prelude::*;
use iroha::data_model::{
    block::{
        consensus::{SumeragiCommittedLaneBlock, committed_lane_block_status_counts_as_progress},
        consensus_v2::SumeragiV2GenesisContextParameters,
    },
    parameter::system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
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
            return_type: None,
            return_schema: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut out = meta.encode();
    out.extend_from_slice(&interface.encode_section());
    out.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    out
}
fn contract_state_probe_artifact() -> Vec<u8> {
    let src = r#"
seiyaku ContractStateProbe {
  error enum ProbeError {
    NotInitialized = 1
  }

  state int Initialized;
  state int StoredValue;
  state int probe_readback;

  kotoage fn main() -> int authorize("CanEnactGovernance") {
    return 0;
  }

  fn initialize_impl() {
    Initialized = 1;
    StoredValue = 7;
    probe_readback = 0;
  }

  hajimari() {
    initialize_impl();
  }

  kotoage fn verify() authorize("CanEnactGovernance") {
    require(Initialized == 1, ProbeError::NotInitialized);
    probe_readback = StoredValue;
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract-state probe program")
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
    bump_hidden(key, delta);
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
            "typed-query genesis must contain exactly one signed consensus handshake; found {}",
            handshakes.len()
        ));
    }
    let custom = handshakes
        .pop()
        .expect("length checked before reading consensus handshake");
    norito::json::from_str(custom.payload().get())
        .map_err(|error| eyre!("decode signed consensus handshake metadata: {error}"))
}
async fn wait_for_cross_peer_rbc_diagnostics(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
    after: Option<&SumeragiCommittedLaneBlock>,
    required_transaction: Option<(u64, &Hash)>,
) -> Result<SumeragiCommittedLaneBlock> {
    let expected_validator_count = u32::try_from(network.peers().len())
        .map_err(|_| eyre!("peer count does not fit in u32"))?;
    let expected_min_quorum = u32::try_from(commit_quorum_from_len(network.peers().len()).max(1))
        .map_err(|_| eyre!("commit quorum does not fit in u32"))?;
    let mut expected_validator_set = network
        .peers()
        .iter()
        .map(|peer| peer.id())
        .collect::<Vec<_>>();
    expected_validator_set.sort();
    let zero_hash = Hash::prehashed([0; Hash::LENGTH]);
    let deadline = Instant::now() + timeout;
    loop {
        let tasks = network
            .peers()
            .iter()
            .map(|peer| {
                let client = peer.client();
                tokio::task::spawn_blocking(move || client.get_sumeragi_diagnostics())
            })
            .collect::<Vec<_>>();
        let mut observations = Vec::with_capacity(tasks.len());
        let mut errors = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(diagnostics)) if diagnostics.npos.is_some() => {
                    let ownerships = diagnostics.lane_payload_ownerships;
                    let evidence = diagnostics
                        .committed_lane_blocks
                        .into_iter()
                        .filter(|record| {
                            let matching_ownership = ownerships.iter().find(|ownership| {
                                ownership.validate_replay_material().is_ok()
                                    && ownership.lane_id == record.lane_id
                                    && ownership.dataspace_id == record.dataspace_id
                                    && ownership.lane_incarnation == record.lane_incarnation
                                    && ownership.lane_block_height == record.lane_block_height
                                    && ownership.lane_block_view == record.lane_block_view
                                    && ownership.lane_block_descriptor_hash
                                        == Some(record.descriptor_hash)
                                    && ownership.subject_hash == record.subject_hash
                                    && ownership.payload_ownership_hash
                                        == record.payload_ownership_hash
                                    && ownership.rbc_instance_hash == record.rbc_instance_hash
                                    && ownership.qc_mode_tag == record.qc_mode_tag
                                    && ownership.lane_block_descriptor_validator_count
                                        == record.validator_count
                                    && ownership.lane_block_descriptor_min_quorum
                                        == record.min_quorum
                                    && ownership.lane_block_descriptor_validator_count
                                        == expected_validator_count
                                    && ownership.lane_block_descriptor_min_quorum
                                        == expected_min_quorum
                                    && ownership.lane_block_descriptor_validator_set.as_slice()
                                        == expected_validator_set.as_slice()
                            });
                            after.is_none_or(|baseline| {
                                record.lane_id == baseline.lane_id
                                    && record.dataspace_id == baseline.dataspace_id
                                    && record.lane_incarnation == baseline.lane_incarnation
                                    && (record.lane_block_height, record.lane_block_view)
                                        > (baseline.lane_block_height, baseline.lane_block_view)
                            }) && matching_ownership.is_some_and(|ownership| {
                                required_transaction.is_none_or(
                                    |(proposal_height, transaction_hash)| {
                                        ownership.proposal_height == proposal_height
                                            && ownership
                                                .accepted_transaction_hashes
                                                .contains(transaction_hash)
                                    },
                                )
                            }) && record.executable_payload_available
                                && committed_lane_block_status_counts_as_progress(
                                    &record.execution_status,
                                    record.executable_payload_available,
                                )
                                && record.validator_count == expected_validator_count
                                && record.min_quorum == expected_min_quorum
                                && record.prepare_qc_signer_count >= record.min_quorum
                                && record.prepare_qc_signer_count <= record.validator_count
                                && record.commit_qc_signer_count >= record.min_quorum
                                && record.commit_qc_signer_count <= record.validator_count
                                && record.descriptor_hash != zero_hash
                                && record.proposal_hash != zero_hash
                                && record.subject_hash != zero_hash
                                && record.payload_ownership_hash != zero_hash
                                && record.rbc_instance_hash != zero_hash
                        })
                        .max_by_key(|record| (record.lane_block_height, record.lane_block_view));
                    observations.push(evidence);
                }
                Ok(Ok(_)) => {
                    errors.push("peer diagnostics did not expose NPoS state".to_owned());
                    observations.push(None);
                }
                Ok(Err(error)) => {
                    errors.push(error.to_string());
                    observations.push(None);
                }
                Err(error) => {
                    errors.push(format!("diagnostics task failed: {error}"));
                    observations.push(None);
                }
            }
        }
        if let Some(first) = observations.first().and_then(Option::as_ref)
            && observations
                .iter()
                .all(|observation| observation.as_ref() == Some(first))
        {
            return Ok(first.clone());
        }
        let last_observed = format!("evidence={observations:?}; errors={errors:?}");
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for identical four-peer certified RBC diagnostics; \
                 last_observed={last_observed}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
fn dynamic_counter_args(key: i64, delta: i64) -> norito::json::Value {
    norito::json::object([
        ("key", norito::json::Value::from(key.to_string())),
        ("delta", norito::json::Value::from(delta.to_string())),
    ])
    .expect("serialize dynamic counter arguments")
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
    use iroha_data_model::isi::smart_contract_code::{
        CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
        SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
    };
    let verified = ivm::verify_contract_artifact(artifact)
        .map_err(|error| eyre!("verify contract artifact: {error}"))?;
    let manifest = verified
        .manifest
        .try_signed(&client.key_pair)
        .map_err(|error| eyre!("sign contract manifest locally: {error}"))?;
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
    let deployment_tx_hash = client.submit_blocking_with_metadata(
        CommitContractDeployment {
            expected_deploy_nonce: deploy_nonce,
            contract_address: contract_address.clone(),
            code_hash: verified.code_hash,
            contract_alias,
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        },
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
    network.ensure_blocks(deploy_height).await?;
    let alice_submission = tokio::task::spawn_blocking({
        let client = alice_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 3);
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::ALICE_ID.clone(),
                Some(iroha_test_samples::ALICE_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_direct",
                Some(&payload),
                None,
                &FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100_000)),
            )
        }
    });
    let bob_submission = tokio::task::spawn_blocking({
        let client = bob_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 5);
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::BOB_ID.clone(),
                Some(iroha_test_samples::BOB_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_via_helper",
                Some(&payload),
                None,
                &FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100_000)),
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
                .write(["nexus", "enabled"], true)
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
        SumeragiV2GenesisContextParameters::recommended().da_layout,
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
#[tokio::test]
async fn contract_state_survives_across_calls_in_sora_profile_network() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let enact_permission: Permission = CanEnactGovernance.into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(4))
        .with_npos_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 1i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            enact_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let context = stringify!(contract_state_survives_across_calls_in_sora_profile_network);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");
    let client = network.client();
    let http = integration_tests::http::client();
    network.ensure_blocks(1).await?;
    let code_bytes = contract_state_probe_artifact();
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "contract_state_probe",
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
    let pk = iroha_data_model::prelude::ExposedPrivateKey(
        iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
    );
    let authority_literal = iroha_test_samples::ALICE_ID.to_string();
    let activate_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "code_hash",
            norito::json::to_value(&code_hash_hex).expect("serialize code hash"),
        ),
    ])
    .expect("serialize activate body");
    let activate_baseline = client.get_status()?.txs_approved;
    let activate_resp = http
        .post(client.torii_url.join("/v1/contracts/instance/activate")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&activate_body)?)
        .send()
        .await?;
    if !activate_resp.status().is_success() {
        let status = activate_resp.status();
        let body = activate_resp.text().await.unwrap_or_default();
        return Err(eyre!("activate returned {status}: {body}"));
    }
    let activate_payload: norito::json::Value =
        norito::json::from_str(&activate_resp.text().await?)?;
    if activate_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "activate produced unsigned scaffold instead of submitting: {activate_payload:?}"
        ));
    }
    if let Some(activate_tx_hash) = activate_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            activate_tx_hash,
            Duration::from_secs(30),
            "activate",
        )
        .await?;
    } else {
        wait_for_approved_txs(
            &client,
            activate_baseline,
            Duration::from_secs(30),
            "activate",
        )
        .await?;
    }
    let hajimari_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "entrypoint",
            norito::json::to_value("hajimari").expect("serialize entrypoint"),
        ),
        (
            "fee_payment",
            norito::json::to_value(&FeePaymentIntent::authority(
                Vec::new(),
                NonZeroU64::new(10_000),
            ))
            .expect("serialize fee payment intent"),
        ),
    ])
    .expect("serialize hajimari body");
    let hajimari_baseline = client.get_status()?.txs_approved;
    let hajimari_resp = http
        .post(client.torii_url.join("/v1/contracts/call")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&hajimari_body)?)
        .send()
        .await?;
    if !hajimari_resp.status().is_success() {
        let status = hajimari_resp.status();
        let body = hajimari_resp.text().await.unwrap_or_default();
        return Err(eyre!("hajimari returned {status}: {body}"));
    }
    let hajimari_payload: norito::json::Value =
        norito::json::from_str(&hajimari_resp.text().await?)?;
    if hajimari_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "hajimari produced unsigned scaffold instead of submitting: {hajimari_payload:?}"
        ));
    }
    if let Some(hajimari_tx_hash) = hajimari_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            hajimari_tx_hash,
            Duration::from_secs(30),
            "hajimari",
        )
        .await?;
    } else {
        wait_for_approved_txs(
            &client,
            hajimari_baseline,
            Duration::from_secs(30),
            "hajimari",
        )
        .await?;
    }
    let verify_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "entrypoint",
            norito::json::to_value("verify").expect("serialize entrypoint"),
        ),
        (
            "fee_payment",
            norito::json::to_value(&FeePaymentIntent::authority(
                Vec::new(),
                NonZeroU64::new(10_000),
            ))
            .expect("serialize fee payment intent"),
        ),
    ])
    .expect("serialize verify body");
    let verify_baseline = client.get_status()?.txs_approved;
    let verify_resp = http
        .post(client.torii_url.join("/v1/contracts/call")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&verify_body)?)
        .send()
        .await?;
    if !verify_resp.status().is_success() {
        let status = verify_resp.status();
        let body = verify_resp.text().await.unwrap_or_default();
        return Err(eyre!("verify returned {status}: {body}"));
    }
    let verify_payload: norito::json::Value = norito::json::from_str(&verify_resp.text().await?)?;
    if verify_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "verify produced unsigned scaffold instead of submitting: {verify_payload:?}"
        ));
    }
    if let Some(verify_tx_hash) = verify_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            verify_tx_hash,
            Duration::from_secs(30),
            "verify",
        )
        .await?;
    } else {
        wait_for_approved_txs(&client, verify_baseline, Duration::from_secs(30), "verify").await?;
    }
    let mut state_url = client.torii_url.join("/v1/contracts/state")?;
    state_url
        .query_pairs_mut()
        .append_pair("path", "probe_readback");
    let state_resp = http
        .get(state_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    if !state_resp.status().is_success() {
        let status = state_resp.status();
        let body = state_resp.text().await.unwrap_or_default();
        return Err(eyre!("state query returned {status}: {body}"));
    }
    let state_payload: norito::json::Value = norito::json::from_str(&state_resp.text().await?)?;
    let found = state_payload
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("found"))
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    assert!(
        found,
        "probe_readback should be persisted: {state_payload:?}"
    );
    Ok(())
}
