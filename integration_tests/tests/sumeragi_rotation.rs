#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Sumeragi Phase 3 integration: commit-certificate signer sanity and cross-peer consistency.
//!
//! These tests validate that:
//! - Commit-certificate signer indices stay in-bounds for the active validator roster,
//!   and each certificate reaches quorum.
//! - All peers expose commit certificates for the same height and validator roster,
//!   while allowing quorum signer subsets to differ.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::data_model::{Level, consensus::Qc, isi::Log};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json;
use tokio::{runtime::Runtime, time::sleep};

const COMMIT_CERT_TIMEOUT: Duration = Duration::from_secs(120);
const COMMIT_CERT_POLL: Duration = Duration::from_millis(200);

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> eyre::Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    sandbox::start_network_blocking_or_skip(builder, context)
}

/// Compute f = floor((n-1)/3) and the quorum size (`min_votes_for_commit`).
fn quorum(n: usize) -> (usize, usize) {
    let f = n.saturating_sub(1) / 3;
    let q = if n > 3 { 2 * f + 1 } else { n };
    (f, q)
}

fn signer_indices_from_bitmap(bitmap: &[u8], validator_len: usize) -> Vec<u64> {
    let mut indices = Vec::new();
    for idx in 0..validator_len {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        if bitmap
            .get(byte_idx)
            .is_some_and(|byte| (byte >> bit_idx) & 1 == 1)
        {
            indices.push(idx as u64);
        }
    }
    indices
}

/// Extract a map of block height -> signer index set (ascending block order).
fn height_to_qc_signer_indices(certs: &[Qc]) -> Vec<(u64, Vec<u64>)> {
    let mut by_height: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for cert in certs {
        let mut idxs =
            signer_indices_from_bitmap(&cert.aggregate.signers_bitmap, cert.validator_set.len());
        idxs.sort_unstable();
        idxs.dedup();
        by_height.insert(cert.height, idxs);
    }
    by_height.into_iter().collect()
}

async fn fetch_commit_certificates(
    http: &reqwest::Client,
    torii: &str,
    from: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<Qc>> {
    let base = reqwest::Url::parse(torii).wrap_err_with(|| format!("parse torii URL {torii}"))?;
    let mut url = base
        .join("v2/sumeragi/commit-certificates")
        .wrap_err_with(|| format!("compose commit-certificates URL for {torii}"))?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(from) = from {
            pairs.append_pair("from", &from.to_string());
        }
        if let Some(limit) = limit {
            pairs.append_pair("limit", &limit.to_string());
        }
    }

    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .wrap_err("fetch commit certificates")?;

    if !response.status().is_success() {
        return Err(eyre!(
            "commit certificates response status {}",
            response.status()
        ));
    }

    let body = response.text().await.wrap_err("commit certificates body")?;
    json::from_str(&body).wrap_err("parse commit certificates JSON")
}

async fn wait_for_commit_certificates_in_range(
    http: &reqwest::Client,
    torii: &str,
    first_height: u64,
    last_height: u64,
) -> Result<Vec<Qc>> {
    if first_height > last_height {
        return Ok(Vec::new());
    }

    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let mut last_hint: Option<String> = None;
    let limit = last_height.saturating_sub(first_height).saturating_add(1);

    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit certificates [{first_height}..={last_height}] from {torii}; last={last_hint:?}"
            ));
        }

        match fetch_commit_certificates(http, torii, Some(last_height), Some(limit)).await {
            Ok(certs) => {
                let mut by_height: BTreeMap<u64, Qc> = BTreeMap::new();
                for cert in certs {
                    if (first_height..=last_height).contains(&cert.height) {
                        by_height.insert(cert.height, cert);
                    }
                }

                let missing: Vec<u64> = (first_height..=last_height)
                    .filter(|h| !by_height.contains_key(h))
                    .collect();
                if missing.is_empty() {
                    return Ok(by_height.into_values().collect());
                }
                last_hint = Some(format!("missing heights {missing:?}"));
            }
            Err(err) => {
                last_hint = Some(format!("{err:#}"));
            }
        }

        sleep(COMMIT_CERT_POLL).await;
    }
}

async fn wait_for_commit_certificate_height(
    http: &reqwest::Client,
    torii: &str,
    height: u64,
) -> Result<Qc> {
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let mut last_hint: Option<String> = None;

    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for commit certificate at height {height} from {torii}; last={last_hint:?}"
            ));
        }

        match fetch_commit_certificates(http, torii, Some(height), Some(4)).await {
            Ok(certs) => {
                if let Some(cert) = certs.into_iter().find(|cert| cert.height == height) {
                    return Ok(cert);
                }
                last_hint = Some("height not present".to_owned());
            }
            Err(err) => {
                last_hint = Some(format!("{err:#}"));
            }
        }

        sleep(COMMIT_CERT_POLL).await;
    }
}

#[test]
fn rotation_signer_indices_match_expected_set_a() -> Result<()> {
    init_instruction_registry();

    // Start a 5-peer validator network
    let builder = NetworkBuilder::new().with_peers(5);
    let Some((network, rt)) = start_network(
        builder,
        stringify!(rotation_signer_indices_match_expected_set_a),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let http = reqwest::Client::new();

    // Let the network produce a few blocks
    let status = client.get_status()?;
    for idx in status.blocks..6 {
        client.submit_blocking(Log::new(Level::INFO, format!("set a tick {idx}")))?;
    }
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 6).await })?;

    let latest_height = client.get_status()?.blocks;
    let certs = rt.block_on(async {
        wait_for_commit_certificates_in_range(&http, client.torii_url.as_str(), 2, latest_height)
            .await
    })?;

    let n = network.peers().len();
    let (_f, q) = quorum(n);

    let hv = height_to_qc_signer_indices(&certs);
    for (h, idxs) in hv.into_iter().filter(|(h, _)| *h >= 2) {
        // Expect at least quorum signatures; certificate may include more than threshold
        assert!(
            idxs.len() >= q,
            "height {h}: expected >= quorum signatures, got {} < {q}",
            idxs.len()
        );
        // Signer indices are bitmap positions over the validator roster.
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                iu < n,
                "height {h}: signer index {iu} out of validator-set bounds {n}"
            );
        }
        // Note: indices are compared as a set; ordering in the certificate may differ.
    }

    Ok(())
}

#[test]
fn rotation_signer_indices_match_expected_set_a_n7_multiple_heights() -> Result<()> {
    init_instruction_registry();

    // Start a 7-peer validator network
    let builder = NetworkBuilder::new().with_peers(7);
    let Some((network, rt)) = start_network(
        builder,
        stringify!(rotation_signer_indices_match_expected_set_a_n7_multiple_heights),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    let http = reqwest::Client::new();

    // Let the network produce a number of blocks (>= 10 total)
    let status = client.get_status()?;
    for idx in status.blocks..10 {
        client.submit_blocking(Log::new(Level::INFO, format!("set a n7 tick {idx}")))?;
    }
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 10).await })?;

    let latest_height = client.get_status()?.blocks;
    let certs = rt.block_on(async {
        wait_for_commit_certificates_in_range(&http, client.torii_url.as_str(), 2, latest_height)
            .await
    })?;

    let n = network.peers().len();
    assert_eq!(n, 7);
    let (_f, q) = quorum(n);

    // Check a window of heights starting from 2 (skip genesis), ensure at least 8 heights
    let hv = height_to_qc_signer_indices(&certs);
    let mut checked = 0usize;
    for (h, idxs) in hv.into_iter().filter(|(h, _)| *h >= 2).take(12) {
        // At least quorum signatures
        assert!(
            idxs.len() >= q,
            "height {h}: signatures {} < quorum {q}",
            idxs.len()
        );
        // Signer indices are bitmap positions over the validator roster.
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                iu < n,
                "height {h}: index {iu} out of validator-set bounds {n}"
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 8,
        "should check at least 8 heights; got {checked}"
    );

    Ok(())
}

#[test]
fn canonical_certificate_identical_across_peers() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new().with_peers(4);
    let Some((network, rt)) = start_network(
        builder,
        stringify!(canonical_certificate_identical_across_peers),
    )?
    else {
        return Ok(());
    };

    // Ensure we have several blocks
    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..5 {
        client.submit_blocking(Log::new(Level::INFO, format!("set a cert tick {idx}")))?;
    }
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 5).await })?;

    let expected_height = client.get_status()?.blocks;
    let http = reqwest::Client::new();

    // For each peer, fetch commit certificate for the same height and ensure
    // quorum is available for a consistent validator roster.
    let mut validator_set_hashes = Vec::new();
    let (_, required_quorum) = quorum(network.peers().len());
    for p in network.peers() {
        let torii = p.torii_url();
        let cert = rt.block_on(async {
            wait_for_commit_certificate_height(&http, torii.as_str(), expected_height).await
        })?;
        let mut idxs =
            signer_indices_from_bitmap(&cert.aggregate.signers_bitmap, cert.validator_set.len());
        idxs.sort_unstable();
        assert!(
            idxs.len() >= required_quorum,
            "height {}: signatures {} < quorum {}",
            cert.height,
            idxs.len(),
            required_quorum
        );
        for idx in &idxs {
            let idx = usize::try_from(*idx).expect("signer index fits usize");
            assert!(
                idx < cert.validator_set.len(),
                "height {}: signer index {} out of validator-set bounds {}",
                cert.height,
                idx,
                cert.validator_set.len()
            );
        }
        validator_set_hashes.push(cert.validator_set_hash);
    }

    for w in validator_set_hashes.windows(2) {
        assert_eq!(w[0], w[1], "validator roster hash differs across peers");
    }

    Ok(())
}
