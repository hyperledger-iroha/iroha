//! Bounded canonical account transport and independent publication verification for real peers.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, ensure};
use iroha::{
    blocking::Client,
    crypto::{KeyPair, Signature},
};
use iroha_data_model::{
    block::consensus_v2::finality::V2FinalityArtifact,
    isi::sorafs::AssertSorafsPublicationV1,
    prelude::*,
    sorafs::{
        pin_registry::{ManifestDigest, ReplicationOrderStatus},
        publication::{
            PUBLICATION_PROOF_MAX_BYTES_V1, SorafsPublicationPreparationV1,
            SorafsPublicationProofRequestV1, SorafsPublicationProofV1,
            verify_sorafs_publication_v1,
        },
    },
};
use iroha_test_network::Network;
use sorafs_car::{
    CarBuildPlan,
    publisher::{
        PublisherSourceChunkRequestV1, PublisherSourceHeaderV1, PublisherSourceRequestV1,
        PublisherSourceUploadV1,
    },
};
use sorafs_manifest::ManifestV1;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, timeout_at};

static NONCE: AtomicU64 = AtomicU64::new(0);
pub(super) const DEADLINE: Duration = Duration::from_secs(180);

pub(super) fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .no_deflate()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()?)
}

pub(super) async fn bytes(mut response: reqwest::Response, maximum: usize) -> Result<Vec<u8>> {
    ensure!(
        !response
            .content_length()
            .is_some_and(|length| length > maximum as u64),
        "publication response length exceeds bound"
    );
    ensure!(
        !response
            .headers()
            .get("Content-Encoding")
            .is_some_and(|encoding| encoding != "identity"),
        "unexpected content encoding"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            chunk.len() <= maximum.saturating_sub(bytes.len()),
            "publication response exceeds bound"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) async fn post(
    http: &reqwest::Client,
    network: &NetworkId,
    base: &str,
    key: &KeyPair,
    path: &str,
    body: Vec<u8>,
) -> Result<reqwest::Response> {
    post_with_headers(
        http,
        network,
        base,
        key,
        path,
        body,
        "application/x-norito",
        None,
    )
    .await
}

/// Send exact canonical Norito JSON with the compliance route's request-bound replay key.
pub(super) async fn post_json(
    http: &reqwest::Client,
    network: &NetworkId,
    base: &str,
    key: &KeyPair,
    path: &str,
    body: Vec<u8>,
    idempotency_key: &str,
) -> Result<reqwest::Response> {
    post_with_headers(
        http,
        network,
        base,
        key,
        path,
        body,
        "application/json",
        Some(idempotency_key),
    )
    .await
}

async fn post_with_headers(
    http: &reqwest::Client,
    network: &NetworkId,
    base: &str,
    key: &KeyPair,
    path: &str,
    body: Vec<u8>,
    content_type: &str,
    idempotency_key: Option<&str>,
) -> Result<reqwest::Response> {
    let url = reqwest::Url::parse(base)?.join(path)?;
    let timestamp = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let nonce = format!(
        "publication-{timestamp}-{}",
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let message = iroha_torii::canonical_network_request_signature_message(
        network,
        &reqwest::Method::POST,
        &url.as_str().parse()?,
        &body,
        timestamp,
        &nonce,
    )?;
    let signature = Signature::try_new(key.private_key(), &message)?;
    let mut request = http
        .post(url)
        .header("Content-Type", content_type)
        .header("Accept-Encoding", "identity")
        .header(
            "X-Iroha-Account",
            AccountId::new(key.public_key().clone()).to_canonical_hex()?,
        )
        .header("X-Iroha-Signature", STANDARD.encode(signature.payload()))
        .header("X-Iroha-Timestamp-Ms", timestamp.to_string())
        .header("X-Iroha-Nonce", nonce)
        .body(body);
    if let Some(value) = idempotency_key {
        request = request.header("Idempotency-Key", value);
    }
    Ok(request.send().await?)
}

pub(super) fn decode<
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize + norito::NoritoSchema,
>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T> {
    ensure!(bytes.len() <= maximum, "publication carrier exceeds limit");
    Ok(norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(maximum, maximum, maximum, maximum * 4, 64),
    )?)
}

/// Start trust from the independently constructed signed genesis, never a queried arbitrary tip.
pub(super) async fn genesis_checkpoint(
    http: &reqwest::Client,
    network: &Network,
) -> Result<V2FinalityArtifact> {
    timeout_at(Instant::now() + DEADLINE, async {
        loop {
            let response = http
                .get(format!(
                    "{}/v1/ledger/state/1",
                    network.peers()[0].torii_url().trim_end_matches('/')
                ))
                .header("Accept", "application/json")
                .send()
                .await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND
                || response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            ensure!(
                response.status().is_success(),
                "genesis finality endpoint rejected: {}",
                response.status()
            );
            let value: norito::json::Value =
                norito::json::from_slice(&bytes(response, 4 * 1024 * 1024).await?)?;
            let artifact: V2FinalityArtifact =
                norito::json::from_value(value["finality_artifact"].clone())?;
            artifact.verify()?;
            artifact.validate_for_header(&network.genesis().0.header())?;
            ensure!(
                artifact.height == 1
                    && artifact.block_hash == network.genesis().0.hash()
                    && artifact.height_context.network_id == network.network_id(),
                "queried checkpoint changed independent signed-genesis identity"
            );
            ensure!(
                artifact.height_context.roster.len() == 4
                    && artifact.validator_set_pops.len() == 4
                    && artifact.commit_qc.signers.len() == 3,
                "qualification requires actual three-of-four finality"
            );
            return Ok(artifact);
        }
    })
    .await?
}

pub(super) async fn preparation(
    http: &reqwest::Client,
    network: &NetworkId,
    base: &str,
    publisher: &KeyPair,
    digest: ManifestDigest,
    complete: bool,
) -> Result<SorafsPublicationPreparationV1> {
    timeout_at(Instant::now() + DEADLINE, async {
        loop {
            let response = post(
                http,
                network,
                base,
                publisher,
                "v1/sorafs/publish/prepare",
                norito::to_bytes(&digest)?,
            )
            .await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND
                || response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
                || response.status() == reqwest::StatusCode::ACCEPTED
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            ensure!(
                response.status().is_success(),
                "preparation rejected: {}",
                response.status()
            );
            let row: SorafsPublicationPreparationV1 =
                decode(&bytes(response, 2 * 1024 * 1024).await?, 2 * 1024 * 1024)?;
            ensure!(
                row.pin.digest == digest
                    && row.pin.submitted_by == AccountId::new(publisher.public_key().clone()),
                "preparation identity substituted"
            );
            if complete && !matches!(row.order.status, ReplicationOrderStatus::Completed(_)) {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            return Ok(row);
        }
    })
    .await?
}

pub(super) async fn prove(
    http: &reqwest::Client,
    client: &Client,
    base: &str,
    publisher: &KeyPair,
    row: &SorafsPublicationPreparationV1,
    floor: &V2FinalityArtifact,
    completed: bool,
) -> Result<V2FinalityArtifact> {
    let deadline = Instant::now() + DEADLINE;
    let challenge = *blake3::hash(&norito::to_bytes(&(
        row.pin.digest,
        completed,
        NONCE.fetch_add(1, Ordering::Relaxed),
    ))?)
    .as_bytes();
    let signed = super::sorafs_network::prepare_transaction(
        client,
        [AssertSorafsPublicationV1 {
            manifest_digest: row.pin.digest,
            order_id: row.order.order_id,
            assignment_revision: row.order.assignment_revision,
            canonical_order_digest: *blake3::hash(&row.order.canonical_order).as_bytes(),
            require_complete: completed,
            challenge,
            minimum_height: floor.height,
            minimum_block_hash: *floor.block_hash.as_ref(),
        }],
        Metadata::default(),
    )
    .await?;
    client
        .account_client()
        .submit_transaction_and_wait(&signed)
        .await?;
    let selector = SorafsPublicationProofRequestV1 {
        entry_hash: *signed.hash_as_entrypoint().as_ref(),
        floor_height: floor.height,
        floor_block_hash: *floor.block_hash.as_ref(),
    };
    timeout_at(deadline, async {
        loop {
            let response = post(
                http,
                &floor.height_context.network_id,
                base,
                publisher,
                "v1/sorafs/publish/proof",
                norito::to_bytes(&selector)?,
            )
            .await?;
            if response.status() == reqwest::StatusCode::ACCEPTED {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            ensure!(
                response.status().is_success(),
                "publication proof rejected: {}",
                response.status()
            );
            let proof: SorafsPublicationProofV1 = decode(
                &bytes(response, PUBLICATION_PROOF_MAX_BYTES_V1).await?,
                PUBLICATION_PROOF_MAX_BYTES_V1,
            )?;
            let verified = verify_sorafs_publication_v1(
                &floor.height_context.network_id,
                floor,
                &signed,
                &proof,
            )?;
            ensure!(
                verified.completed() == completed,
                "publication phase substituted"
            );
            let mut malformed = proof.clone();
            malformed.executed_block.clear();
            ensure!(
                verify_sorafs_publication_v1(
                    &floor.height_context.network_id,
                    floor,
                    &signed,
                    &malformed
                )
                .is_err(),
                "unexecuted claim accepted"
            );
            return Ok(verified.finality().clone());
        }
    })
    .await?
}

pub(super) async fn stage(
    http: &reqwest::Client,
    network: &NetworkId,
    base: &str,
    publisher: &KeyPair,
    provider: [u8; 32],
    row: &SorafsPublicationPreparationV1,
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    payload: &[u8],
) -> Result<()> {
    let header = PublisherSourceHeaderV1::new(
        provider,
        *row.order.order_id.as_bytes(),
        row.order.assignment_revision,
        manifest,
        plan,
    )?;
    let header_digest = header.canonical_digest()?;
    let manifest_digest = *manifest.digest()?.as_bytes();
    let requests = std::iter::once(PublisherSourceRequestV1::Metadata(header)).chain(
        plan.chunks.iter().enumerate().map(|(index, chunk)| {
            PublisherSourceRequestV1::Chunk(PublisherSourceChunkRequestV1 {
                provider_id: provider,
                order_id: *row.order.order_id.as_bytes(),
                assignment_revision: row.order.assignment_revision,
                manifest_digest,
                header_digest,
                upload: PublisherSourceUploadV1 {
                    index: index as u32,
                    bytes: payload
                        [chunk.offset as usize..chunk.offset as usize + chunk.length as usize]
                        .to_vec(),
                },
            })
        }),
    );
    for request in requests {
        let response = post(
            http,
            network,
            base,
            publisher,
            "v1/sorafs/publish/source",
            norito::encode_canonical(&request)?,
        )
        .await?;
        ensure!(
            response.status() == reqwest::StatusCode::NO_CONTENT,
            "staging rejected: {}",
            response.status()
        );
    }
    Ok(())
}
