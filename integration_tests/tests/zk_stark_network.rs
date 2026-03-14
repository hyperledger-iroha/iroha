//! Multi-peer STARK integration coverage for governance voting and shielded IVM admission.
#![cfg(feature = "zk-stark")]

use std::{str::FromStr as _, time::Duration};

use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{
    isi::{Grant, InstructionBox},
    metadata::Metadata,
    name::Name,
    permission::Permission,
    proof::{
        ProofAttachment, ProofAttachmentList, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord,
    },
    transaction::{Executable, IvmBytecode, IvmProved, signed::TransactionBuilder},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanManageParliament, CanSubmitGovernanceBallot,
};
use iroha_primitives::json::Json;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::ALICE_ID;
use reqwest::{Client as HttpClient, StatusCode};

#[derive(norito::JsonSerialize)]
struct ZkIvmDeriveRequest {
    vk_ref: VerifyingKeyId,
    authority: iroha_data_model::account::AccountId,
    metadata: Metadata,
    bytecode: IvmBytecode,
}

#[derive(norito::JsonDeserialize)]
struct ZkIvmDeriveResponse {
    proved: IvmProved,
}

#[derive(norito::JsonSerialize)]
struct ZkIvmProveRequest {
    vk_ref: VerifyingKeyId,
    authority: iroha_data_model::account::AccountId,
    metadata: Metadata,
    bytecode: IvmBytecode,
    proved: Option<IvmProved>,
}

#[derive(norito::JsonDeserialize)]
struct ZkIvmProveJobCreated {
    job_id: String,
}

#[derive(norito::JsonDeserialize)]
struct ZkIvmProveJob {
    status: String,
    error: Option<String>,
    attachment: Option<ProofAttachment>,
}

fn has_test_network_feature(feature: &str) -> bool {
    std::env::var("TEST_NETWORK_IROHAD_FEATURES")
        .ok()
        .map(|value| {
            value
                .split([',', ' ', '\t', '\n'])
                .any(|item| item.trim() == feature)
        })
        .unwrap_or(false)
}

fn require_test_network_feature(feature: &str, test_name: &str) -> Result<()> {
    ensure!(
        has_test_network_feature(feature),
        "{test_name}: TEST_NETWORK_IROHAD_FEATURES must include `{feature}` to execute the runtime path"
    );
    Ok(())
}

fn sample_stark_vk_box(backend: &str, circuit_id: &str) -> VerifyingKeyBox {
    let vk_payload = iroha_core::zk_stark::StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: circuit_id.to_owned(),
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: iroha_core::zk_stark::STARK_HASH_SHA256_V1,
    };
    let bytes = norito::to_bytes(&vk_payload).expect("encode stark vk payload");
    VerifyingKeyBox::new(backend.to_owned(), bytes)
}

fn limb_as_instance_bytes(limb: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&limb.to_le_bytes());
    out
}

fn derive_ballot_nullifier(
    domain_tag: &str,
    chain_id: &iroha_data_model::ChainId,
    election_id: &str,
    commit: &[u8; 32],
) -> [u8; 32] {
    let mut input = Vec::with_capacity(
        domain_tag.len() + chain_id.as_str().len() + election_id.len() + commit.len() + 24,
    );
    let push_len = |buf: &mut Vec<u8>, len: usize| {
        let len_u64 = len as u64;
        buf.extend_from_slice(&len_u64.to_le_bytes());
    };
    push_len(&mut input, domain_tag.len());
    input.extend_from_slice(domain_tag.as_bytes());
    push_len(&mut input, chain_id.as_str().len());
    input.extend_from_slice(chain_id.as_str().as_bytes());
    push_len(&mut input, election_id.len());
    input.extend_from_slice(election_id.as_bytes());
    input.extend_from_slice(commit);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

async fn submit_and_wait_next_block<I>(
    client: &Client,
    network: &sandbox::SerializedNetwork,
    instruction: I,
    expected_height: &mut u64,
    context: &str,
) -> Result<()>
where
    I: Into<InstructionBox>,
{
    client
        .submit_with_metadata(instruction, Metadata::default())
        .wrap_err_with(|| format!("submit {context}"))?;
    *expected_height = expected_height.saturating_add(1);
    network
        .ensure_blocks(*expected_height)
        .await
        .wrap_err_with(|| format!("wait for commit after {context}"))?;
    Ok(())
}

fn torii_v2_url(client: &Client, segments: &[&str]) -> reqwest::Url {
    let mut url = client.torii_url.clone();
    let mut path_segments = url
        .path_segments_mut()
        .expect("torii_url must be a base URL");
    path_segments.clear();
    path_segments.extend(segments.iter().copied());
    drop(path_segments);
    url
}

async fn post_torii_json_v2(
    client: &Client,
    segments: &[&str],
    payload: &norito::json::Value,
    context: &str,
) -> Result<norito::json::Value> {
    let url = torii_v2_url(client, segments);
    let body = norito::json::to_vec(payload)?;
    let response = HttpClient::builder()
        .timeout(Duration::from_secs(20))
        .build()?
        .post(url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;
    let status = response.status();
    let bytes = response.bytes().await?.to_vec();
    if status != StatusCode::OK {
        let body = String::from_utf8_lossy(&bytes);
        return Err(eyre!("{context}: HTTP {status}. {body}"));
    }
    Ok(norito::json::from_slice(&bytes)?)
}

async fn get_torii_json_v2(
    client: &Client,
    segments: &[&str],
    context: &str,
) -> Result<norito::json::Value> {
    let url = torii_v2_url(client, segments);
    let response = HttpClient::builder()
        .timeout(Duration::from_secs(20))
        .build()?
        .get(url)
        .send()
        .await?;
    let status = response.status();
    let bytes = response.bytes().await?.to_vec();
    if status != StatusCode::OK {
        let body = String::from_utf8_lossy(&bytes);
        return Err(eyre!("{context}: HTTP {status}. {body}"));
    }
    Ok(norito::json::from_slice(&bytes)?)
}

async fn wait_for_prove_attachment(client: &Client, job_id: &str) -> Result<ProofAttachment> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let value = get_torii_json_v2(
            client,
            &["v2", "zk", "ivm", "prove", job_id],
            "fetch /v2/zk/ivm/prove/{job_id}",
        )
        .await?;
        let dto: ZkIvmProveJob =
            norito::json::from_value(value).wrap_err("decode prove job dto")?;
        match dto.status.as_str() {
            "pending" | "running" => {}
            "done" => {
                return dto
                    .attachment
                    .ok_or_else(|| eyre!("prove job completed without attachment"));
            }
            "error" => {
                let message = dto
                    .error
                    .unwrap_or_else(|| "unknown prove error".to_owned());
                return Err(eyre!("prove job failed: {message}"));
            }
            other => return Err(eyre!("unexpected prove job status: {other}")),
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(eyre!("timed out waiting for prove job completion"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test]
async fn stark_governance_and_shielded_ivm_paths() -> Result<()> {
    require_test_network_feature(
        "zk-stark",
        stringify!(stark_governance_and_shielded_ivm_paths),
    )?;

    let backend = "stark/fri";
    let ivm_circuit_id = "ivm-execution";
    let ballot_circuit_id = "vote-ballot";
    let tally_circuit_id = "vote-tally";

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["zk", "stark", "enabled"], true)
                .write(["pipeline", "ivm_proved", "enabled"], true)
                .write(
                    ["pipeline", "ivm_proved", "allowed_circuits"],
                    vec![ivm_circuit_id.to_owned()],
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(stark_governance_and_shielded_ivm_paths),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let mut expected_height = 1_u64;

    let client = network.client();
    let election_id = "stark-vote-network-e2e".to_owned();

    submit_and_wait_next_block(
        &client,
        &network,
        Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".to_string(), Json::new(())),
            ALICE_ID.clone(),
        ),
        &mut expected_height,
        "grant CanManageVerifyingKeys",
    )
    .await?;
    submit_and_wait_next_block(
        &client,
        &network,
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone()),
        &mut expected_height,
        "grant CanManageParliament",
    )
    .await?;
    submit_and_wait_next_block(
        &client,
        &network,
        Grant::account_permission(
            Permission::from(CanSubmitGovernanceBallot {
                referendum_id: election_id.clone(),
            }),
            ALICE_ID.clone(),
        ),
        &mut expected_height,
        "grant CanSubmitGovernanceBallot",
    )
    .await?;
    submit_and_wait_next_block(
        &client,
        &network,
        Grant::account_permission(Permission::from(CanEnactGovernance), ALICE_ID.clone()),
        &mut expected_height,
        "grant CanEnactGovernance",
    )
    .await?;

    let ivm_vk_id = VerifyingKeyId::new(backend, "ivm_exec_stark");
    let ivm_vk_box = sample_stark_vk_box(backend, ivm_circuit_id);
    let mut ivm_vk_record = VerifyingKeyRecord::new(
        1,
        ivm_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
        iroha_core::zk::hash_vk(&ivm_vk_box),
    );
    ivm_vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    ivm_vk_record.gas_schedule_id = Some("sched_0".to_owned());
    ivm_vk_record.vk_len = ivm_vk_box.bytes.len() as u32;
    ivm_vk_record.max_proof_bytes = 8 * 1024 * 1024;
    ivm_vk_record.key = Some(ivm_vk_box.clone());
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: ivm_vk_id.clone(),
            record: ivm_vk_record,
        },
        &mut expected_height,
        "register ivm verifying key",
    )
    .await?;

    let ballot_vk_id = VerifyingKeyId::new(backend, "vote_ballot");
    let ballot_vk_box = sample_stark_vk_box(backend, ballot_circuit_id);
    let ballot_schema = b"gov:vote:ballot:schema:v1".to_vec();
    let mut ballot_vk_record = VerifyingKeyRecord::new(
        1,
        ballot_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        iroha_crypto::Hash::new(&ballot_schema).into(),
        iroha_core::zk::hash_vk(&ballot_vk_box),
    );
    ballot_vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    ballot_vk_record.gas_schedule_id = Some("sched_ballot".to_owned());
    ballot_vk_record.vk_len = ballot_vk_box.bytes.len() as u32;
    ballot_vk_record.max_proof_bytes = 8 * 1024 * 1024;
    ballot_vk_record.key = Some(ballot_vk_box.clone());
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: ballot_vk_id.clone(),
            record: ballot_vk_record,
        },
        &mut expected_height,
        "register ballot verifying key",
    )
    .await?;

    let tally_vk_id = VerifyingKeyId::new(backend, "vote_tally");
    let tally_vk_box = sample_stark_vk_box(backend, tally_circuit_id);
    let tally_schema = b"gov:vote:tally:schema:v1".to_vec();
    let mut tally_vk_record = VerifyingKeyRecord::new(
        1,
        tally_circuit_id,
        BackendTag::Stark,
        "goldilocks",
        iroha_crypto::Hash::new(&tally_schema).into(),
        iroha_core::zk::hash_vk(&tally_vk_box),
    );
    tally_vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    tally_vk_record.gas_schedule_id = Some("sched_tally".to_owned());
    tally_vk_record.vk_len = tally_vk_box.bytes.len() as u32;
    tally_vk_record.max_proof_bytes = 8 * 1024 * 1024;
    tally_vk_record.key = Some(tally_vk_box.clone());
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: tally_vk_id.clone(),
            record: tally_vk_record,
        },
        &mut expected_height,
        "register tally verifying key",
    )
    .await?;

    let nullifier_domain = "gov:ballot:v1".to_owned();
    let eligible_root = [0x22; 32];
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::zk::CreateElection {
            election_id: election_id.clone(),
            options: 2,
            eligible_root,
            start_ts: 0,
            end_ts: 0,
            vk_ballot: ballot_vk_id.clone(),
            vk_tally: tally_vk_id.clone(),
            domain_tag: nullifier_domain.clone(),
        },
        &mut expected_height,
        "create election",
    )
    .await?;

    let bad_commit = [0x33; 32];
    let mismatched_ballot_proof = iroha_core::zk::prove_stark_fri_open_verify_envelope(
        backend,
        tally_circuit_id,
        &tally_vk_box,
        &tally_schema,
        vec![vec![bad_commit], vec![eligible_root]],
    )
    .map_err(|err| eyre!(err))?;
    let mismatched_ballot_attachment = ProofAttachment::new_ref(
        backend.to_owned(),
        mismatched_ballot_proof,
        ballot_vk_id.clone(),
    );
    let mismatched_nullifier =
        derive_ballot_nullifier(&nullifier_domain, &client.chain, &election_id, &bad_commit);
    let bad_ballot = client.submit_blocking(iroha_data_model::isi::zk::SubmitBallot {
        election_id: election_id.clone(),
        ciphertext: bad_commit.to_vec(),
        ballot_proof: mismatched_ballot_attachment,
        nullifier: mismatched_nullifier,
    });
    assert!(
        bad_ballot.is_err(),
        "mismatched ballot circuit/backend binding should be rejected"
    );

    let commit = [0x11; 32];
    let ballot_proof = iroha_core::zk::prove_stark_fri_open_verify_envelope(
        backend,
        ballot_circuit_id,
        &ballot_vk_box,
        &ballot_schema,
        vec![vec![commit], vec![eligible_root]],
    )
    .map_err(|err| eyre!(err))?;
    let ballot_attachment =
        ProofAttachment::new_ref(backend.to_owned(), ballot_proof, ballot_vk_id.clone());
    let nullifier =
        derive_ballot_nullifier(&nullifier_domain, &client.chain, &election_id, &commit);
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::zk::SubmitBallot {
            election_id: election_id.clone(),
            ciphertext: commit.to_vec(),
            ballot_proof: ballot_attachment,
            nullifier,
        },
        &mut expected_height,
        "submit valid ballot",
    )
    .await?;

    let tally = vec![7_u64, 2_u64];
    let tally_columns = tally
        .iter()
        .map(|&value| vec![limb_as_instance_bytes(value)])
        .collect::<Vec<_>>();
    let tally_proof = iroha_core::zk::prove_stark_fri_open_verify_envelope(
        backend,
        tally_circuit_id,
        &tally_vk_box,
        &tally_schema,
        tally_columns,
    )
    .map_err(|err| eyre!(err))?;
    let tally_attachment = ProofAttachment::new_ref(backend.to_owned(), tally_proof, tally_vk_id);
    submit_and_wait_next_block(
        &client,
        &network,
        iroha_data_model::isi::zk::FinalizeElection {
            election_id,
            tally,
            tally_proof: tally_attachment,
        },
        &mut expected_height,
        "finalize election",
    )
    .await?;

    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let mut tx_meta = Metadata::default();
    tx_meta.insert(
        Name::from_str("gas_limit").expect("static gas_limit key"),
        Json::new(50_000_000_u64),
    );

    let derive_req = ZkIvmDeriveRequest {
        vk_ref: ivm_vk_id.clone(),
        authority: client.account.clone(),
        metadata: tx_meta.clone(),
        bytecode: bytecode.clone(),
    };
    let derive_req_json = norito::json::to_value(&derive_req)?;
    let derive_resp_json = post_torii_json_v2(
        &client,
        &["v2", "zk", "ivm", "derive"],
        &derive_req_json,
        "post /v2/zk/ivm/derive",
    )
    .await?;
    let derive_resp: ZkIvmDeriveResponse =
        norito::json::from_value(derive_resp_json).wrap_err("decode derive response")?;

    let prove_req = ZkIvmProveRequest {
        vk_ref: ivm_vk_id.clone(),
        authority: client.account.clone(),
        metadata: tx_meta.clone(),
        bytecode,
        proved: Some(derive_resp.proved.clone()),
    };
    let prove_req_json = norito::json::to_value(&prove_req)?;
    let prove_created_json = post_torii_json_v2(
        &client,
        &["v2", "zk", "ivm", "prove"],
        &prove_req_json,
        "post /v2/zk/ivm/prove",
    )
    .await?;
    let prove_created: ZkIvmProveJobCreated =
        norito::json::from_value(prove_created_json).wrap_err("decode prove created response")?;
    let attachment = wait_for_prove_attachment(&client, &prove_created.job_id).await?;

    let tx_valid = TransactionBuilder::new(client.chain.clone(), client.account.clone())
        .with_executable(Executable::IvmProved(derive_resp.proved.clone()))
        .with_metadata(tx_meta.clone())
        .with_attachments(ProofAttachmentList(vec![attachment.clone()]))
        .sign(client.key_pair.private_key());
    client
        .submit_transaction_blocking(&tx_valid)
        .wrap_err("submit valid STARK IvmProved tx")?;

    let bad_attachment =
        ProofAttachment::new_ref(backend.to_owned(), attachment.proof.clone(), ballot_vk_id);
    let tx_bad = TransactionBuilder::new(client.chain.clone(), client.account.clone())
        .with_executable(Executable::IvmProved(derive_resp.proved))
        .with_metadata(tx_meta)
        .with_attachments(ProofAttachmentList(vec![bad_attachment]))
        .sign(client.key_pair.private_key());
    let bad = client.submit_transaction_blocking(&tx_bad);
    assert!(
        bad.is_err(),
        "mismatched backend/circuit attachment should be rejected"
    );

    Ok(())
}
