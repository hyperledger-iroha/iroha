//! Publisher seeding and challenged native publication qualification.
use super::*;
use iroha_data_model::{
    block::consensus_v2::finality::V2FinalityArtifact,
    isi::sorafs::AssertSorafsPublicationV1,
    sorafs::{
        pin_registry::{ReplicationOrderStatus, derive_sorafs_auto_replication_order_id_v1},
        publication::{
            PUBLICATION_PROOF_MAX_BYTES_V1, SorafsPublicationPreparationV1,
            SorafsPublicationProofRequestV1, SorafsPublicationProofV1,
            verify_sorafs_publication_v1,
        },
    },
    transaction::SignedTransaction,
};
use sha2::{Digest as _, Sha256};
use sorafs_car::publisher::{
    PublisherSourceChunkRequestV1, PublisherSourceHeaderV1, PublisherSourceRequestV1,
    PublisherSourceUploadV1,
};
use sorafs_manifest::capacity::ReplicationOrderV1;
use std::time::Instant;

const PUBLICATION_TIMEOUT: Duration = Duration::from_secs(600);
const PREPARATION_MAX_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn load_checkpoint(
    path: &Path,
    network: &NetworkId,
) -> Result<V2FinalityArtifact, String> {
    let bytes = read_file_bounded(path, 4 * 1024 * 1024, "trusted finality checkpoint")?;
    let checkpoint: V2FinalityArtifact = decode(&bytes, 4 * 1024 * 1024)?;
    if checkpoint.height_context.network_id != *network || checkpoint.height == 0 {
        return Err(
            "trusted publication checkpoint belongs to another network or has zero height"
                .to_owned(),
        );
    }
    checkpoint
        .verify()
        .map_err(|_| "trusted publication checkpoint has invalid native finality".to_owned())?;
    Ok(checkpoint)
}

fn decode<
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize + norito::NoritoSchema,
>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, String> {
    if bytes.len() > maximum {
        return Err("publication response exceeds its bound".to_owned());
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(maximum, maximum, maximum, maximum * 4, 64),
    )
    .map_err(|_| "publication response is not canonical bounded Norito".to_owned())
}

fn ensure_live(started: Instant) -> Result<(), String> {
    if started.elapsed() >= PUBLICATION_TIMEOUT {
        Err("publication did not finalize before its ten-minute deadline".to_owned())
    } else {
        Ok(())
    }
}

pub(super) fn endpoint(base: &Url, path: &str) -> Result<Url, String> {
    if base.username() != ""
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !(base.scheme() == "https"
            || base.scheme() == "http"
                && matches!(base.host_str(), Some("127.0.0.1" | "[::1]" | "::1")))
    {
        return Err("publication endpoints require HTTPS, or HTTP on explicit loopback, without URL credentials/query/fragment".to_owned());
    }
    base.join(path)
        .map_err(|_| "invalid publication endpoint".to_owned())
}

fn request_message(
    network: &NetworkId,
    url: &Url,
    bytes: &[u8],
    timestamp: u64,
    nonce: &str,
) -> Result<Vec<u8>, String> {
    if url.query().is_some()
        || nonce.is_empty()
        || nonce.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err("noncanonical publication request".to_owned());
    }
    let mut message = b"iroha.app.request.network.v1\0".to_vec();
    message.extend_from_slice(network.as_bytes());
    message.extend_from_slice(
        format!(
            "POST\n{}\n\n{}\n{timestamp}\n{nonce}",
            url.path(),
            hex_encode(Sha256::digest(bytes))
        )
        .as_bytes(),
    );
    Ok(message)
}

pub(super) fn quote_and_sign(
    client: &HttpClient,
    base: &Url,
    private_key: &PrivateKey,
    mut payload: iroha_data_model::transaction::TransactionPayload,
) -> Result<SignedTransaction, String> {
    let network = payload
        .network_id()
        .ok_or_else(|| "publication fee quote requires exact network".to_owned())?;
    let url = endpoint(base, "v1/fees/quote")?;
    let bytes = norito::json::to_vec(&iroha_torii_shared::FeeQuoteRequest {
        payload: payload.clone(),
    })
    .map_err(|_| "publication fee quote encoding failed".to_owned())?;
    let timestamp = reputation_request_timestamp_ms_at(SystemTime::now())?;
    let nonce = reputation_request_nonce_with_rng(&mut rand::rngs::OsRng)?;
    let signature = Signature::try_new(
        private_key,
        &request_message(network, &url, &bytes, timestamp, &nonce)?,
    )
    .map_err(|_| "publication fee quote signing failed".to_owned())?;
    let response = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .header(
            REPUTATION_HEADER_ACCOUNT,
            payload
                .authority()
                .to_canonical_hex()
                .map_err(|_| "invalid publisher account".to_owned())?,
        )
        .header(
            REPUTATION_HEADER_SIGNATURE,
            BASE64_STANDARD.encode(signature.payload()),
        )
        .header(REPUTATION_HEADER_TIMESTAMP_MS, timestamp.to_string())
        .header(REPUTATION_HEADER_NONCE, nonce)
        .body(bytes)
        .send()
        .map_err(|_| "publication fee quote unavailable".to_owned())?;
    if !response.status().is_success()
        || response.headers().get_all(CONTENT_TYPE).iter().count() != 1
        || response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            != Some("application/json")
    {
        return Err("publication fee quote rejected".to_owned());
    }
    let bytes = read_response(response, 1024 * 1024)?;
    let quote: iroha_torii_shared::FeeQuoteResponse = norito::json::from_slice(&bytes)
        .map_err(|_| "publication fee quote is malformed".to_owned())?;
    quote.validate_for_draft(&payload).map_err(|_| {
        "publication fee quote changed or invalidated the signed payer selection".to_owned()
    })?;
    payload.fee_payment = quote.intent;
    TransactionBuilder::from_payload(payload)
        .and_then(|builder| builder.try_sign(private_key))
        .map_err(|_| "publication exact fee-quoted payload signing failed".to_owned())
}

fn post(
    client: &HttpClient,
    config: &DeployClientConfig,
    url: Url,
    bytes: Vec<u8>,
) -> Result<reqwest::blocking::Response, String> {
    let authority = AccountId::new(config.public_key.clone());
    let timestamp = reputation_request_timestamp_ms_at(SystemTime::now())?;
    let nonce = reputation_request_nonce_with_rng(&mut rand::rngs::OsRng)?;
    let message = request_message(&config.network_id, &url, &bytes, timestamp, &nonce)?;
    let signature = Signature::try_new(&config.private_key, &message)
        .map_err(|_| "failed to authenticate publication request".to_owned())?;
    client
        .post(url)
        .header(CONTENT_TYPE, "application/x-norito")
        .header("Accept", "application/x-norito")
        .header(ACCEPT_ENCODING, "identity")
        .header(
            REPUTATION_HEADER_ACCOUNT,
            authority
                .to_canonical_hex()
                .map_err(|_| "invalid publisher account".to_owned())?,
        )
        .header(
            REPUTATION_HEADER_SIGNATURE,
            BASE64_STANDARD.encode(signature.payload()),
        )
        .header(REPUTATION_HEADER_TIMESTAMP_MS, timestamp.to_string())
        .header(REPUTATION_HEADER_NONCE, nonce)
        .body(bytes)
        .send()
        .map_err(|_| "publication transport unavailable".to_owned())
}

fn read_response(
    mut response: reqwest::blocking::Response,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    if response
        .headers()
        .get(CONTENT_ENCODING)
        .is_some_and(|encoding| encoding != "identity")
        || response
            .content_length()
            .is_some_and(|length| length > maximum as u64)
    {
        return Err("publication response encoding or length rejected".to_owned());
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "publication response could not be read".to_owned())?;
    if bytes.len() > maximum {
        return Err("publication response exceeded bound".to_owned());
    }
    Ok(bytes)
}

fn preparation(
    client: &HttpClient,
    base: &Url,
    config: &DeployClientConfig,
    digest: ManifestDigest,
    started: Instant,
    completed: bool,
) -> Result<SorafsPublicationPreparationV1, String> {
    loop {
        ensure_live(started)?;
        let response = post(
            client,
            config,
            endpoint(base, "v1/sorafs/publish/prepare")?,
            norito::to_bytes(&digest)
                .map_err(|_| "manifest selector encoding failed".to_owned())?,
        )?;
        let status = response.status();
        if status == StatusCode::ACCEPTED || status == StatusCode::NOT_FOUND {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        if status != StatusCode::OK {
            return Err(format!("publication preparation rejected with {status}"));
        }
        let row: SorafsPublicationPreparationV1 = decode(
            &read_response(response, PREPARATION_MAX_BYTES)?,
            PREPARATION_MAX_BYTES,
        )?;
        if row.pin.digest != digest
            || row.pin.submitted_by != AccountId::new(config.public_key.clone())
            || row.order.order_id != derive_sorafs_auto_replication_order_id_v1(&digest)
            || row.order.manifest_digest != digest
        {
            return Err(
                "publication preparation substituted the requested publisher, manifest or order"
                    .to_owned(),
            );
        }
        if matches!(row.pin.status, PinStatus::Retired(_))
            || matches!(
                row.order.status,
                ReplicationOrderStatus::Expired(_) | ReplicationOrderStatus::Cancelled(_)
            )
        {
            return Err("publication pin or assignment is no longer active".to_owned());
        }
        if !matches!(row.pin.status, PinStatus::Approved(_))
            || completed && !matches!(row.order.status, ReplicationOrderStatus::Completed(_))
        {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        return Ok(row);
    }
}

fn assert_and_verify(
    client: &HttpClient,
    base: &Url,
    config: &DeployClientConfig,
    row: &SorafsPublicationPreparationV1,
    floor: &V2FinalityArtifact,
    started: Instant,
    completed: bool,
    out: &Path,
) -> Result<V2FinalityArtifact, String> {
    ensure_live(started)?;
    let mut challenge = [0; 32];
    rand::rand_core::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut challenge)
        .map_err(|_| "publication challenge entropy unavailable".to_owned())?;
    if challenge == [0; 32] {
        return Err("publication challenge entropy invalid".to_owned());
    }
    let instruction = AssertSorafsPublicationV1 {
        manifest_digest: row.pin.digest,
        order_id: row.order.order_id,
        assignment_revision: row.order.assignment_revision,
        canonical_order_digest: *blake3_hash(&row.order.canonical_order).as_bytes(),
        require_complete: completed,
        challenge,
        minimum_height: floor.height,
        minimum_block_hash: *floor.block_hash.as_ref(),
    };
    let payload = TransactionBuilder::new(
        config.network_id,
        AccountId::new(config.public_key.clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .into_payload()
    .map_err(|_| "publication assertion construction failed".to_owned())?;
    let signed = quote_and_sign(client, base, &config.private_key, payload)?;
    let response = client
        .post(endpoint(base, "v1/pipeline/transactions")?)
        .header(CONTENT_TYPE, "application/x-norito")
        .body(signed.encode_versioned())
        .send()
        .map_err(|_| "publication assertion submission unavailable".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "publication assertion submission rejected with {}",
            response.status()
        ));
    }
    let request = SorafsPublicationProofRequestV1 {
        entry_hash: *signed.hash_as_entrypoint().as_ref(),
        floor_height: floor.height,
        floor_block_hash: *floor.block_hash.as_ref(),
    };
    let request_bytes = norito::to_bytes(&request)
        .map_err(|_| "publication proof selector encoding failed".to_owned())?;
    loop {
        ensure_live(started)?;
        let response = post(
            client,
            config,
            endpoint(base, "v1/sorafs/publish/proof")?,
            request_bytes.clone(),
        )?;
        if response.status() == StatusCode::ACCEPTED {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        if response.status() != StatusCode::OK {
            return Err(format!(
                "publication proof unavailable with {}; full block carriers require CanReadAllLedgerData and a checkpoint within 1023 blocks",
                response.status()
            ));
        }
        let bytes = read_response(response, PUBLICATION_PROOF_MAX_BYTES_V1)?;
        let proof: SorafsPublicationProofV1 = decode(&bytes, PUBLICATION_PROOF_MAX_BYTES_V1)?;
        let verified = verify_sorafs_publication_v1(&config.network_id, floor, &signed, &proof)
            .map_err(|_| {
                "publication assertion did not have matching successful native finalized execution"
                    .to_owned()
            })?;
        ensure_live(started)?;
        if verified.completed() != completed {
            return Err("publication phase proof substituted".to_owned());
        }
        write_bytes(out, &bytes)?;
        return Ok(verified.finality().clone());
    }
}

pub(super) fn qualify(
    client: &HttpClient,
    base: &Url,
    config: &DeployClientConfig,
    artifacts: &DeployPackArtifacts,
    checkpoint: V2FinalityArtifact,
    provider_urls: &[String],
    out_dir: &Path,
) -> Result<Value, String> {
    let started = Instant::now();
    if provider_urls.is_empty() || provider_urls.len() > 64 {
        return Err("publication requires 1..=64 provider source endpoints".to_owned());
    }
    let digest = ManifestDigest::new(
        *artifacts
            .manifest
            .digest()
            .map_err(|_| "manifest digest failed".to_owned())?
            .as_bytes(),
    );
    let assigned = preparation(client, base, config, digest, started, false)?;
    let order: ReplicationOrderV1 = decode(&assigned.order.canonical_order, 256 * 1024)?;
    order
        .validate()
        .map_err(|_| "publication assignment is invalid".to_owned())?;
    let assigned_floor = assert_and_verify(
        client,
        base,
        config,
        &assigned,
        &checkpoint,
        started,
        false,
        &out_dir.join("publication.assigned.proof.to"),
    )?;
    for assignment in &order.assignments {
        if assigned
            .order
            .provider_completions
            .iter()
            .any(|completion| completion.provider_id.as_bytes() == &assignment.provider_id)
        {
            continue;
        }
        let header = PublisherSourceHeaderV1::new(
            assignment.provider_id,
            order.order_id,
            assigned.order.assignment_revision,
            &artifacts.manifest,
            &artifacts.plan,
        )
        .map_err(|_| "publisher source metadata does not match prepared manifest".to_owned())?;
        let header_digest = header
            .canonical_digest()
            .map_err(|_| "publisher metadata digest failed".to_owned())?;
        let reservation = norito::encode_canonical(&PublisherSourceRequestV1::Metadata(header))
            .map_err(|_| "publisher source encoding failed".to_owned())?;
        let mut selected = None;
        for candidate in provider_urls {
            ensure_live(started)?;
            let candidate =
                Url::parse(candidate).map_err(|_| "invalid provider source URL".to_owned())?;
            let url = endpoint(&candidate, "v1/sorafs/publish/source")?;
            match post(client, config, url.clone(), reservation.clone()) {
                Ok(response) if response.status() == StatusCode::NO_CONTENT => {
                    selected = Some(url);
                    break;
                }
                _ => {}
            }
        }
        let url = selected.ok_or_else(|| {
            format!(
                "no configured endpoint accepted initial source for assigned provider {}",
                hex_encode(assignment.provider_id)
            )
        })?;
        for (index, chunk) in artifacts.plan.chunks.iter().enumerate() {
            ensure_live(started)?;
            let offset = usize::try_from(chunk.offset)
                .map_err(|_| "source chunk offset overflow".to_owned())?;
            let end = offset
                .checked_add(chunk.length as usize)
                .ok_or_else(|| "source chunk length overflow".to_owned())?;
            let bytes = artifacts
                .payload
                .get(offset..end)
                .ok_or_else(|| "source chunk exceeds verified payload".to_owned())?
                .to_vec();
            let request = PublisherSourceRequestV1::Chunk(PublisherSourceChunkRequestV1 {
                provider_id: assignment.provider_id,
                order_id: order.order_id,
                assignment_revision: assigned.order.assignment_revision,
                manifest_digest: *digest.as_bytes(),
                header_digest,
                upload: PublisherSourceUploadV1 {
                    index: u32::try_from(index)
                        .map_err(|_| "source chunk ordinal overflow".to_owned())?,
                    bytes,
                },
            });
            let response = post(
                client,
                config,
                url.clone(),
                norito::encode_canonical(&request)
                    .map_err(|_| "publisher chunk encoding failed".to_owned())?,
            )?;
            if response.status() != StatusCode::NO_CONTENT {
                return Err(format!(
                    "publisher source chunk was rejected with {}",
                    response.status()
                ));
            }
        }
    }
    let complete = preparation(client, base, config, digest, started, true)?;
    if complete.order.assignment_revision != assigned.order.assignment_revision
        || complete.order.canonical_order != assigned.order.canonical_order
    {
        return Err("publication assignment changed while supplying source bytes".to_owned());
    }
    let finality = assert_and_verify(
        client,
        base,
        config,
        &complete,
        &assigned_floor,
        started,
        true,
        &out_dir.join("publication.completed.proof.to"),
    )?;
    write_bytes(
        &out_dir.join("publication.finality.to"),
        &norito::to_bytes(&finality)
            .map_err(|_| "publication checkpoint encoding failed".to_owned())?,
    )?;
    Ok(Value::Object(Map::from_iter([
        ("state".into(), Value::from("completed")),
        ("assignment_finalized".into(), Value::from(true)),
        ("completion_finalized".into(), Value::from(true)),
        (
            "initial_source".into(),
            Value::from("authenticated_chunk_staging"),
        ),
        (
            "order_id_hex".into(),
            Value::from(hex_encode(order.order_id)),
        ),
        (
            "assignment_revision".into(),
            Value::from(complete.order.assignment_revision),
        ),
        (
            "provider_count".into(),
            Value::from(order.assignments.len() as u64),
        ),
        ("finalized_height".into(), Value::from(finality.height)),
        (
            "finalized_block_hash_hex".into(),
            Value::from(hex_encode(finality.block_hash.as_ref())),
        ),
        ("direct_http_ingest".into(), Value::from(false)),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new([seed]),
        ))
    }
    #[test]
    fn publication_request_signature_binds_network_target_body_and_freshness() {
        let url = Url::parse("https://node.example/v1/sorafs/publish/source").unwrap();
        let message = request_message(&network(1), &url, b"chunk", 100, "nonce").unwrap();
        let signer = KeyPair::try_from_seed(vec![4; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        let signature = Signature::try_new(signer.private_key(), &message).unwrap();
        signature.verify(signer.public_key(), &message).unwrap();
        for altered in [
            request_message(&network(2), &url, b"chunk", 100, "nonce").unwrap(),
            request_message(&network(1), &url, b"wrong", 100, "nonce").unwrap(),
            request_message(&network(1), &url, b"chunk", 101, "nonce").unwrap(),
            request_message(&network(1), &url, b"chunk", 100, "other").unwrap(),
            request_message(
                &network(1),
                &Url::parse("https://node.example/v1/sorafs/publish/proof").unwrap(),
                b"chunk",
                100,
                "nonce",
            )
            .unwrap(),
        ] {
            assert!(signature.verify(signer.public_key(), &altered).is_err());
        }
    }
    #[test]
    fn publication_endpoints_and_deadlines_fail_closed() {
        for raw in [
            "http://node.example/",
            "https://user@node.example/",
            "https://node.example/?token=secret",
            "https://node.example/#fragment",
        ] {
            assert!(endpoint(&Url::parse(raw).unwrap(), "v1/sorafs/publish/source").is_err());
        }
        assert!(
            endpoint(
                &Url::parse("http://127.0.0.1:8080/").unwrap(),
                "v1/sorafs/publish/source"
            )
            .is_ok()
        );
        assert!(ensure_live(Instant::now()).is_ok());
        assert!(ensure_live(Instant::now() - PUBLICATION_TIMEOUT).is_err());
        let temporary = tempfile::NamedTempFile::new().unwrap();
        fs::write(temporary.path(), b"HTTP 200 is not native finality").unwrap();
        assert!(load_checkpoint(temporary.path(), &network(1)).is_err());
    }
}
