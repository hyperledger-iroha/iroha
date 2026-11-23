//! Publisher seeding and challenged native publication qualification.
use super::*;
use iroha_data_model::{
    isi::sorafs::AssertSorafsPublicationV1,
    sorafs::{
        pin_registry::{
            ReplicationOrderStatus, SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
            derive_sorafs_auto_replication_order_id_v1,
        },
        publication::{
            PUBLICATION_PROOF_MAX_BYTES_V1, SorafsPublicationPreparationV1,
            SorafsPublicationProofRequestV1, SorafsPublicationProofV1,
            verify_sorafs_publication_v1,
        },
    },
    sumeragi_finality::{
        MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint, SumeragiFinalityVerifier,
        VerifiedSumeragiBlock,
    },
    transaction::SignedTransaction,
};
use sha2::{Digest as _, Sha256};
use sorafs_car::publisher::{
    PublisherSourceChunkRequestV1, PublisherSourceHeaderV1, PublisherSourceRequestV1,
    PublisherSourceUploadV1,
};
use sorafs_manifest::capacity::ReplicationOrderV1;
use std::io::Seek as _;
use std::time::Instant;

const PUBLICATION_TIMEOUT: Duration = Duration::from_secs(600);
const PREPARATION_MAX_BYTES: usize = 2 * 1024 * 1024;

// Public status DTOs are small; retain the SDK's bounded response envelope.
const REGISTRATION_STATUS_MAX_BYTES: usize = 64 * 1024;

/// Generated deploy manifests retain two canonical ingest windows: one window
/// of submission headroom followed by the full automatic-replication SLA.
/// Explicit manifests and the server's strict deadline/retention ceiling remain
/// unchanged. This default is not permission to extend publication timeouts.
pub(super) fn default_deploy_retention_epoch(created_epoch: u64) -> Result<u64, String> {
    let ingest_window = u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1);
    created_epoch
        .checked_add(ingest_window)
        .and_then(|epoch| epoch.checked_add(ingest_window))
        .ok_or_else(|| "retention deadline overflow".to_owned())
}

pub(super) fn load_checkpoint(
    path: &Path,
    network: &NetworkId,
    chain_id: &ChainId,
) -> Result<(SumeragiFinalityCheckpoint, VerifiedSumeragiBlock), String> {
    let bytes = read_file_bounded(
        path,
        MAX_FINALITY_CHECKPOINT_BYTES as u64,
        "trusted finality checkpoint",
    )?;
    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes).map_err(|_| {
        "trusted publication checkpoint is not canonical current finality".to_owned()
    })?;
    // This local file is independently selected by the caller; response material cannot replace
    // its signed-genesis, network, chain or authenticated lag-2 schedule commitments.
    let (_, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
        &checkpoint,
        network,
        chain_id.as_str(),
    )
    .map_err(|_| {
        "trusted publication checkpoint does not match the configured current network and chain"
            .to_owned()
    })?;
    Ok((checkpoint, tip))
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
    let nonce = reputation_request_nonce_with_rng(&mut rand::rngs::SysRng)?;
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
    let nonce = reputation_request_nonce_with_rng(&mut rand::rngs::SysRng)?;
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

// Status may stop a rejected publication, but can never establish successful
// publication. Success still requires the unchanged challenged native proof.
fn check_registration_status(
    expected: iroha_crypto::HashOf<SignedTransaction>,
    row: &iroha_torii_shared::PipelineTransactionStatusResponse,
) -> Result<(), String> {
    if row.hash != expected.to_string() || row.scope != "global" {
        return Err(
            "pin registration status substituted the exact signed hash or global scope".to_owned(),
        );
    }
    if !matches!(row.resolved_from.as_str(), "cache" | "queue" | "state") {
        return Err("pin registration status has an unsupported resolution source".to_owned());
    }
    if !matches!(
        row.status.kind.as_str(),
        "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired"
    ) {
        return Err("pin registration status has an unsupported status kind".to_owned());
    }
    if row.resolved_from == "state" && matches!(row.status.kind.as_str(), "Rejected" | "Expired") {
        return Err(format!(
            "pin registration {expected} reached global state-resolved {}",
            row.status.kind
        ));
    }
    Ok(())
}

fn check_registration_response(
    response: reqwest::blocking::Response,
    expected: iroha_crypto::HashOf<SignedTransaction>,
) -> Result<bool, String> {
    let status = response.status();
    if !matches!(status, StatusCode::OK | StatusCode::NOT_FOUND) {
        return Err(format!(
            "pin registration status read rejected with {status}"
        ));
    }
    if response.headers().get_all(CONTENT_TYPE).iter().count() != 1
        || response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            != Some("application/json")
    {
        return Err(
            "pin registration status requires one application/json content type".to_owned(),
        );
    }
    let maximum = if status == StatusCode::NOT_FOUND {
        iroha_torii_shared::PIPELINE_TRANSACTION_STATUS_NOT_FOUND_MAX_BYTES
    } else {
        REGISTRATION_STATUS_MAX_BYTES
    };
    let bytes = read_response(response, maximum)?;
    if status == StatusCode::NOT_FOUND {
        let envelope: iroha_torii_shared::ErrorEnvelope = norito::json::from_slice(&bytes)
            .map_err(|_| "pin registration status absence is malformed".to_owned())?;
        if envelope.code() != iroha_torii_shared::PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE
            || !envelope
                .details
                .as_ref()
                .and_then(|details| details.pipeline_transaction_status_not_found.as_ref())
                .is_some_and(|absence| absence.matches(&expected, "global"))
        {
            return Err(
                "pin registration status absence substituted the exact signed hash or global scope"
                    .to_owned(),
            );
        }
        return Ok(false);
    }
    let row: iroha_torii_shared::PipelineTransactionStatusResponse =
        norito::json::from_slice(&bytes)
            .map_err(|_| "pin registration status is malformed".to_owned())?;
    check_registration_status(expected, &row).map(|()| true)
}

fn registration_observed(
    client: &HttpClient,
    base: &Url,
    expected: iroha_crypto::HashOf<SignedTransaction>,
    started: Instant,
) -> Result<bool, String> {
    ensure_live(started)?;
    let remaining = PUBLICATION_TIMEOUT.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err("publication did not finalize before its ten-minute deadline".to_owned());
    }
    let mut status_url = endpoint(base, "v1/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", &expected.to_string())
        .append_pair("scope", "global");
    let response = client
        .get(status_url)
        .header("Accept", "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .timeout(remaining.min(Duration::from_secs(30)))
        .send()
        .map_err(|_| "pin registration status transport unavailable".to_owned())?;
    let result = check_registration_response(response, expected);
    ensure_live(started)?;
    result
}

/// A proof or preparation read can lose its Torii work budget while the original
/// transaction remains committed. Only read observations share this backoff.
fn publication_read_backoff(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    remaining: Duration,
) -> Option<Duration> {
    if !matches!(
        status,
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::TOO_MANY_REQUESTS
    ) {
        return None;
    }
    let delay = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map_or(Duration::from_millis(500), Duration::from_secs);
    Some(delay.max(Duration::from_millis(100)).min(remaining))
}

/// A 202 from the authenticated prepare route means the pin exists under this publisher
/// but its order is not available yet. Only a 404 admits a first registration.
enum Preparation {
    Absent,
    AwaitingOrder,
    Order(Box<SorafsPublicationPreparationV1>),
}

fn observe_preparation(
    session: PublicationSession<'_>,
    digest: ManifestDigest,
) -> Result<Preparation, String> {
    let PublicationSession {
        client,
        base,
        config,
        started,
    } = session;
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
        if let Some(wait) = publication_read_backoff(
            status,
            response.headers(),
            PUBLICATION_TIMEOUT.saturating_sub(started.elapsed()),
        ) {
            drop(response);
            std::thread::sleep(wait);
            continue;
        }
        match status {
            StatusCode::NOT_FOUND => return Ok(Preparation::Absent),
            StatusCode::ACCEPTED => return Ok(Preparation::AwaitingOrder),
            StatusCode::OK => {}
            _ => return Err(format!("publication preparation rejected with {status}")),
        }
        let row: SorafsPublicationPreparationV1 = decode(
            &read_response(response, PREPARATION_MAX_BYTES)?,
            PREPARATION_MAX_BYTES,
        )?;
        if row.pin.digest != digest
            || row.pin.submitted_by != AccountId::new(config.public_key.clone())
            || row.order.order_id != derive_sorafs_auto_replication_order_id_v1(&digest)
            || row.order.manifest_digest != digest
            || row.order.manifest_root_cid != row.pin.root_cid
            || row.order.musubi_archive.is_some()
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
        return Ok(Preparation::Order(Box::new(row)));
    }
}

fn preparation(
    session: PublicationSession<'_>,
    digest: ManifestDigest,
    registration_hash: Option<iroha_crypto::HashOf<SignedTransaction>>,
    completed: bool,
) -> Result<SorafsPublicationPreparationV1, String> {
    loop {
        match observe_preparation(session, digest)? {
            Preparation::Order(row) => {
                if row.pin.status.is_active()
                    && (!completed
                        || matches!(row.order.status, ReplicationOrderStatus::Completed(_)))
                {
                    return Ok(*row);
                }
            }
            Preparation::Absent if registration_hash.is_none() => {
                return Err("previously observed publication pin disappeared".into());
            }
            Preparation::Absent | Preparation::AwaitingOrder => {
                if let Some(hash) = registration_hash {
                    registration_observed(session.client, session.base, hash, session.started)?;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The only registration mutations are a new signed envelope or an exact replay
/// whose hash was absent from the global status lookup.
pub(super) enum Registration {
    Existing,
    Observed(iroha_crypto::HashOf<SignedTransaction>),
    Submitted(ManifestRegisterSubmission),
}

fn retained_registration(
    bytes: &[u8],
    request: &ManifestSubmitRequest<'_>,
    manifest: &ManifestV1,
) -> Result<SignedTransaction, String> {
    use iroha_data_model::transaction::Executable;
    use iroha_version::codec::DecodeVersioned as _;
    let tx = SignedTransaction::decode_all_versioned(bytes)
        .map_err(|_| "retained pin transaction is malformed".to_owned())?;
    let Executable::Instructions(instructions) = tx.instructions() else {
        return Err("retained pin transaction must contain a registration".into());
    };
    let [instruction] = instructions.as_ref() else {
        return Err("retained pin transaction must contain one registration".into());
    };
    let pin = instruction
        .as_any()
        .downcast_ref::<RegisterPinManifest>()
        .ok_or("retained pin transaction contains a different instruction")?;
    if tx.network_id() != Some(request.network_id)
        || tx.authority() != request.authority
        || pin.manifest_payload != manifest.encode().map_err(|e| e.to_string())?
        || pin.alias.is_some()
        || pin.successor_of.is_some()
        || tx.encode_versioned() != bytes
        || tx.verify_signature().is_err()
    {
        return Err(
            "retained pin transaction does not bind this network, publisher and exact manifest"
                .into(),
        );
    }
    Ok(tx)
}

fn registration_filename(network: &NetworkId, digest: &ManifestDigest) -> String {
    format!(
        "pin-register-{}-{}.to",
        hex_encode(network.as_bytes()),
        hex_encode(digest.as_bytes())
    )
}

pub(super) fn register_if_absent(
    session: PublicationSession<'_>,
    request: &ManifestSubmitRequest<'_>,
    manifest: &ManifestV1,
    out_dir: &Path,
) -> Result<Registration, String> {
    let digest = ManifestDigest::from_manifest(manifest).map_err(|e| e.to_string())?;
    if !matches!(observe_preparation(session, digest)?, Preparation::Absent) {
        return Ok(Registration::Existing);
    }

    // A durable exact transaction survives a lost response and races sharing this output
    // directory. The ledger's unique manifest digest protects independent output directories.
    let dir = iroha_fs::OwnerDirectory::open(out_dir.canonicalize().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let name = registration_filename(request.network_id, &digest);
    let limit = MAX_MANIFEST_ENCODED_BYTES + (256 << 10);
    let read = || {
        dir.read_private_optional(&name, limit)
            .map_err(|e| e.to_string())
    };
    let prior = match read()? {
        Some(prior) => prior,
        None => {
            let tx = prepare_pin_registration(request, manifest, None)?;
            let bytes = tx.encode_versioned();
            if bytes.len() > limit {
                return Err("pin registration envelope exceeds its retention bound".into());
            }
            match dir.write_private_atomic(&name, &bytes, iroha_fs::PublishMode::CreateNew) {
                Ok(()) => return send_pin_registration(request, &tx).map(Registration::Submitted),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    read()?.ok_or("concurrent pin registration evidence disappeared")?
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    };
    let tx = retained_registration(&prior, request, manifest)?;
    if registration_observed(session.client, session.base, tx.hash(), session.started)? {
        return Ok(Registration::Observed(tx.hash()));
    }
    send_pin_registration(request, &tx).map(Registration::Submitted)
}

/// Transport, signer and deadline shared by every publication round trip of one deployment.
#[derive(Clone, Copy)]
pub(super) struct PublicationSession<'a> {
    client: &'a HttpClient,
    base: &'a Url,
    config: &'a DeployClientConfig,
    started: Instant,
}

impl<'a> PublicationSession<'a> {
    /// Start the ten-minute publication deadline over the deployment's transport and signer.
    pub(super) fn begin(
        client: &'a HttpClient,
        base: &'a Url,
        config: &'a DeployClientConfig,
    ) -> Self {
        Self {
            client,
            base,
            config,
            started: Instant::now(),
        }
    }
}

// Locations in the receipt are relative to the deployment's artifact directory.
fn retain_evidence(out_dir: &Path, filename: &str, bytes: &[u8]) -> Result<Value, String> {
    write_bytes(&out_dir.join(filename), bytes)?;
    Ok(Value::Object(Map::from_iter([
        ("file".into(), Value::from(filename)),
        (
            "blake3_hex".into(),
            Value::from(blake3::hash(bytes).to_hex().to_string()),
        ),
    ])))
}

fn assert_and_verify(
    session: PublicationSession<'_>,
    row: &SorafsPublicationPreparationV1,
    floor: &SumeragiFinalityCheckpoint,
    completed: bool,
    out_dir: &Path,
) -> Result<(u64, Value), String> {
    let PublicationSession {
        client,
        base,
        config,
        started,
    } = session;
    ensure_live(started)?;
    let mut challenge = [0; 32];
    rand::rand_core::TryRng::try_fill_bytes(&mut rand::rngs::SysRng, &mut challenge)
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
        minimum_height: floor.height(),
        minimum_block_hash: *floor.block_hash().as_ref(),
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
    let signed_wire = signed.encode_wire_v1().map_err(|error| error.to_string())?;
    let response = client
        .post(endpoint(base, "v1/pipeline/transactions")?)
        .header(CONTENT_TYPE, "application/x-norito")
        .body(signed_wire.clone())
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
        floor_height: floor.height(),
        floor_block_hash: *floor.block_hash().as_ref(),
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
        if let Some(wait) = publication_read_backoff(
            response.status(),
            response.headers(),
            PUBLICATION_TIMEOUT.saturating_sub(started.elapsed()),
        ) {
            drop(response);
            std::thread::sleep(wait);
            continue;
        }
        if response.status() == StatusCode::ACCEPTED {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        if response.status() != StatusCode::OK {
            return Err(format!(
                "publication proof unavailable with {}; the checkpoint must be a committed block before the assertion and within 256 validator epochs",
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
        let phase = if completed { "completed" } else { "assigned" };
        // Compact inclusion proofs require the exact signed entrypoint for offline verification.
        let assertion = retain_evidence(
            out_dir,
            &format!("publication.{phase}.assertion.to"),
            &signed_wire,
        )?;
        let proof = retain_evidence(out_dir, &format!("publication.{phase}.proof.to"), &bytes)?;
        let height = verified.height();
        return Ok((
            height,
            Value::Object(Map::from_iter([
                ("height".into(), Value::from(height)),
                ("assertion".into(), assertion),
                ("proof".into(), proof),
            ])),
        ));
    }
}

pub(super) fn qualify(
    session: PublicationSession<'_>,
    artifacts: &mut DeployPackArtifacts,
    registration_hash: Option<iroha_crypto::HashOf<SignedTransaction>>,
    checkpoint: SumeragiFinalityCheckpoint,
    provider_urls: &[String],
    out_dir: &Path,
) -> Result<Value, String> {
    let PublicationSession {
        client,
        base: _,
        config,
        started,
    } = session;
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
    let assigned = preparation(session, digest, registration_hash, false)?;
    let order: ReplicationOrderV1 = decode(&assigned.order.canonical_order, 256 * 1024)?;
    order
        .validate()
        .map_err(|_| "publication assignment is invalid".to_owned())?;
    // Both assertions bind the same independently selected checkpoint: the compact proof
    // authenticates execution under it and never replaces it.
    let (_, assignment_evidence) =
        assert_and_verify(session, &assigned, &checkpoint, false, out_dir)?;
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
            artifacts
                .payload
                .seek(std::io::SeekFrom::Start(chunk.offset))
                .map_err(|error| error.to_string())?;
            let mut bytes = vec![0u8; chunk.length as usize];
            artifacts
                .payload
                .read_exact(&mut bytes)
                .map_err(|error| error.to_string())?;
            if blake3::hash(&bytes).as_bytes() != &chunk.digest {
                return Err("publisher source chunk changed after verification".to_owned());
            }
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
    let complete = preparation(session, digest, registration_hash, true)?;
    if complete.order.assignment_revision != assigned.order.assignment_revision
        || complete.order.canonical_order != assigned.order.canonical_order
    {
        return Err("publication assignment changed while supplying source bytes".to_owned());
    }
    let (finalized_height, completion_evidence) =
        assert_and_verify(session, &complete, &checkpoint, true, out_dir)?;
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
        (
            "provider_ids_hex".into(),
            Value::Array(
                order
                    .assignments
                    .iter()
                    .map(|assignment| Value::from(hex_encode(assignment.provider_id)))
                    .collect(),
            ),
        ),
        (
            "canonical_order".into(),
            retain_evidence(
                out_dir,
                "publication.order.to",
                &complete.order.canonical_order,
            )?,
        ),
        ("assignment_evidence".into(), assignment_evidence),
        ("completion_evidence".into(), completion_evidence),
        ("finalized_height".into(), Value::from(finalized_height)),
        ("direct_http_ingest".into(), Value::from(false)),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_evidence_identifies_the_exact_bytes_relative_to_the_receipt_output() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().canonicalize().unwrap();
        let name = "publication.completed.assertion.to";
        let bytes = b"signed publication assertion";
        let item = retain_evidence(&output, name, bytes).unwrap();
        assert_eq!(item["file"].as_str(), Some(name));
        let stored = fs::read(output.join(name)).unwrap();
        assert_eq!(stored.as_slice(), bytes);
        assert_eq!(
            item["blake3_hex"].as_str(),
            Some(blake3::hash(&stored).to_hex().as_str())
        );
        assert!(retain_evidence(&output.join(name), "proof", bytes).is_err());
    }

    fn network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new([seed]),
        ))
    }
    fn registration_hash(seed: u8) -> iroha_crypto::HashOf<SignedTransaction> {
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([seed]))
    }

    fn status_row(
        kind: &str,
        source: &str,
    ) -> iroha_torii_shared::PipelineTransactionStatusResponse {
        iroha_torii_shared::PipelineTransactionStatusResponse::new(
            registration_hash(1).to_string(),
            iroha_torii_shared::PipelineTransactionStatus {
                kind: kind.into(),
                block_height: None,
            },
            "global".into(),
            source.into(),
        )
    }

    fn absent_status(hash: iroha_crypto::HashOf<SignedTransaction>, scope: &str) -> Vec<u8> {
        use iroha_torii_shared::{
            ErrorDetails, ErrorEnvelope, PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE,
            PipelineTransactionStatusNotFoundV1,
        };
        norito::json::to_vec(
            &ErrorEnvelope::new(PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE, "No status")
                .with_details(ErrorDetails {
                    pipeline_transaction_status_not_found: Some(
                        PipelineTransactionStatusNotFoundV1::new(&hash, scope),
                    ),
                    ..ErrorDetails::default()
                }),
        )
        .unwrap()
    }

    // Exercise real HTTP response parsing without a node or any signing material from disk.
    fn status_server(
        replies: Vec<(u16, &'static str, Vec<u8>)>,
    ) -> (HttpClient, Url, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut requests = Vec::new();
            for (status, headers, body) in replies {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "missing bounded test request");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(error) => panic!("test accept: {error}"),
                    }
                };
                // BSD sockets (macOS) inherit the listener's O_NONBLOCK; Linux ones do not.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 16 * 1024, "test request header bound");
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                let body_length = request
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                assert!(body_length <= 16 * 1024, "test request body bound");
                stream.read_exact(&mut vec![0; body_length]).unwrap();
                requests.push(request);
                let head = format!(
                    "HTTP/1.1 {status} Test\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",
                    body.len()
                );
                if stream.write_all(head.as_bytes()).is_ok() {
                    let _ = stream.write_all(&body);
                }
            }
            requests
        });
        let client = HttpClient::builder()
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        (client, base, server)
    }

    #[test]
    fn deploy_default_retention_preserves_full_ingest_window_with_submission_headroom() {
        let created = 1_800_000_000;
        let window = u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1);
        let retention = default_deploy_retention_epoch(created).unwrap();
        assert_eq!(retention, created + 2 * window);
        for delay in [0, 1, 600, window - 1] {
            assert!(created + delay + window < retention);
        }
        // At the exact edge there is no strict headroom: the server must still refuse.
        assert_eq!(created + window + window, retention);
        assert_eq!(
            default_deploy_retention_epoch(u64::MAX - 2 * window).unwrap(),
            u64::MAX
        );
        assert!(default_deploy_retention_epoch(u64::MAX - 2 * window + 1).is_err());
        assert!(default_deploy_retention_epoch(u64::MAX).is_err());
    }

    #[test]
    fn registration_status_only_refuses_exact_global_state_rejection_or_expiry() {
        for source in ["cache", "queue", "state"] {
            for kind in [
                "Queued",
                "Approved",
                "Committed",
                "Applied",
                "Rejected",
                "Expired",
            ] {
                let result =
                    check_registration_status(registration_hash(1), &status_row(kind, source));
                let terminal = source == "state" && matches!(kind, "Rejected" | "Expired");
                assert_eq!(result.is_err(), terminal, "{source}/{kind}");
                if terminal {
                    assert!(
                        result
                            .unwrap_err()
                            .contains(&format!("state-resolved {kind}"))
                    );
                }
            }
        }
        for row in [
            iroha_torii_shared::PipelineTransactionStatusResponse {
                hash: registration_hash(2).to_string(),
                ..status_row("Rejected", "state")
            },
            iroha_torii_shared::PipelineTransactionStatusResponse {
                scope: "local".into(),
                ..status_row("Rejected", "state")
            },
            status_row("Rejected", "untrusted"),
            status_row("Unknown", "state"),
        ] {
            assert!(check_registration_status(registration_hash(1), &row).is_err());
        }
    }

    #[test]
    fn registration_http_reads_exact_hash_and_global_scope() {
        let expected = registration_hash(1);
        let replies = vec![
            (
                404,
                "Content-Type: application/json\r\n",
                absent_status(expected, "global"),
            ),
            (
                200,
                "Content-Type: application/json\r\n",
                norito::json::to_vec(&status_row("Approved", "state")).unwrap(),
            ),
            (
                200,
                "Content-Type: application/json\r\n",
                norito::json::to_vec(&status_row("Applied", "state")).unwrap(),
            ),
        ];
        let (client, base, server) = status_server(replies);
        for observed in [false, true, true] {
            assert_eq!(
                registration_observed(&client, &base, expected, Instant::now()).unwrap(),
                observed
            );
        }
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        for request in requests {
            assert!(
                request.starts_with(&format!(
                    "GET /v1/pipeline/transactions/status?hash={expected}&scope=global HTTP/1.1\r\n"
                )),
                "{request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("accept: application/json\r\n")
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("accept-encoding: identity\r\n")
            );
        }
    }

    #[test]
    fn registration_http_rejects_unbound_absence_and_permanent_response() {
        let expected = registration_hash(1);
        for (status, body) in [
            (404, absent_status(registration_hash(2), "global")),
            (404, absent_status(expected, "local")),
            (404, b"{}".to_vec()),
            (403, b"private upstream detail must not appear".to_vec()),
            (503, b"private upstream detail must not appear".to_vec()),
        ] {
            let (client, base, server) =
                status_server(vec![(status, "Content-Type: application/json\r\n", body)]);
            let error =
                registration_observed(&client, &base, expected, Instant::now()).unwrap_err();
            assert!(!error.contains("private upstream"));
            assert_eq!(server.join().unwrap().len(), 1);
        }
    }

    #[test]
    fn registration_http_rejects_wrong_media_encoding_and_oversize_envelopes() {
        let valid = norito::json::to_vec(&status_row("Approved", "state")).unwrap();
        for (status, headers, body) in [
            (200, "Content-Type: text/plain\r\n", valid.clone()),
            (
                200,
                "Content-Type: application/json\r\nContent-Type: application/json\r\n",
                valid.clone(),
            ),
            (
                200,
                "Content-Type: application/json\r\nContent-Encoding: gzip\r\n",
                valid,
            ),
            (
                200,
                "Content-Type: application/json\r\n",
                vec![b' '; REGISTRATION_STATUS_MAX_BYTES + 1],
            ),
            (
                404,
                "Content-Type: application/json\r\n",
                vec![b' '; iroha_torii_shared::PIPELINE_TRANSACTION_STATUS_NOT_FOUND_MAX_BYTES + 1],
            ),
        ] {
            let (client, base, server) = status_server(vec![(status, headers, body)]);
            assert!(
                registration_observed(&client, &base, registration_hash(1), Instant::now())
                    .is_err()
            );
            assert_eq!(server.join().unwrap().len(), 1);
        }
    }

    fn publication_test_config() -> DeployClientConfig {
        let key = KeyPair::try_from_seed(vec![4; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        DeployClientConfig {
            torii_url: None,
            public_key: key.public_key().clone(),
            private_key: key.private_key().clone(),
            network_id: network(1),
            chain_id: "publication-test".parse().unwrap(),
            chain_discriminant: 777,
        }
    }

    fn published_row(
        config: &DeployClientConfig,
        manifest: &ManifestV1,
    ) -> SorafsPublicationPreparationV1 {
        use iroha_data_model::sorafs::pin_registry::{PinManifestRecord, ReplicationOrderRecord};
        let owner = AccountId::new(config.public_key.clone());
        let digest = ManifestDigest::from_manifest(manifest).unwrap();
        let root = ManifestRootCid::try_from_slice(&manifest.root_cid).unwrap();
        let mut pin = PinManifestRecord::new(
            digest,
            root,
            chunker_handle_from_profile(&manifest.chunking),
            manifest.chunk_digest_sha3_256,
            manifest.por_root,
            manifest.content_length,
            convert_pin_policy(&manifest.pin_policy),
            owner.clone(),
            4,
            None,
            None,
            Metadata::default(),
        );
        pin.approve(5, None);
        SorafsPublicationPreparationV1 {
            pin,
            order: ReplicationOrderRecord {
                order_id: derive_sorafs_auto_replication_order_id_v1(&digest),
                manifest_digest: digest,
                manifest_root_cid: root,
                musubi_archive: None,
                issued_by: owner,
                issued_epoch: 5,
                deadline_epoch: 30,
                canonical_order: Vec::new(),
                assignment_revision: 1,
                provider_completions: Vec::new(),
                status: ReplicationOrderStatus::Pending,
            },
        }
    }

    fn registration_request<'a>(
        client: &'a HttpClient,
        base: &'a Url,
        config: &'a DeployClientConfig,
        owner: &'a AccountId,
    ) -> ManifestSubmitRequest<'a> {
        ManifestSubmitRequest {
            client,
            torii_base_url: base,
            network_id: &config.network_id,
            authority: owner,
            private_key: &config.private_key,
            alias_inputs: None,
        }
    }

    #[test]
    fn existing_pin_skips_registration_for_pending_and_completed_orders_and_refuses_substitution() {
        let config = publication_test_config();
        let manifest = super::super::tests::sample_manifest();
        let owner = AccountId::new(config.public_key.clone());
        let row = published_row(&config, &manifest);
        let encoded = |row: &SorafsPublicationPreparationV1| norito::encode_canonical(row).unwrap();
        let mut complete = row.clone();
        complete.order.status = ReplicationOrderStatus::Completed(8);
        let mut wrong_owner = row.clone();
        wrong_owner.pin.submitted_by = AccountId::new(
            KeyPair::try_from_seed(vec![99; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        );
        let mut wrong_manifest = row.clone();
        wrong_manifest.order.manifest_digest = ManifestDigest::new([0x99; 32]);
        let mut expired = row.clone();
        expired.order.status = ReplicationOrderStatus::Expired(8);
        let mut retired = row.clone();
        retired.pin.retire(9, None);
        let dir = tempfile::tempdir().unwrap();
        for (status, bytes, accepted) in [
            (202, Vec::new(), true),
            (200, encoded(&row), true),
            (200, encoded(&complete), true),
            (403, Vec::new(), false),
            (200, encoded(&wrong_owner), false),
            (200, encoded(&wrong_manifest), false),
            (200, encoded(&expired), false),
            (200, encoded(&retired), false),
            (200, b"invalid".to_vec(), false),
        ] {
            let (client, base, server) = status_server(vec![(
                status,
                "Content-Type: application/x-norito\r\n",
                bytes,
            )]);
            let session = PublicationSession::begin(&client, &base, &config);
            let request = registration_request(&client, &base, &config, &owner);
            assert_eq!(
                matches!(
                    register_if_absent(session, &request, &manifest, dir.path()),
                    Ok(Registration::Existing)
                ),
                accepted
            );
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].starts_with("POST /v1/sorafs/publish/prepare "));
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn interrupted_registration_checks_exact_hash_and_only_replays_its_retained_bytes_after_absence()
     {
        let config = publication_test_config();
        let owner = AccountId::new(config.public_key.clone());
        let manifest = super::super::tests::sample_manifest();
        let tx = build_pin_register_transaction(
            &config.network_id,
            &owner,
            &config.private_key,
            &manifest,
            None,
            None,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let retained = iroha_fs::OwnerDirectory::open(dir.path().canonicalize().unwrap()).unwrap();
        let name = registration_filename(
            &config.network_id,
            &ManifestDigest::from_manifest(&manifest).unwrap(),
        );
        let bytes = tx.encode_versioned();
        retained
            .write_private_atomic(&name, &bytes, iroha_fs::PublishMode::CreateNew)
            .unwrap();
        for observed in [true, false] {
            let mut replies = vec![(404, "", Vec::new())];
            if observed {
                let mut status = status_row("Queued", "queue");
                status.hash = tx.hash().to_string();
                replies.push((
                    200,
                    "Content-Type: application/json\r\n",
                    norito::json::to_vec(&status).unwrap(),
                ));
            } else {
                replies.push((
                    404,
                    "Content-Type: application/json\r\n",
                    absent_status(tx.hash(), "global"),
                ));
                replies.push((202, "Content-Type: application/json\r\n", b"{}".to_vec()));
            }
            let (client, base, server) = status_server(replies);
            let request = registration_request(&client, &base, &config, &owner);
            let result = register_if_absent(
                PublicationSession::begin(&client, &base, &config),
                &request,
                &manifest,
                dir.path(),
            )
            .unwrap();
            match result {
                Registration::Observed(hash) if observed => assert_eq!(hash, tx.hash()),
                Registration::Submitted(response) if !observed => {
                    assert_eq!(response.transaction_hash, tx.hash())
                }
                _ => panic!("unexpected publication registration"),
            }
            let requests = server.join().unwrap();
            assert!(requests[1].contains(&format!("hash={}&scope=global", tx.hash())));
            assert_eq!(requests.len(), if observed { 2 } else { 3 });
            assert_eq!(
                retained
                    .read_private_optional(&name, bytes.len())
                    .unwrap()
                    .unwrap()
                    .as_slice(),
                bytes
            );
            if !observed {
                assert!(requests[2].starts_with("POST /v1/sorafs/pin/register "));
            }
            let mut other = manifest.clone();
            other.pin_policy.retention_epoch += 1;
            assert!(retained_registration(&bytes, &request, &other).is_err());
            assert!(retained_registration(&bytes[..bytes.len() - 1], &request, &manifest).is_err());
        }
    }

    #[test]
    fn publication_session_starts_a_live_deadline_over_the_supplied_transport_and_signer() {
        let config = publication_test_config();
        let client = HttpClient::builder().no_proxy().build().unwrap();
        let base = Url::parse("http://127.0.0.1:1/").unwrap();
        let before = Instant::now();
        let session = PublicationSession::begin(&client, &base, &config);
        assert!(std::ptr::eq(session.client, &raw const client));
        assert!(std::ptr::eq(session.base, &raw const base));
        assert!(std::ptr::eq(session.config, &raw const config));
        assert!(before <= session.started && session.started <= Instant::now());
        assert!(ensure_live(session.started).is_ok());
    }

    #[test]
    fn pending_preparation_stops_on_bound_terminal_registration_without_polling_for_proof() {
        let config = publication_test_config();
        for pending in [202, 404] {
            for terminal in ["Rejected", "Expired"] {
                let (client, base, server) = status_server(vec![
                    (
                        pending,
                        "Content-Type: application/json\r\n",
                        b"{}".to_vec(),
                    ),
                    (
                        200,
                        "Content-Type: application/json\r\n",
                        norito::json::to_vec(&status_row(terminal, "state")).unwrap(),
                    ),
                ]);
                let error = preparation(
                    PublicationSession::begin(&client, &base, &config),
                    ManifestDigest::new([1; 32]),
                    Some(registration_hash(1)),
                    false,
                )
                .unwrap_err();
                assert!(
                    error.contains(&format!("state-resolved {terminal}")),
                    "{error}"
                );
                let requests = server.join().unwrap();
                assert_eq!(requests.len(), 2);
                assert!(requests[0].starts_with("POST /v1/sorafs/publish/prepare HTTP/1.1\r\n"));
                assert!(requests[1].starts_with("GET /v1/pipeline/transactions/status?"));
            }
        }
    }

    #[test]
    fn approved_or_applied_status_cannot_substitute_native_publication_preparation() {
        let config = publication_test_config();
        for kind in ["Approved", "Applied"] {
            let (client, base, server) = status_server(vec![
                (503, "Retry-After: 0\r\n", Vec::new()),
                (404, "Content-Type: application/json\r\n", b"{}".to_vec()),
                (
                    200,
                    "Content-Type: application/json\r\n",
                    norito::json::to_vec(&status_row(kind, "state")).unwrap(),
                ),
                (
                    200,
                    "Content-Type: application/x-norito\r\n",
                    b"ordinary status is not native publication preparation".to_vec(),
                ),
            ]);
            let error = preparation(
                PublicationSession::begin(&client, &base, &config),
                ManifestDigest::new([1; 32]),
                Some(registration_hash(1)),
                false,
            )
            .unwrap_err();
            assert!(error.contains("not canonical bounded Norito"), "{error}");
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), 4);
            assert!(requests[0].starts_with("POST /v1/sorafs/publish/prepare HTTP/1.1\r\n"));
            assert!(requests[1].starts_with("POST /v1/sorafs/publish/prepare HTTP/1.1\r\n"));
            assert!(requests[2].starts_with("GET /v1/pipeline/transactions/status?"));
            assert!(requests[3].starts_with("POST /v1/sorafs/publish/prepare HTTP/1.1\r\n"));
        }
    }

    #[test]
    fn proof_and_preparation_backoff_is_bounded_and_refusals_are_terminal() {
        use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
        let remaining = Duration::from_secs(2);
        let mut headers = HeaderMap::new();
        for status in [
            StatusCode::FORBIDDEN,
            StatusCode::CONFLICT,
            StatusCode::UNAUTHORIZED,
        ] {
            assert_eq!(publication_read_backoff(status, &headers, remaining), None);
        }
        assert_eq!(
            publication_read_backoff(StatusCode::SERVICE_UNAVAILABLE, &headers, remaining),
            Some(Duration::from_millis(500))
        );
        headers.insert(RETRY_AFTER, HeaderValue::from_static("17"));
        assert_eq!(
            publication_read_backoff(StatusCode::TOO_MANY_REQUESTS, &headers, remaining),
            Some(remaining)
        );
        assert_eq!(
            publication_read_backoff(StatusCode::SERVICE_UNAVAILABLE, &headers, Duration::ZERO),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn registration_status_deadline_refuses_before_starting_http() {
        let client = HttpClient::builder().no_proxy().build().unwrap();
        let base = Url::parse("http://127.0.0.1:1/").unwrap();
        let error = registration_observed(
            &client,
            &base,
            registration_hash(1),
            Instant::now() - PUBLICATION_TIMEOUT,
        )
        .unwrap_err();
        assert_eq!(
            error,
            "publication did not finalize before its ten-minute deadline"
        );
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
        assert!(
            load_checkpoint(
                temporary.path(),
                &network(1),
                &"publication-test".parse().unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn checkpoint_size_is_rejected_before_decode() {
        let directory = tempfile::tempdir().unwrap();
        let selected = directory.path().join("selected.to");
        let file = fs::File::create(&selected).unwrap();
        file.set_len(MAX_FINALITY_CHECKPOINT_BYTES as u64 + 1)
            .unwrap();
        let error = load_checkpoint(&selected, &network(1), &"publication-test".parse().unwrap())
            .unwrap_err();
        assert!(error.contains("maximum"), "{error}");
    }

    #[test]
    fn deployment_config_retains_independent_chain_label_for_checkpoint_import() {
        let key = KeyPair::try_from_seed(vec![4; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        let chain = "publication-independent-chain";
        let config = format!(
            "chain = '{chain}'\nnetwork_id = '{}'\n[account]\npublic_key = '{}'\nprivate_key = '{}'\nchain_discriminant = 777\n",
            network(1),
            key.public_key(),
            iroha_crypto::ExposedPrivateKey(key.private_key().clone())
                .try_to_multihash_string()
                .unwrap()
        );
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), config).unwrap();
        let parsed = load_deploy_client_config(file.path()).unwrap();
        assert_eq!(parsed.chain_id.to_string(), chain);
        assert_eq!(parsed.network_id, network(1));
        assert_eq!(parsed.chain_discriminant, 777);
    }

    #[test]
    fn deployment_config_resolves_discriminant_like_the_canonical_client() {
        let account = |body: &str| -> toml::Table {
            let root: toml::Table = format!("[account]\n{body}").parse().unwrap();
            root["account"].as_table().unwrap().clone()
        };
        for missing in ["", "profile = ' '"] {
            assert!(
                resolve_deploy_chain_discriminant(&account(missing))
                    .unwrap_err()
                    .contains("no default network"),
                "{missing:?}"
            );
        }
        assert!(
            resolve_deploy_chain_discriminant(&account("chain_discriminant = 0"))
                .unwrap_err()
                .contains("nonzero")
        );
        assert_eq!(
            resolve_deploy_chain_discriminant(&account("chain_discriminant = 777")).unwrap(),
            777
        );
        assert_eq!(
            resolve_deploy_chain_discriminant(&account("profile = 'taira'")).unwrap(),
            iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
        );
        assert!(
            resolve_deploy_chain_discriminant(&account(
                "profile = 'taira'\nchain_discriminant = 753"
            ))
            .unwrap_err()
            .contains("does not match profile")
        );
        assert!(
            resolve_deploy_chain_discriminant(&account("profile = 'unknown'"))
                .unwrap_err()
                .contains("not supported")
        );
    }
}
