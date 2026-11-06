//! Pending four-validator governed activation, contract callbacks and native verification.
//! Requires current anonymous-ballot E2/E3 integration and B6 qualification;
//! no node-config activation or retired timed-OVN/TLE fallback is permitted.

use eyre::{Result, ensure, eyre};
use integration_tests::{manifest::ManifestEncodingBudget, sandbox};
use iroha::{blocking::Client, data_model::prelude::*};
use iroha_data_model::{
    block::{consensus::SumeragiRootScope, decode_framed_signed_block},
    isi::smart_contract_code::{
        CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
        SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
    },
    smart_contract::{ContractAddress, ContractAlias, ContractArtifactId},
    transaction::{
        TransactionBuilder,
        executable::{ContractInvocation, ExecutableBatchItem},
    },
    trigger::action::TimeTriggerRetryPolicy,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    smart_contract::{CanManageSmartContractCode, CanUseContractPermission},
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_network::{Network, NetworkBuilder};
use iroha_test_samples::ALICE_ID;
use iroha_version::codec::EncodeVersioned;
use norito::json::{self, Value};
use std::{
    num::{NonZeroU32, NonZeroU64},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Runtime;

/// TODO: replace this fail-closed prerequisite with the real current Parliament
/// approval, unanimous release support and committed effective height after E2/E3/B6.
fn governed_activation_height() -> Result<u64> {
    Err(eyre!(
        "governed lifecycle activation unavailable: anonymous-ballot E2 State/API integration, E3 four-validator qualification and B6 profile/security/resource qualification are incomplete; node-config and retired TLE activation are forbidden"
    ))
}
const CONTRACT: &str = r#"
seiyaku LifecycleLive {
    permission Run;
    error enum Failure { Rejected = 1, }
    state StateMap<int, int> Calls;
    kotoage fn unrelated() authorize(Run) { Calls[1] = Calls.get(1).unwrap_or(0) + 1; }
    kotoage fn callback() authorize(Run) { Calls[1] = Calls.get(1).unwrap_or(0) + 10; }
    kotoage fn reject() authorize(Run) { require(false, Failure::Rejected); }
    kotoage fn enable_retry() authorize(Run) { Calls[2] = 1; }
    kotoage fn retry_callback() authorize(Run) {
        require(Calls.get(2).unwrap_or(0) == 1, Failure::Rejected);
        Calls[1] = Calls.get(1).unwrap_or(0) + 10;
    }
}
"#;

fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000))
}
fn tip(client: &Client) -> Result<u64> {
    Ok(client.client().get_sumeragi_status()?.committed_height)
}
fn tick(client: &Client) -> Result<()> {
    client.submit(
        Log::new(
            Level::INFO,
            format!("lifecycle live tick {:?}", SystemTime::now()),
        ),
        fee(),
    )?;
    Ok(())
}
fn read_json(rt: &Runtime, peer: &iroha_test_network::NetworkPeer, path: &str) -> Result<String> {
    rt.block_on(async {
        let response = reqwest::Client::new()
            .get(format!("{}{path}", peer.torii_url().trim_end_matches('/')))
            .timeout(Duration::from_secs(60))
            .send()
            .await?
            .error_for_status()?;
        ensure!(
            response.content_length().unwrap_or(0) <= 32 * 1024 * 1024,
            "oversized lifecycle evidence"
        );
        Ok(response.text().await?)
    })
}
fn proof(
    client: &Client,
    height: u64,
) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
    client
        .client()
        .get_sumeragi_finality_proof(NonZeroU64::new(height).unwrap(), SumeragiRootScope::Global)
}
fn output_version(client: &Client, height: u64, v2: bool) -> Result<()> {
    let p = proof(client, height)?;
    let block = decode_framed_signed_block(&p.block_wire)?;
    ensure!(
        !block.execution_outputs().is_empty(),
        "actual nonempty block required"
    );
    ensure!(
        block
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_some() == v2),
        "wrong output version at {height}"
    );
    Ok(())
}
fn contract_artifact() -> Result<Vec<u8>> {
    kotodama_lang::compiler::Compiler::new()
        .compile_source(CONTRACT)
        .map_err(|e| eyre!("compile lifecycle contract: {e}"))
}

#[test]
fn live_callback_contract_compiles() -> Result<()> {
    iroha_test_network::init_instruction_registry();
    let artifact = contract_artifact()?;
    let verified =
        ivm::verify_contract_artifact(&artifact).map_err(|e| eyre!("verify contract: {e}"))?;
    let entries: std::collections::BTreeSet<_> = verified
        .manifest
        .entrypoints
        .as_ref()
        .ok_or_else(|| eyre!("lifecycle contract has no entrypoint manifest"))?
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    ensure!(
        entries
            == std::collections::BTreeSet::from([
                "callback",
                "enable_retry",
                "reject",
                "retry_callback",
                "unrelated",
            ]),
        "lifecycle contract entrypoints differ from the live calls: {entries:?}"
    );
    Ok(())
}

fn deploy(client: &Client) -> Result<(ContractAddress, iroha_crypto::Hash)> {
    let artifact = contract_artifact()?;
    let verified =
        ivm::verify_contract_artifact(&artifact).map_err(|e| eyre!("verify contract: {e}"))?;
    let encoding = ManifestEncodingBudget::new()?;
    let manifest = verified
        .manifest
        .try_signed(
            encoding.context(),
            encoding.max_frame_bytes(),
            client.client().key_pair(),
        )
        .map_err(|e| eyre!("sign manifest: {e}"))?;
    let artifact_id = ContractArtifactId::new(DataSpaceId::UNIVERSAL, verified.code_hash);
    let chunks = artifact.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES) as u32;
    for (index, chunk) in artifact.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
        client.submit(
            UploadSmartContractCodeChunk {
                artifact_id,
                total_size: artifact.len() as u64,
                chunk_index: index as u32,
                chunk_count: chunks,
                chunk: chunk.to_vec(),
            },
            fee(),
        )?;
    }
    client.submit_all(
        [
            InstructionBox::from(FinalizeSmartContractCodeUpload {
                artifact_id,
                total_size: artifact.len() as u64,
                chunk_count: chunks,
            }),
            InstructionBox::from(RegisterSmartContractCode {
                artifact_id,
                manifest,
            }),
        ],
        fee(),
    )?;
    let address = ContractAddress::derive(
        client.client().network_id(),
        client.client().account(),
        0,
        DataSpaceId::UNIVERSAL,
    )?;
    client.submit_all(
        [
            InstructionBox::from(CommitContractDeployment {
                expected_deploy_nonce: 0,
                contract_address: address.clone(),
                code_hash: verified.code_hash,
                contract_alias: ContractAlias::from_components(
                    "lifecycle_live",
                    None,
                    "universal",
                )?,
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            }),
            InstructionBox::from(Grant::account_permission(
                Permission::from(CanUseContractPermission {
                    contract: address.clone(),
                    permission: "Run".parse()?,
                }),
                client.client().account().clone(),
            )),
        ],
        fee(),
    )?;
    Ok((address, verified.code_hash))
}
fn call(client: &Client, invocation: ContractInvocation) -> Result<()> {
    let tx = TransactionBuilder::new(
        *client.client().network_id(),
        client.client().account().clone(),
        fee(),
    )
    .with_executable(Executable::ContractCall(invocation))
    .sign(client.client().key_pair().private_key());
    client.submit_transaction_and_wait(&tx)?;
    Ok(())
}

fn verify_python(
    network: &Network,
    rt: &Runtime,
    client: &Client,
    original: &SignedTransaction,
    id: &str,
    registration: u64,
    end: u64,
    expected: &str,
    expected_attempts: usize,
) -> Result<()> {
    rt.block_on(network.ensure_blocks(end))?;
    // Read original archives and proofs from the restarted seat itself.
    let reader = network.peers()[3].client();
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    std::fs::write(
        root.join("genesis.wire"),
        network.genesis().0.encode_wire()?,
    )?;
    std::fs::write(root.join("registration.wire"), original.encode_versioned())?;
    for height in 1..registration {
        std::fs::write(
            root.join(format!("prefix-{height}.norito")),
            norito::to_bytes(&proof(&reader, height)?)?,
        )?;
    }
    let proofs: Vec<_> = (registration - 1..=end)
        .map(|height| proof(&reader, height))
        .collect::<Result<_>>()?;
    std::fs::write(root.join("proofs.json"), json::to_json(&proofs)?)?;
    std::fs::write(
        root.join("witness.json"),
        read_json(
            rt,
            &network.peers()[3],
            &format!("/v1/triggers/lifecycle-witness?start_height={registration}&end_height={end}"),
        )?,
    )?;
    std::fs::write(
        root.join("activation.json"),
        read_json(rt, &network.peers()[3], "/v1/triggers/lifecycle-activation")?,
    )?;
    let network_id = client.client().network_id();
    let chain = client.account_client().chain_id().to_string();
    let discriminant = iroha_config::parameters::defaults::common::chain_discriminant();
    std::fs::write(
        root.join("expected.json"),
        json::to_json(&norito::json!({
            "network_id": network_id, "chain": chain,
            "discriminant": discriminant, "trigger": id,
            "registration_height": registration, "terminal": expected,
            "attempts": expected_attempts
        }))?,
    )?;
    let python = std::env::var_os("IROHA_LIFECYCLE_TEST_PYTHON").ok_or_else(|| {
        eyre!(
            "set IROHA_LIFECYCLE_TEST_PYTHON to the Python executable with this clone's native SDK"
        )
    })?;
    let helper =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lifecycle_trace_activation/verify.py");
    let output = std::process::Command::new(python)
        .arg(helper)
        .arg(root)
        .output()?;
    ensure!(
        output.status.success(),
        "Python SDK live verification failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

#[test]
#[ignore = "requires genuine current Parliament anonymous-ballot E2/E3 integration and B6 qualification; no config or retired TLE activation"]
fn four_peer_governed_activation_contract_time_and_restart() -> Result<()> {
    let activation = governed_activation_height()?;
    iroha_test_network::init_instruction_registry();
    let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    fees.fee_sink_account_id = ALICE_ID.clone();
    fees.base_fee = "0".parse()?;
    fees.per_byte_fee = "0".parse()?;
    fees.per_instruction_fee = "0".parse()?;
    fees.per_gas_unit_fee = "0".parse()?;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_genesis_instruction(SetParameter::new(
            iroha_data_model::parameter::Parameter::Custom(fees.into_custom_parameter()?),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSmartContractCode),
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Alias(
                    iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                        "lifecycle_live@universal".parse()?,
                        DataSpaceId::UNIVERSAL,
                    ),
                ),
            }),
            ALICE_ID.clone(),
        ));
    let (network, rt) = sandbox::start_network_blocking_or_skip(builder, "lifecycle_trace_activation")?
        .ok_or_else(|| eyre!("four-peer lifecycle qualification requires real localhost sockets; skipping is not success"))?;
    let client = network.peers()[0].client();
    rt.block_on(network.ensure_blocks(1))?;
    let mut predecessor_fingerprint = None;
    for peer in network.peers() {
        let policy: Value =
            json::from_json(&read_json(&rt, peer, "/v1/triggers/lifecycle-activation")?)?;
        // A scheduled proposal has no committed boundary yet. The genuine
        // governance helper must later establish the exact activation receipt.
        ensure!(policy["v2_activation_height"].is_null());
        ensure!(policy["committed_height"].as_u64().unwrap() < activation);
        ensure!(policy["active"].as_bool() == Some(false));
        if let Some(expected) = &predecessor_fingerprint {
            ensure!(expected == &policy["consensus_config_fingerprint"]);
        } else {
            predecessor_fingerprint = Some(policy["consensus_config_fingerprint"].clone());
        }
    }
    tick(&client)?;
    output_version(&client, 2, false)?;
    // Keep this seat's pre-activation Kura, then cross activation with the remaining quorum.
    let restarted = &network.peers()[3];
    rt.block_on(restarted.shutdown());
    while tip(&client)? < activation {
        tick(&client)?;
    }
    output_version(&client, activation, true)?;
    let mut fingerprint = None;
    for peer in network.peers().iter().take(3) {
        let policy: Value =
            json::from_json(&read_json(&rt, peer, "/v1/triggers/lifecycle-activation")?)?;
        ensure!(policy["active"].as_bool() == Some(true));
        ensure!(policy["v2_activation_height"].as_u64() == Some(activation));
        if let Some(expected) = &fingerprint {
            ensure!(expected == &policy["consensus_config_fingerprint"]);
        } else {
            fingerprint = Some(policy["consensus_config_fingerprint"].clone());
        }
    }
    ensure!(fingerprint != predecessor_fingerprint);
    let layers: Vec<_> = network.config_layers_for_peer(restarted).collect();
    rt.block_on(restarted.start_checked(layers.iter(), None))?;
    rt.block_on(network.ensure_blocks(activation))?;
    let (address, code_hash) = deploy(&client)?;
    let invocation = |entrypoint: &str| ContractInvocation {
        contract_address: address.clone(),
        expected_code_hash: code_hash,
        entrypoint: entrypoint.into(),
        arguments: None,
    };
    let mut last_case = None;
    for (id, entrypoint, retry, terminal, count) in [
        ("live_applied", "callback", false, "applied", 1),
        ("live_retry", "retry_callback", true, "applied", 2),
        ("live_exhausted", "reject", false, "exhausted", 2),
    ] {
        let start = SystemTime::now().duration_since(UNIX_EPOCH)? + Duration::from_secs(15);
        let action = iroha_data_model::trigger::action::Action::new(
            Executable::Batch(
                vec![ExecutableBatchItem::ContractCall(invocation(entrypoint))].into(),
            ),
            Repeats::Exactly(1),
            client.client().account().clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                TimeSchedule::starting_at(start).with_period(Duration::from_secs(1)),
            )),
        )?
        .with_retry_policy(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(1).unwrap(),
            retry_after_ms: NonZeroU64::new(if retry { 30_000 } else { 2_000 }).unwrap(),
        })?;
        let original = TransactionBuilder::new(
            *client.client().network_id(),
            client.client().account().clone(),
            fee(),
        )
        .with_executable_batch([
            ExecutableBatchItem::ContractCall(invocation("unrelated")),
            ExecutableBatchItem::Instruction(
                Register::trigger(Trigger::new(id.parse()?, action)).into(),
            ),
        ])
        .sign(client.client().key_pair().private_key());
        let before = tip(&client)?;
        client.submit_transaction_and_wait(&original)?;
        let after = tip(&client)?;
        let registration = (before + 1..=after)
            .find(|height| {
                proof(&client, *height)
                    .ok()
                    .and_then(|p| decode_framed_signed_block(&p.block_wire).ok())
                    .is_some_and(|block| {
                        block
                            .network_entrypoints()
                            .any(|tx| tx.hash() == original.hash_as_entrypoint())
                    })
            })
            .ok_or_else(|| eyre!("registration missing from certified blocks"))?;
        call(&client, invocation("unrelated"))?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut seen = 0;
        let mut recovered = false;
        let mut scanned = registration;
        loop {
            ensure!(
                Instant::now() < deadline,
                "{id}: timed out waiting for actual Time outputs"
            );
            tick(&client)?;
            let end = tip(&client)?;
            for height in scanned + 1..=end {
                let p = proof(&client, height)?;
                let block = decode_framed_signed_block(&p.block_wire)?;
                for row in block.execution_outputs() {
                    let time = match row {
                        iroha_data_model::block::execution_output::ExecutionOutputV1::TimeWithLifecycle(row) => Some(&row.execution),
                        _ => None,
                    };
                    if let Some(time) = time {
                        if time.invocation.trigger.trigger_id.to_string() == id {
                            seen += 1;
                        }
                    }
                }
            }
            scanned = end;
            if retry && seen == 1 && !recovered {
                // Recover immediately; verifying a frozen prefix must not consume
                // the retry window on a loaded debug validator machine.
                call(&client, invocation("enable_retry"))?;
                recovered = true;
                verify_python(
                    &network,
                    &rt,
                    &client,
                    &original,
                    id,
                    registration,
                    end,
                    "retrying",
                    1,
                )?;
            }
            if seen >= count {
                verify_python(
                    &network,
                    &rt,
                    &client,
                    &original,
                    id,
                    registration,
                    end,
                    terminal,
                    count,
                )?;
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        last_case = Some((original, id, registration, count));
    }
    // Restart again with stored V2 State/history, and verify the same policy and continued rows.
    rt.block_on(restarted.shutdown());
    rt.block_on(restarted.start_checked(layers.iter(), None))?;
    tick(&client)?;
    rt.block_on(network.ensure_blocks(tip(&client)?))?;
    let policy: Value = json::from_json(&read_json(
        &rt,
        restarted,
        "/v1/triggers/lifecycle-activation",
    )?)?;
    ensure!(policy["active"].as_bool() == Some(true));
    ensure!(Some(policy["consensus_config_fingerprint"].clone()) == fingerprint);
    output_version(&restarted.client(), tip(&client)?, true)?;
    let (original, id, registration, count) = last_case.expect("three executed cases");
    verify_python(
        &network,
        &rt,
        &client,
        &original,
        id,
        registration,
        tip(&client)?,
        "exhausted",
        count,
    )?;
    eprintln!(
        "LIFECYCLE_LOCALNET_OK peers=4 activation={activation} applied=1 retry=2 exhausted=2 restart=2 python_verified=true"
    );
    Ok(())
}

#[test]
fn governed_activation_prerequisite_refuses_unqualified_corridor() {
    let error = governed_activation_height().unwrap_err().to_string();
    assert!(error.contains("E2 State/API integration"));
    assert!(error.contains("E3 four-validator qualification"));
    assert!(error.contains("B6"));
}
