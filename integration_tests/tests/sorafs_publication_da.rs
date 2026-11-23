//! Four validators publish a signed multipart DA blob and recover it from cold providers.
use super::*;
use iroha::da::{DaCommitmentProofRequest, DaIngestParams, DaPinIntentQueryRequest};
use iroha_core::da::{verify_da_commitment_proof, verify_da_pin_intent_proof};
use iroha_data_model::{
    da::{
        commitment::DaProofScheme,
        manifest::{ChunkRole, DaManifestV1},
        types::ExtraMetadata,
    },
    isi::{Log, SetParameter},
    parameter::Parameter,
    sumeragi_finality::SumeragiFinalityVerifier,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

fn receipt_operator(index: usize) -> KeyPair {
    KeyPair::try_from_seed(
        format!("DA publication receipt operator {index}").into_bytes(),
        iroha_crypto::Algorithm::Ed25519,
    )
    .unwrap()
}

fn configure_da(index: usize, root: &Path, layer: &mut toml::Table) {
    let operator = receipt_operator(index);
    set(
        layer,
        &["torii", "receipt_public_key"],
        operator.public_key().to_string(),
    );
    set(
        layer,
        &["torii", "receipt_private_key"],
        iroha_crypto::ExposedPrivateKey(operator.private_key().clone())
            .try_to_multihash_string()
            .unwrap(),
    );
    let set_path = |layer: &mut toml::Table, key: &str, directory: &str| {
        set(
            layer,
            &["torii", "da_ingest", key],
            root.join(format!("{directory}-{index}"))
                .display()
                .to_string(),
        );
    };
    set_path(layer, "manifest_store_dir", "da-manifests");
    set_path(layer, "replay_cache_store_dir", "da-replay");
    // The three real providers in this fixture carry funded Hot reserve terms.
    let policy = toml::toml! {
        [[overrides]]
        class = "nexus_lane_sidecar"
        [overrides.retention]
        hot_retention_secs = 3600
        cold_retention_secs = 172800
        required_replicas = 3
        storage_class = "hot"
        governance_tag = "da.sidecar"
    };
    set(layer, &["torii", "da_ingest", "replication_policy"], policy);
}

async fn ingest(published: &PublishedNetwork, payload: &[u8]) -> Result<DaIngestReceipt> {
    let governor = client(&published.network, 0, &ALICE_KEYPAIR);
    let reader = governor.clone();
    let policy = read_on_dedicated_thread(move || {
        crate::da_admission::policy_for(
            reader.client().get_lane_lifecycle_status()?,
            cli_publisher_id(),
            1,
        )
    })
    .await?;
    submit_instruction(
        &governor,
        SetParameter::new(Parameter::Custom(policy.into_custom_parameter())),
    )
    .await?;
    let producer = client(&published.network, 0, &cli_publisher());
    let producer = integration_tests::sync::rebind_blocking_client(&producer, |client| {
        client.torii_request_timeout = wire::DEADLINE;
    });
    let bytes = payload.to_vec();
    let result = read_on_dedicated_thread(move || {
        let params = DaIngestParams {
            epoch: 1,
            sequence: 0,
            ..Default::default()
        };
        producer.client().submit_da_blob(
            bytes,
            &params,
            ExtraMetadata::default(),
            None,
            receipt_operator(0).public_key(),
        )
    })
    .await?;
    ensure!(
        result.status == "accepted",
        "DA ingest returned {}",
        result.status
    );
    let receipt = result
        .receipt
        .ok_or_else(|| eyre!("signed multipart ingest omitted its receipt"))?;
    ensure!(
        receipt.retention_policy.required_replicas == 3
            && receipt.retention_policy.storage_class == StorageClass::Hot
    );
    ensure!(receipt.blob_hash.as_bytes() == blake3::hash(payload).as_bytes());
    Ok(receipt)
}

async fn finalized_inclusion(
    published: &PublishedNetwork,
    receipt: &DaIngestReceipt,
) -> Result<SumeragiFinalityCheckpoint> {
    let expected = receipt.commitment_record(0, DaProofScheme::SorafsPorBlake3);
    let reader = client(&published.network, 2, &ALICE_KEYPAIR);
    let query = DaCommitmentProofRequest {
        manifest_hash: Some(expected.manifest_hash),
        lane_id: Some(0),
        epoch: Some(1),
        sequence: Some(0),
    };
    let payer = client(&published.network, 0, &ALICE_KEYPAIR);
    for round in 0..32 {
        submit_instruction(
            &payer,
            Log::new(
                iroha_data_model::Level::INFO,
                format!("DA inclusion paid work {round}"),
            ),
        )
        .await?;
        if let Some(found) = reader
            .account_client()
            .da()
            .prove_commitment(&query)
            .await?
        {
            ensure!(
                found.proof.commitment == expected,
                "independent peer returned another commitment"
            );
            let checkpoint = wire::advance_checkpoint(
                &published.http,
                &published.network,
                &published.floor,
                found.proof.location.block_height,
            )
            .await?;
            let (_, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
                &checkpoint,
                &published.network.network_id(),
                published.network.chain_id().as_ref(),
            )?;
            let block = tip.block();
            verify_da_commitment_proof(&found.proof, &block.header(), &found.policies)?;
            let pin_proof = reader
                .account_client()
                .da()
                .prove_pin_intent(&DaPinIntentQueryRequest {
                    storage_ticket: Some(receipt.storage_ticket),
                    ..Default::default()
                })
                .await?
                .ok_or_else(|| eyre!("independent peer omitted the finalized pin-intent proof"))?;
            verify_da_pin_intent_proof(&pin_proof, &block.header())?;
            ensure!(
                block
                    .da_commitments()
                    .is_some_and(|bundle| bundle.commitments().contains(&expected))
            );
            let pin = block
                .da_pin_intents()
                .and_then(|bundle| {
                    bundle
                        .intents
                        .iter()
                        .find(|pin| pin.storage_ticket == receipt.storage_ticket)
                })
                .ok_or_else(|| eyre!("authenticated inclusion block omitted the pin"))?;
            ensure!(
                pin == &pin_proof.intent
                    && pin.authorization.owner == cli_publisher_id()
                    && pin.pin_scope_authorization.scope.commitment_digest
                        == expected.pin_scope_digest()
                    && pin.pin_scope_authorization.has_valid_canonical_signatures()
                    && pin.authorization.has_valid_canonical_signatures()
            );
            // A different in-flight ordinary publication transaction can win the proposer race.
            // Authenticate a positive fee from the actual inclusion block's execution outputs.
            let charged = (0..block.network_entrypoint_count()).any(|index| {
                let Some(TransactionEntrypoint::External(tx)) = block.network_entrypoint_at(index)
                else {
                    return false;
                };
                let Some((_, output)) = u32::try_from(index)
                    .ok()
                    .and_then(|i| block.network_output_at(i))
                else {
                    return false;
                };
                output.result.0.is_ok()
                    && output.result.nexus_fee_receipt().is_some_and(|fee| {
                        fee.block_height == found.proof.location.block_height
                            && fee.source_id
                                == *iroha_crypto::Hash::from(tx.hash_as_entrypoint()).as_ref()
                            && fee.fee_amount == Quantity::from(1_u32)
                    })
            });
            ensure!(
                charged,
                "the certified DA block must contain a successful ordinary transaction with an actual fee debit"
            );
            return Ok(checkpoint);
        }
    }
    Err(eyre!(
        "DA pair stayed uncommitted across 32 paid transactions"
    ))
}

async fn read_asset(
    published: &PublishedNetwork,
    peer: usize,
    root_cid: &str,
    name: &str,
    maximum: usize,
) -> Result<Vec<u8>> {
    let url = format!(
        "{}/sorafs/cid/{root_cid}/{name}",
        published.network.peers()[peer]
            .torii_url()
            .trim_end_matches('/')
    );
    let response = published
        .http
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .await?
        .error_for_status()?;
    ensure!(
        response
            .headers()
            .get("sora-content-cid")
            .and_then(|header| header.to_str().ok())
            == Some(root_cid),
        "cold provider returned a different content CID"
    );
    wire::bytes(response, maximum).await
}

async fn verify_cold_lookup(
    published: &PublishedNetwork,
    peer: usize,
    publication: &ManifestV1,
    manifest: &DaManifestV1,
) -> Result<()> {
    let package_cid = cid(&publication.root_cid);
    let response = published
        .http
        .get(format!(
            "{}/v1/sorafs/cid/{package_cid}?limit=1",
            published.network.peers()[peer]
                .torii_url()
                .trim_end_matches('/')
        ))
        .send()
        .await?
        .error_for_status()?;
    let value: norito::json::Value = norito::json::from_slice(&wire::bytes(response, 8192).await?)?;
    let digest = publication.digest()?.to_hex();
    let files = value["files"]
        .as_array()
        .ok_or_else(|| eyre!("gateway omitted its file list"))?;
    ensure!(
        value["content_cid"].as_str() == Some(package_cid.as_str())
            && value["manifest_digest_hex"].as_str() == Some(digest.as_str())
            && value["manifest_id_hex"].as_str() == Some(digest.as_str())
            && value["file_count"].as_u64() == Some(manifest.chunks.len() as u64 + 1)
            && value["returned_file_count"].as_u64() == Some(1)
            && value["truncated_files"].as_bool() == Some(true)
            && files.len() == 1,
        "cold provider CID lookup differs from the deployed package"
    );
    let first = manifest
        .chunks
        .first()
        .ok_or_else(|| eyre!("fixture requires a shard"))?;
    let name = format!("chunk_{:05}.bin", first.index);
    let path = files[0]["path"]
        .as_array()
        .ok_or_else(|| eyre!("gateway omitted its path components"))?;
    ensure!(
        path.len() == 1
            && path[0].as_str() == Some(name.as_str())
            && files[0]["offset"].as_u64() == Some(0)
            && files[0]["size"].as_u64() == Some(u64::from(first.length)),
        "cold provider file inventory differs from the signed manifest"
    );
    Ok(())
}

async fn recover_cli(
    published: &PublishedNetwork,
    package_cid: &str,
    receipt: &DaIngestReceipt,
    expected: &[u8],
    gateways: &[String],
    stem: &str,
) -> Result<()> {
    let root = published.runtime_directory.path();
    let out = root.join(format!("{stem}.bin"));
    let log = root.join(format!("{stem}.log"));
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&log)?;
    let mut command = tokio::process::Command::new(&published.cli_binary);
    command
        .arg("da-recover")
        .arg(format!("--cid={package_cid}"))
        .arg(format!(
            "--manifest-hash={}",
            hex::encode(receipt.manifest_hash.as_bytes())
        ))
        .arg(format!("--output={}", out.display()))
        .stdout(output.try_clone()?)
        .stderr(output)
        .kill_on_drop(true);
    for gateway in gateways {
        command.arg(format!("--gateway={gateway}"));
    }
    let mut child = command.spawn()?;
    let status = match timeout(wire::DEADLINE, child.wait()).await {
        Ok(result) => result?,
        Err(deadline) => {
            child.kill().await?;
            child.wait().await?;
            return Err(deadline).wrap_err("cold DA recovery CLI exceeded its deadline");
        }
    };
    ensure!(
        status.success(),
        "cold DA recovery CLI failed ({status}): {}",
        bounded_cli_failure_log(&log)
    );
    ensure!(
        fs::metadata(&out)?.len() == expected.len() as u64 && fs::read(&out)? == expected,
        "cold DA recovery CLI changed the original signed bytes"
    );
    Ok(())
}

/// Read one bounded loopback HTTP request without advancing an upstream URL from user input.
async fn requested_target(stream: &mut TcpStream) -> Result<String> {
    let mut headers = [0_u8; 8192];
    let mut used = 0;
    loop {
        let read = stream.read(&mut headers[used..]).await?;
        ensure!(read != 0, "truncated or oversized cold gateway request");
        used += read;
        if headers[..used].windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
    }
    let first = std::str::from_utf8(&headers[..used])?
        .lines()
        .next()
        .ok_or_else(|| eyre!("missing request line"))?;
    let path = first
        .strip_prefix("GET ")
        .and_then(|rest| rest.strip_suffix(" HTTP/1.1"))
        .ok_or_else(|| eyre!("unexpected cold gateway method or HTTP version"))?;
    Ok(path.to_owned())
}

/// The real CLI sees one unavailable data shard. Each other response is fetched from an
/// already-restarted provider and checked against the independently finalized manifest.
async fn recover_cli_with_missing_shard(
    published: &PublishedNetwork,
    cid: &str,
    receipt: &DaIngestReceipt,
    manifest: &DaManifestV1,
    payload: &[u8],
) -> Result<()> {
    let chunks = manifest
        .chunks
        .iter()
        .map(|chunk| (format!("chunk_{:05}.bin", chunk.index), chunk))
        .collect::<BTreeMap<_, _>>();
    let missing = manifest
        .chunks
        .iter()
        .find(|chunk| chunk.role == ChunkRole::Data)
        .ok_or_else(|| eyre!("fixture has no data shard"))?
        .index;
    let socket = TcpListener::bind(("127.0.0.1", 0)).await?;
    let gateways = [format!("http://{}/", socket.local_addr()?)];
    let prefix = format!("/sorafs/cid/{cid}/");
    let mut refused = 0;
    let mut parity_reads = 0;
    {
        let serve = async {
            let mut seen = BTreeSet::new();
            // A single CLI with one gateway may request each source file once. Hold only
            // one verified provider shard in memory and complete each write before counting it.
            for _ in 0..=chunks.len() + 1 {
                let (mut stream, remote) = socket.accept().await?;
                ensure!(remote.ip().is_loopback());
                let target = requested_target(&mut stream).await?;
                ensure!(seen.insert(target.clone()), "repeated cold package request");
                let name = target
                    .strip_prefix(&prefix)
                    .ok_or_else(|| eyre!("foreign package target"))?;
                let (body, parity) = if name == "manifest.norito" {
                    let bytes = read_asset(published, 1, cid, name, 2 << 20).await?;
                    ensure!(blake3::hash(&bytes).as_bytes() == receipt.manifest_hash.as_bytes());
                    (bytes, false)
                } else {
                    let chunk = chunks
                        .get(name)
                        .ok_or_else(|| eyre!("unexpected package asset {name}"))?;
                    if chunk.index == missing {
                        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                        stream.shutdown().await?;
                        refused += 1;
                        continue;
                    }
                    let parity = chunk.role != ChunkRole::Data;
                    let bytes = read_asset(
                        published,
                        if parity { 2 } else { 1 },
                        cid,
                        name,
                        chunk.length as usize,
                    )
                    .await?;
                    ensure!(
                        bytes.len() == chunk.length as usize
                            && blake3::hash(&bytes).as_bytes() == chunk.commitment.as_ref(),
                        "cold provider returned an incorrect shard for {name}"
                    );
                    (bytes, parity)
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).await?;
                stream.write_all(&body).await?;
                stream.shutdown().await?;
                parity_reads += usize::from(parity);
            }
            Err(eyre!(
                "CLI exceeded the finalized manifest's request inventory"
            ))
        };
        tokio::select! {
            result = recover_cli(published, cid, receipt, payload, &gateways, "cold-parity-recovered") => result?,
            result = serve => return result,
        }
    }
    ensure!(
        refused == 1 && parity_reads > 0,
        "cold da-recover must observe the missing data shard and retrieve actual provider parity"
    );
    Ok(())
}

#[test]
fn four_peer_multipart_da_publication_and_cold_parity_recovery() -> Result<()> {
    crate::sorafs_network::run("four-peer-multipart-da", || async {
        let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
        // The inclusion proof checks an exact one-unit native debit for the selected work.
        fees.base_fee = Quantity::one();
        fees.per_byte_fee = Quantity::zero();
        fees.per_instruction_fee = Quantity::zero();
        fees.per_gas_unit_fee = Quantity::zero();
        fees.fee_sink_account_id = BOB_ID.clone();
        let paid = SetParameter::new(Parameter::Custom(fees.into_custom_parameter()?));
        let published = Box::pin(create_publication_network(
            |builder| builder.with_genesis_instruction(paid),
            configure_da,
        ))
        .await?;
        // One complete 1 MiB upload part and one partial part, including a short tail shard.
        let payload = (0..(1_usize << 20) + 137)
            .map(|index| ((index * 73 + index / 17) % 251) as u8)
            .collect::<Vec<_>>();
        let receipt = ingest(&published, &payload).await?;
        let checkpoint = finalized_inclusion(&published, &receipt).await?;
        let ticket = hex::encode(receipt.storage_ticket.as_bytes());
        let source = published
            .runtime_directory
            .path()
            .join("da-manifests-0")
            .join("artifacts")
            .join(&ticket[..2])
            .join(&ticket);
        let publication =
            qualify_cli_deploy(&published, Some((&source, &checkpoint, &receipt))).await?;
        let root_cid = cid(&publication.root_cid);
        restart(&published, 1).await?;
        restart(&published, 2).await?;
        let lost = &published.network.peers()[0];
        ensure!(lost.is_running() && lost.process_id().await.is_some());
        timeout(wire::DEADLINE, lost.shutdown()).await?;
        ensure!(
            !lost.is_running() && lost.process_id().await.is_none(),
            "ingress provider survived the outage"
        );
        let gateways = [1, 2].map(|peer| published.network.peers()[peer].torii_url().to_string());
        recover_cli(
            &published,
            &root_cid,
            &receipt,
            &payload,
            &gateways,
            "cold-recovered",
        )
        .await?;

        let bytes = read_asset(&published, 1, &root_cid, "manifest.norito", 2 << 20).await?;
        ensure!(blake3::hash(&bytes).as_bytes() == receipt.manifest_hash.as_bytes());
        let manifest = DaManifestV1::decode_canonical(&bytes)?;
        ensure!(
            manifest.storage_ticket == receipt.storage_ticket
                && manifest.total_size == payload.len() as u64
        );
        let retention = &receipt.retention_policy;
        let deadline = manifest
            .issued_at_unix
            .checked_add(
                retention
                    .hot_retention_secs
                    .max(retention.cold_retention_secs),
            )
            .ok_or_else(|| eyre!("certified retention deadline overflow"))?;
        ensure!(
            manifest.retention_policy == *retention
                && publication.pin_policy.min_replicas == retention.required_replicas
                && publication.pin_policy.storage_class == sorafs_manifest::StorageClass::Hot
                && publication.pin_policy.retention_epoch == deadline,
            "provider publication changed the certified replica count, storage class or retention deadline"
        );
        for peer in [1, 2] {
            verify_cold_lookup(&published, peer, &publication, &manifest).await?;
        }
        recover_cli_with_missing_shard(&published, &root_cid, &receipt, &manifest, &payload)
            .await?;
        timeout(wire::DEADLINE, published.network.shutdown()).await?;
        for peer in published.network.all_peers() {
            ensure!(
                !peer.is_running() && peer.process_id().await.is_none(),
                "DA validator process survived teardown"
            );
        }
        Ok(())
    })
}
