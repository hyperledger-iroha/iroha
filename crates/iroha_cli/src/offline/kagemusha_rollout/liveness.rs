//! Direct four-validator liveness collection after the production canary.

use super::*;
use iroha::data_model::{
    bridge::{BridgeFinalityAttestationV1, BridgeFinalityProof, BridgeFinalityVerifier},
    offline::{
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES,
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_BODY_SCHEMA,
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_BODY_SCHEMA,
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS,
        KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_OBSERVATION_SCHEMA,
        KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES,
        KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
        KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1,
        KagemushaV4PostCanaryValidatorLivenessChallengeV1,
        KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
        KagemushaV4PostCanaryValidatorLivenessEvidenceV1,
        KagemushaV4PostCanaryValidatorLivenessObservationV1,
        KagemushaV4PostCanaryValidatorLivenessTargetV1, KagemushaV4TairaCanaryEvidenceV1,
        KagemushaV4VerifiedTairaCanaryEvidenceV1,
    },
    peer::PeerId,
};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use reqwest::{
    StatusCode,
    blocking::{Client as HttpClient, Response as HttpResponse},
    header::{
        ACCEPT, ACCEPT_ENCODING, CACHE_CONTROL, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH,
        CONTENT_TYPE, HOST, HeaderMap, HeaderName, HeaderValue, TRANSFER_ENCODING,
    },
};
use std::{collections::BTreeMap, io::Read as _, thread};

const APPLICATION_NORITO: &str = "application/x-norito";
const FINALITY_CHALLENGE_HEADER: &str = "x-iroha-finality-challenge";
const STATUS_HINT_TIMEOUT: Duration = Duration::from_secs(10);
const ATTESTATION_TIMEOUT: Duration = Duration::from_secs(60);
const TIP_RACE_RETRY_DELAY: Duration = Duration::from_millis(200);

/// Collect and publish challenge-bound liveness from all four qualified validators.
#[derive(ClapArgs, Debug)]
pub(super) struct FinalizeValidatorLiveness {
    #[command(flatten)]
    trusted: TrustedInputs,
    /// Exact immutable issuer-signed activation-finality receipt.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    activation_receipt: PathBuf,
    /// Exact controller-signed canary authorization.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    canary_authorization: PathBuf,
    /// Exact immutable issuer-signed canary evidence.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    canary_evidence: PathBuf,
    /// Four exact `PEER_ID=https://dns-origin[:port]` mappings.
    #[arg(long = "validator", value_name = "PEER_ID=HTTPS_ORIGIN", num_args = 4)]
    validators: Vec<String>,
    /// Lifetime of the precommitted collection challenge, at most five minutes.
    #[arg(long, value_name = "MILLISECONDS", default_value = "300000")]
    collection_ttl_ms: NonZeroU64,
    /// Runtime-only owner-private receipt-issuer key file.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    issuer_private_key_file: PathBuf,
    /// Exact absent promotion-keyed liveness-evidence destination.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    output: PathBuf,
}

impl FinalizeValidatorLiveness {
    pub(super) fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        require_root()?;
        let loaded = load_verified_expectations(&self.trusted)?;
        let receipt = load_verified_receipt(&self.activation_receipt, &loaded)?;
        let authorization =
            load_verified_canary_authorization(&self.canary_authorization, &loaded, &receipt)?;
        let canary = load_verified_canary_evidence(
            &self.canary_evidence,
            &loaded,
            &receipt,
            &authorization,
        )?;
        require_rollout_state_path(
            &self.output,
            loaded.verified.binding().promotion_id,
            CANARY_VALIDATOR_LIVENESS_EVIDENCE_FILE_NAME,
        )?;
        preflight_root_owned_output(&self.output)?;

        let client = context.client_from_config();
        require_canary_client_binding(&client, &authorization)?;
        let issuer_key =
            load_root_custodied_key(&self.issuer_private_key_file, "receipt-issuer key")?;
        if issuer_key.public_key() != loaded.verified.receipt_issuer() {
            bail!("receipt-issuer key file does not match authenticated expectations");
        }
        let targets = parse_validator_targets(&self.validators, &loaded.verified)?;
        let ttl_ms = self.collection_ttl_ms.get();
        if ttl_ms > KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS {
            bail!(
                "validator-liveness collection lifetime exceeds the {} millisecond maximum",
                KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_MAX_INTERVAL_MS
            );
        }
        let challenge_path = rollout_state_path(
            loaded.verified.binding().promotion_id,
            CANARY_VALIDATOR_LIVENESS_CHALLENGE_FILE_NAME,
        )?;
        let challenge = load_or_publish_challenge(
            &challenge_path,
            &loaded.verified,
            &canary.verified,
            &canary.anchor,
            &canary.finality_proof,
            targets,
            ttl_ms,
            &issuer_key,
        )?;
        let endpoint_challenge = challenge
            .verify_bound(
                &loaded.verified,
                &canary.verified,
                &canary.anchor,
                &canary.finality_proof,
            )
            .wrap_err("validator-liveness challenge failed pre-dispatch trust verification")?;
        let response_headers = liveness_request_headers(&client.headers)?;
        let http = build_liveness_http_client(client.torii_request_timeout)?;
        let observations = collect_validator_observations(
            &client,
            &http,
            &response_headers,
            &challenge,
            endpoint_challenge,
            canary.anchor.canary_finalized_height,
        )?;
        let highest_tip = observations
            .iter()
            .map(|observation| {
                observation
                    .attestation
                    .body
                    .finality_proof
                    .finality_artifact
                    .height
            })
            .max()
            .expect("four validator observations are nonempty");
        let post_canary_finality_proof_chain = collect_shared_finality_chain(
            &client,
            &loaded.verified,
            &canary.finality_proof,
            highest_tip,
        )?;
        let body = KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1 {
            schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_EVIDENCE_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            challenge,
            endpoint_challenge,
            observations,
            post_canary_finality_proof_chain,
        };
        let evidence = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign(
            body,
            &issuer_key,
            &loaded.verified,
            &canary.verified,
            &canary.anchor,
            &canary.finality_proof,
        )
        .wrap_err("failed to sign complete four-validator liveness evidence")?;
        let bytes = norito::encode_canonical(&evidence)
            .wrap_err("failed to encode four-validator liveness evidence")?;
        let verified = evidence
            .verify_exact(
                &bytes,
                &loaded.verified,
                &canary.verified,
                &canary.anchor,
                &canary.finality_proof,
            )
            .wrap_err("new four-validator liveness evidence failed exact verification")?;
        publish_root_owned(&self.output, &bytes, |published| {
            let artifact =
                KagemushaV4PostCanaryValidatorLivenessEvidenceV1::decode_canonical(published)
                    .map_err(|error| error.to_string())?;
            artifact
                .verify_exact(
                    published,
                    &loaded.verified,
                    &canary.verified,
                    &canary.anchor,
                    &canary.finality_proof,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
        let report = norito::json!({
            "status": "finalized",
            "output": (self.output.display().to_string()),
            "byte_len": (u64::try_from(bytes.len()).unwrap_or(u64::MAX)),
            "sha256": (hex::encode(KagemushaExactBytesDigestV1::from_bytes(&bytes)?.sha256)),
            "promotion_id": (hex::encode(verified.promotion_id())),
            "canary_transaction_intent": (verified.canary_transaction_intent().to_string()),
            "canary_finalized_height": (verified.canary_finalized_height()),
            "highest_observed_tip_height": (verified.highest_observed_tip_height()),
            "endpoint_challenge": (hex::encode(verified.endpoint_challenge())),
            "validator_count": (u64::try_from(verified.validator_ids().len()).unwrap_or(u64::MAX)),
        });
        context.print_data(&report).map_err(|error| {
            eyre!(PublicationError::CommitUncertain {
                path: self.output,
                detail: format!("published validator-liveness report failed: {error}"),
            })
        })
    }
}

struct LoadedVerifiedCanaryEvidence {
    verified: KagemushaV4VerifiedTairaCanaryEvidenceV1,
    anchor: KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    finality_proof: BridgeFinalityProof,
}

fn load_verified_canary_evidence(
    path: &Path,
    loaded: &LoadedVerifiedExpectations,
    receipt: &LoadedVerifiedReceipt,
    authorization: &LoadedVerifiedCanaryAuthorization,
) -> Result<LoadedVerifiedCanaryEvidence> {
    require_rollout_state_path(
        path,
        loaded.verified.binding().promotion_id,
        CANARY_EVIDENCE_FILE_NAME,
    )?;
    let exact_bytes = read_root_private_artifact(
        path,
        KAGEMUSHA_V4_TAIRA_CANARY_EVIDENCE_MAX_BYTES,
        "canary evidence",
    )?;
    let artifact = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(&exact_bytes)
        .wrap_err("invalid canary evidence")?;
    let verified = artifact
        .verify_exact(
            &exact_bytes,
            &authorization.artifact,
            &authorization.exact_bytes,
            &loaded.verified,
            &receipt.artifact,
            &receipt.exact_bytes,
        )
        .wrap_err("canary evidence failed exact authenticated verification")?;
    let finality_proof = artifact
        .body
        .finality_proof_chain
        .last()
        .cloned()
        .ok_or_else(|| eyre!("canary evidence finality chain is empty"))?;
    let block_time = u64::try_from(finality_proof.block_header.creation_time().as_millis())
        .wrap_err("canary finalized block time does not fit u64 milliseconds")?;
    let anchor = KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1 {
        schema: iroha::data_model::offline::KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CANARY_ANCHOR_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        activation_finality_receipt: KagemushaExactBytesDigestV1::from_bytes(&receipt.exact_bytes)?,
        canary_authorization: verified.authorization_identity(),
        canary_transaction_intent: verified.canary_transaction_intent(),
        canary_transaction_wire: authorization.verified.canary_transaction_wire(),
        canary_finalized_height: verified.finalized_height(),
        canary_finalized_block_hash: verified.finalized_block_hash(),
        canary_finalized_block_time_unix_ms: block_time,
    };
    Ok(LoadedVerifiedCanaryEvidence {
        verified,
        anchor,
        finality_proof,
    })
}

fn parse_validator_targets(
    values: &[String],
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
) -> Result<[KagemushaV4PostCanaryValidatorLivenessTargetV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT]>
{
    if values.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT {
        bail!("exactly four validator identity/origin mappings are required");
    }
    let mut supplied = BTreeMap::new();
    for value in values {
        let (peer_text, origin) = value
            .split_once('=')
            .ok_or_else(|| eyre!("validator mapping must be PEER_ID=HTTPS_ORIGIN"))?;
        if peer_text.is_empty() || origin.is_empty() || origin.contains('=') {
            bail!("validator mapping must contain one nonempty identity and origin");
        }
        let validator_id: PeerId = peer_text
            .parse()
            .map_err(|_| eyre!("validator mapping contains a noncanonical peer id"))?;
        if validator_id.to_string() != peer_text {
            bail!("validator peer id is not in canonical text form");
        }
        let parsed = url::Url::parse(&format!("{origin}/"))
            .map_err(|_| eyre!("validator Torii origin is not a canonical HTTPS URL"))?;
        if canonical_torii_origin(&parsed)? != origin {
            bail!("validator Torii origin is not in exact canonical form");
        }
        if supplied.insert(validator_id, origin.to_owned()).is_some() {
            bail!("validator mappings contain a duplicate peer id");
        }
    }
    let mut targets = Vec::with_capacity(KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT);
    for body in expectations.validator_bodies() {
        let origin = supplied
            .remove(&body.validator_id)
            .ok_or_else(|| eyre!("validator mappings do not match the qualified roster"))?;
        targets.push(KagemushaV4PostCanaryValidatorLivenessTargetV1 {
            validator_id: body.validator_id.clone(),
            canonical_torii_origin: origin,
        });
    }
    if !supplied.is_empty()
        || targets
            .windows(2)
            .any(|pair| pair[0].validator_id >= pair[1].validator_id)
        || targets.iter().enumerate().any(|(index, target)| {
            targets[..index]
                .iter()
                .any(|prior| prior.canonical_torii_origin == target.canonical_torii_origin)
        })
    {
        bail!("validator mappings are not one-to-one with the ordered qualified roster");
    }
    targets
        .try_into()
        .map_err(|_| eyre!("validator target cardinality changed during parsing"))
}

fn load_or_publish_challenge(
    path: &Path,
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    verified_canary: &KagemushaV4VerifiedTairaCanaryEvidenceV1,
    canary_anchor: &KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    canary_finality_proof: &BridgeFinalityProof,
    targets: [KagemushaV4PostCanaryValidatorLivenessTargetV1;
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
    ttl_ms: u64,
    issuer: &KeyPair,
) -> Result<KagemushaV4PostCanaryValidatorLivenessChallengeV1> {
    let existing = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).wrap_err("inspect validator-liveness challenge journal"),
        Ok(_) => Some(read_root_private_artifact(
            path,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
            "validator-liveness challenge journal",
        )?),
    };
    let challenge = if let Some(bytes) = existing {
        KagemushaV4PostCanaryValidatorLivenessChallengeV1::decode_canonical(&bytes)
            .wrap_err("invalid validator-liveness challenge journal")?
    } else {
        let issued_at_unix_ms = current_unix_ms()?;
        if issued_at_unix_ms <= canary_anchor.canary_finalized_block_time_unix_ms {
            bail!("protected-host clock does not strictly follow the finalized canary block");
        }
        let expires_at_unix_ms = issued_at_unix_ms
            .checked_add(ttl_ms)
            .ok_or_else(|| eyre!("validator-liveness challenge expiry overflow"))?;
        let body = KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1 {
            schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            binding: expectations.binding().clone(),
            canary_anchor: canary_anchor.clone(),
            targets: targets.clone(),
            issuer: expectations.receipt_issuer().clone(),
            nonce: random_nonzero_challenge_nonce()?,
            issued_at_unix_ms,
            expires_at_unix_ms,
        };
        let challenge = KagemushaV4PostCanaryValidatorLivenessChallengeV1::try_sign(body, issuer)
            .wrap_err("failed to sign validator-liveness challenge")?;
        let bytes = norito::encode_canonical(&challenge)
            .wrap_err("failed to encode validator-liveness challenge")?;
        publish_root_owned(path, &bytes, |published| {
            let decoded =
                KagemushaV4PostCanaryValidatorLivenessChallengeV1::decode_canonical(published)
                    .map_err(|error| error.to_string())?;
            if decoded != challenge {
                return Err("published validator-liveness challenge changed".to_owned());
            }
            decoded
                .verify_bound(
                    expectations,
                    verified_canary,
                    canary_anchor,
                    canary_finality_proof,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
        let persisted = read_root_private_artifact(
            path,
            KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_CHALLENGE_MAX_BYTES,
            "validator-liveness challenge journal",
        )?;
        if persisted != bytes {
            bail!("validator-liveness challenge journal changed after publication");
        }
        KagemushaV4PostCanaryValidatorLivenessChallengeV1::decode_canonical(&persisted)
            .wrap_err("published validator-liveness challenge failed reread")?
    };
    let interval = challenge
        .body
        .expires_at_unix_ms
        .checked_sub(challenge.body.issued_at_unix_ms)
        .ok_or_else(|| eyre!("validator-liveness challenge interval underflow"))?;
    if challenge.body.binding != *expectations.binding()
        || challenge.body.canary_anchor != *canary_anchor
        || challenge.body.targets != targets
        || challenge.body.issuer != *expectations.receipt_issuer()
        || interval != ttl_ms
    {
        bail!("validator-liveness challenge journal differs from the requested collection");
    }
    challenge
        .verify_bound(
            expectations,
            verified_canary,
            canary_anchor,
            canary_finality_proof,
        )
        .wrap_err("validator-liveness challenge journal failed authenticated verification")?;
    let now = current_unix_ms()?;
    if now < challenge.body.issued_at_unix_ms || now >= challenge.body.expires_at_unix_ms {
        bail!("validator-liveness challenge journal is not currently usable");
    }
    Ok(challenge)
}

fn random_nonzero_challenge_nonce() -> Result<[u8; 32]> {
    for _ in 0..2 {
        let mut nonce = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|error| eyre!("validator-liveness OS RNG failed: {error}"))?;
        if nonce != [0; 32] {
            return Ok(nonce);
        }
    }
    bail!("validator-liveness OS RNG returned an all-zero nonce repeatedly")
}

fn liveness_request_headers(
    configured: &std::collections::HashMap<String, String>,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (name, value) in configured {
        if [
            ACCEPT.as_str(),
            ACCEPT_ENCODING.as_str(),
            CONTENT_TYPE.as_str(),
            CONTENT_ENCODING.as_str(),
            CONTENT_LENGTH.as_str(),
            TRANSFER_ENCODING.as_str(),
            CONNECTION.as_str(),
            HOST.as_str(),
            FINALITY_CHALLENGE_HEADER,
        ]
        .iter()
        .any(|reserved| name.eq_ignore_ascii_case(reserved))
        {
            continue;
        }
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| eyre!("configured validator-liveness header name is invalid"))?;
        let value = value
            .parse::<HeaderValue>()
            .map_err(|_| eyre!("configured validator-liveness header value is invalid"))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn build_liveness_http_client(configured_timeout: Duration) -> Result<HttpClient> {
    let timeout = if configured_timeout == Duration::ZERO {
        ATTESTATION_TIMEOUT
    } else {
        configured_timeout.min(ATTESTATION_TIMEOUT)
    };
    HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .connect_timeout(timeout.min(STATUS_HINT_TIMEOUT))
        .timeout(timeout)
        .build()
        .wrap_err("failed to build direct validator-liveness HTTPS client")
}

fn collect_validator_observations(
    base_client: &Client,
    http: &HttpClient,
    headers: &HeaderMap,
    challenge: &KagemushaV4PostCanaryValidatorLivenessChallengeV1,
    endpoint_challenge: [u8; 32],
    canary_height: u64,
) -> Result<
    [KagemushaV4PostCanaryValidatorLivenessObservationV1; KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT],
> {
    let outcomes = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT);
        for target in challenge.body.targets.clone() {
            let client = base_client.clone();
            let http = http.clone();
            let headers = headers.clone();
            let expires_at_unix_ms = challenge.body.expires_at_unix_ms;
            handles.push(scope.spawn(move || {
                collect_validator_observation(
                    client,
                    http,
                    headers,
                    target,
                    endpoint_challenge,
                    canary_height,
                    expires_at_unix_ms,
                )
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| eyre!("validator-liveness collector thread panicked"))?
            })
            .collect::<Result<Vec<_>>>()
    })?;
    outcomes
        .try_into()
        .map_err(|_| eyre!("validator observation cardinality changed during collection"))
}

fn collect_validator_observation(
    mut client: Client,
    http: HttpClient,
    headers: HeaderMap,
    target: KagemushaV4PostCanaryValidatorLivenessTargetV1,
    endpoint_challenge: [u8; 32],
    canary_height: u64,
    expires_at_unix_ms: u64,
) -> Result<KagemushaV4PostCanaryValidatorLivenessObservationV1> {
    client.torii_url = url::Url::parse(&format!("{}/", target.canonical_torii_origin))
        .wrap_err("failed to construct direct validator Torii URL")?;
    client.torii_request_timeout = if client.torii_request_timeout == Duration::ZERO {
        STATUS_HINT_TIMEOUT
    } else {
        client.torii_request_timeout.min(STATUS_HINT_TIMEOUT)
    };
    loop {
        if current_unix_ms()? >= expires_at_unix_ms {
            bail!("validator-liveness challenge expired before all validators responded");
        }
        let height = client
            .get_status()
            .wrap_err("failed to read direct validator status hint")?
            .blocks;
        if height < canary_height {
            bail!("validator durable-tip hint is behind the finalized canary");
        }
        let Some(height) = NonZeroU64::new(height) else {
            bail!("validator durable-tip hint is zero");
        };
        match fetch_validator_attestation(
            &http,
            &headers,
            &target,
            endpoint_challenge,
            height,
            expires_at_unix_ms,
        )? {
            AttestationFetch::TipRace => {
                thread::sleep(TIP_RACE_RETRY_DELAY);
            }
            AttestationFetch::Complete {
                request_started_at_unix_ms,
                response_completed_at_unix_ms,
                exact_bytes,
                attestation,
            } => {
                if attestation.body.node_id != target.validator_id
                    || attestation.body.challenge != endpoint_challenge
                    || attestation.body.finality_proof.finality_artifact.height != height.get()
                    || height.get() < canary_height
                {
                    bail!("validator attestation differs from its challenged identity or tip");
                }
                attestation
                    .verify()
                    .map_err(|_| eyre!("validator attestation signature is invalid"))?;
                return Ok(KagemushaV4PostCanaryValidatorLivenessObservationV1 {
                    schema: KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_OBSERVATION_SCHEMA
                        .to_owned(),
                    version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
                    target,
                    request_started_at_unix_ms,
                    response_completed_at_unix_ms,
                    attestation_response_norito: KagemushaExactBytesDigestV1::from_bytes(
                        &exact_bytes,
                    )?,
                    attestation,
                });
            }
        }
    }
}

enum AttestationFetch {
    TipRace,
    Complete {
        request_started_at_unix_ms: u64,
        response_completed_at_unix_ms: u64,
        exact_bytes: Vec<u8>,
        attestation: BridgeFinalityAttestationV1,
    },
}

fn fetch_validator_attestation(
    http: &HttpClient,
    headers: &HeaderMap,
    target: &KagemushaV4PostCanaryValidatorLivenessTargetV1,
    challenge: [u8; 32],
    height: NonZeroU64,
    expires_at_unix_ms: u64,
) -> Result<AttestationFetch> {
    let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
        .path()
        .replace("{height}", &height.get().to_string());
    let url = url::Url::parse(&format!("{}{path}", target.canonical_torii_origin))
        .wrap_err("failed to construct validator finality-attestation URL")?;
    let request_started_at_unix_ms = current_unix_ms()?;
    if request_started_at_unix_ms >= expires_at_unix_ms {
        bail!("validator-liveness challenge expired before request dispatch");
    }
    let response = http
        .get(url.clone())
        .headers(headers.clone())
        .header(ACCEPT, APPLICATION_NORITO)
        .header(ACCEPT_ENCODING, "identity")
        .header(FINALITY_CHALLENGE_HEADER, hex::encode(challenge))
        .send()
        .wrap_err("validator finality-attestation request failed")?;
    if response.url() != &url {
        bail!("validator finality-attestation response changed origin or path");
    }
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(AttestationFetch::TipRace);
    }
    let exact_bytes = read_attestation_response(response)?;
    let response_completed_at_unix_ms = current_unix_ms()?;
    if response_completed_at_unix_ms >= expires_at_unix_ms {
        bail!("validator finality-attestation completed after challenge expiry");
    }
    let attestation: BridgeFinalityAttestationV1 = norito::decode_canonical_with_limits(
        &exact_bytes,
        norito::canonical_decode_limits(exact_bytes.len()),
    )
    .map_err(|error| eyre!("invalid canonical validator finality attestation: {error}"))?;
    if norito::encode_canonical(&attestation)
        .wrap_err("failed to re-encode validator finality attestation")?
        != exact_bytes
    {
        bail!("validator finality-attestation response is not exact canonical Norito");
    }
    Ok(AttestationFetch::Complete {
        request_started_at_unix_ms,
        response_completed_at_unix_ms,
        exact_bytes,
        attestation,
    })
}

fn read_attestation_response(mut response: HttpResponse) -> Result<Vec<u8>> {
    if response.status() != StatusCode::OK {
        bail!(
            "validator finality-attestation returned HTTP status {}",
            response.status()
        );
    }
    let mut content_types = response.headers().get_all(CONTENT_TYPE).iter();
    let content_type = content_types
        .next()
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if content_type != APPLICATION_NORITO || content_types.next().is_some() {
        bail!("validator finality-attestation has an invalid Content-Type");
    }
    if response.headers().contains_key(CONTENT_ENCODING) {
        bail!("validator finality-attestation must not carry Content-Encoding");
    }
    let has_no_store = response
        .headers()
        .get_all(CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|directive| directive.trim().eq_ignore_ascii_case("no-store"));
    if !has_no_store {
        bail!("validator finality-attestation is missing Cache-Control: no-store");
    }
    if response.content_length().is_some_and(|length| {
        length
            > u64::try_from(KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES)
                .expect("attestation byte limit fits u64")
    }) {
        bail!("validator finality-attestation exceeds its fixed byte ceiling");
    }
    let maximum = KAGEMUSHA_V4_POST_CANARY_VALIDATOR_LIVENESS_ATTESTATION_MAX_BYTES;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(16 * 1024)
            .min(maximum),
    )?;
    response
        .by_ref()
        .take(u64::try_from(maximum)?.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err("failed to read bounded validator finality-attestation response")?;
    if bytes.is_empty() || bytes.len() > maximum {
        bail!("validator finality-attestation response is empty or oversized");
    }
    Ok(bytes)
}

fn collect_shared_finality_chain(
    client: &Client,
    expectations: &KagemushaV4ActivationReceiptExpectationsV1,
    canary_proof: &BridgeFinalityProof,
    highest_tip: u64,
) -> Result<Vec<BridgeFinalityProof>> {
    let canary_height = canary_proof.finality_artifact.height;
    let proof_count = highest_tip
        .checked_sub(canary_height)
        .ok_or_else(|| eyre!("validator tip precedes the finalized canary"))?;
    if proof_count
        > u64::try_from(KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1)
            .expect("proof bound fits u64")
    {
        bail!("validator tips exceed the bounded post-canary finality corridor");
    }
    require_qualified_finality_context(canary_proof, expectations)?;
    let mut verifier = BridgeFinalityVerifier::with_context(
        expectations.binding().network_id.clone(),
        canary_proof.finality_artifact.context_id(),
    );
    verifier
        .verify(canary_proof)
        .map_err(|error| eyre!("canary finality anchor failed verification: {error}"))?;
    let mut proofs = Vec::with_capacity(usize::try_from(proof_count)?);
    if proof_count == 0 {
        return Ok(proofs);
    }
    let first_successor = canary_height
        .checked_add(1)
        .ok_or_else(|| eyre!("canary finality height overflow"))?;
    for height in first_successor..=highest_tip {
        let height =
            NonZeroU64::new(height).ok_or_else(|| eyre!("post-canary finality height is zero"))?;
        let proof = client.get_next_bridge_finality_proof(height, &mut verifier)?;
        require_qualified_finality_context(&proof, expectations)?;
        proofs.push(proof);
    }
    Ok(proofs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liveness_headers_replace_transport_control_fields() {
        let configured = std::collections::HashMap::from([
            ("X-API-Token".to_owned(), "runtime-secret".to_owned()),
            ("Accept".to_owned(), "text/plain".to_owned()),
            (FINALITY_CHALLENGE_HEADER.to_owned(), "stale".to_owned()),
            ("Accept-Encoding".to_owned(), "gzip".to_owned()),
        ]);
        let headers = liveness_request_headers(&configured).expect("headers parse");
        assert_eq!(
            headers
                .get("x-api-token")
                .and_then(|value| value.to_str().ok()),
            Some("runtime-secret")
        );
        assert!(!headers.contains_key(ACCEPT));
        assert!(!headers.contains_key(ACCEPT_ENCODING));
        assert!(!headers.contains_key(FINALITY_CHALLENGE_HEADER));
    }

    #[test]
    fn liveness_nonce_is_nonzero() {
        assert_ne!(
            random_nonzero_challenge_nonce().expect("OS RNG available"),
            [0; 32]
        );
    }
}
