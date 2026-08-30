#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Sumeragi Phase 3 integration: commit-certificate signer sanity and cross-peer consistency.
//!
//! These tests validate that:
//! - Commit-certificate signer indices stay in-bounds for the active validator roster,
//!   and each certificate reaches quorum.
//! - All peers expose commit certificates for the same height and validator roster,
//!   while allowing quorum signer subsets to differ.
use eyre::{Report, Result, WrapErr, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level, NetworkId,
        block::consensus_v2::{GlobalPhase, finality::V2FinalityArtifact},
        isi::Log,
    },
};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, time::sleep};
const COMMIT_CERT_TIMEOUT: Duration = Duration::from_secs(120);
const COMMIT_CERT_POLL: Duration = Duration::from_millis(200);
const ROTATION_NETWORK_START_ATTEMPTS: usize = 3;
const ROTATION_NETWORK_START_RETRY_DELAY: Duration = Duration::from_secs(1);
fn start_network(
    build: impl Fn() -> NetworkBuilder,
    context: &'static str,
) -> eyre::Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    for attempt in 1..=ROTATION_NETWORK_START_ATTEMPTS {
        match sandbox::start_network_blocking_or_skip(build(), context) {
            Ok(network) => return Ok(network),
            Err(err)
                if attempt < ROTATION_NETWORK_START_ATTEMPTS
                    && is_retryable_rotation_startup_error(&err) =>
            {
                eprintln!(
                    "warning: {context} network rebuild attempt {attempt}/{ROTATION_NETWORK_START_ATTEMPTS} failed after startup retries; retrying in {:?}: {err}",
                    ROTATION_NETWORK_START_RETRY_DELAY
                );
                std::thread::sleep(ROTATION_NETWORK_START_RETRY_DELAY);
            }
            Err(err) => return Err(err),
        }
    }
    unreachable!("rotation startup retry loop exits via return");
}
fn is_retryable_rotation_startup_error(err: &Report) -> bool {
    err.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("expected peers to start within timeout")
            || text.contains("peer startup failed; startup snapshot:")
    })
}
fn drive_network_to_total_height(
    network: &sandbox::SerializedNetwork,
    runtime: &Runtime,
    client: &iroha::client::Client,
    target_height: u64,
    label: &str,
) -> Result<()> {
    let mut current_height = client.get_status()?.blocks;
    while current_height < target_height {
        let next_height = current_height.saturating_add(1);
        client.submit_all(
            [Log::new(Level::INFO, format!("{label} {next_height}"))],
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        runtime.block_on(async {
            network
                .ensure_blocks_with(|height| height.total >= next_height)
                .await
        })?;
        current_height = client.get_status()?.blocks;
    }
    Ok(())
}
/// Compute f = floor((n-1)/3) and the quorum size (`min_votes_for_commit`).
fn quorum(n: usize) -> (usize, usize) {
    let f = n.saturating_sub(1) / 3;
    let q = if n > 3 { 2 * f + 1 } else { n };
    (f, q)
}
/// Extract a map of block height -> signer index set (ascending block order).
fn height_to_qc_signer_indices(certs: &[V2FinalityArtifact]) -> Vec<(u64, Vec<u64>)> {
    let mut by_height: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for cert in certs {
        let idxs = cert
            .commit_qc
            .signers
            .iter()
            .copied()
            .map(u64::from)
            .collect();
        by_height.insert(cert.height, idxs);
    }
    by_height.into_iter().collect()
}
async fn wait_for_commit_certificates_in_range(
    client: &Client,
    network_id: NetworkId,
    first_height: u64,
    last_height: u64,
) -> Result<Vec<V2FinalityArtifact>> {
    if first_height > last_height {
        return Ok(Vec::new());
    }
    let mut certificates = Vec::new();
    for height in first_height..=last_height {
        certificates.push(wait_for_commit_certificate_height(client, network_id, height).await?);
    }
    Ok(certificates)
}
async fn wait_for_commit_certificate_height(
    client: &Client,
    network_id: NetworkId,
    height: u64,
) -> Result<V2FinalityArtifact> {
    let height = NonZeroU64::new(height)
        .ok_or_else(|| eyre!("genesis height zero has no revision-4 finality artifact"))?;
    let deadline = Instant::now() + COMMIT_CERT_TIMEOUT;
    let mut last_hint: Option<String> = None;
    loop {
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for verified finality artifact at height {height}; last={last_hint:?}"
            ));
        }
        match client.get_bridge_finality_anchor(height, network_id) {
            Ok((proof, _)) => return Ok(proof.finality_artifact),
            Err(error) => last_hint = Some(format!("{error:#}")),
        }
        sleep(COMMIT_CERT_POLL).await;
    }
}
#[test]
fn rotation_signer_indices_match_expected_set_a() -> Result<()> {
    init_instruction_registry();
    // Start the second-smallest admissible revision-4 validator committee.
    let Some((network, rt)) = start_network(
        || NetworkBuilder::new().with_peers(7),
        stringify!(rotation_signer_indices_match_expected_set_a),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Let the network produce a few blocks
    drive_network_to_total_height(&network, &rt, &client, 6, "set a tick")?;
    let latest_height = client.get_status()?.blocks;
    let certs = rt.block_on(async {
        wait_for_commit_certificates_in_range(&client, network.network_id(), 2, latest_height).await
    })?;
    let n = network.peers().len();
    let (_f, q) = quorum(n);
    let hv = height_to_qc_signer_indices(&certs);
    for ((h, idxs), cert) in hv.into_iter().filter(|(h, _)| *h >= 2).zip(&certs) {
        assert_eq!(cert.commit_qc.phase, GlobalPhase::Commit);
        assert_eq!(cert.height_context.roster.len(), n);
        // The wire certificate carries the canonical exact threshold subset.
        assert!(
            idxs.len() == q,
            "height {h}: expected exactly {q} quorum signatures, got {}",
            idxs.len()
        );
        // Signer indices resolve directly through the frozen validator roster.
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                iu < n,
                "height {h}: signer index {iu} out of validator-set bounds {n}"
            );
        }
        assert!(
            idxs.windows(2).all(|pair| pair[0] < pair[1]),
            "height {h}: signer indices are not strictly increasing"
        );
    }
    Ok(())
}
#[test]
fn rotation_signer_indices_match_expected_set_a_n7_multiple_heights() -> Result<()> {
    init_instruction_registry();
    // Start a 7-peer validator network
    let Some((network, rt)) = start_network(
        || {
            NetworkBuilder::new()
                .with_peers(7)
                .with_sync_timeout(Duration::from_secs(300))
        },
        stringify!(rotation_signer_indices_match_expected_set_a_n7_multiple_heights),
    )?
    else {
        return Ok(());
    };
    let client = network.client();
    // Let the network produce a number of blocks (>= 10 total)
    drive_network_to_total_height(&network, &rt, &client, 10, "set a n7 tick")?;
    let latest_height = client.get_status()?.blocks;
    let certs = rt.block_on(async {
        wait_for_commit_certificates_in_range(&client, network.network_id(), 2, latest_height).await
    })?;
    let n = network.peers().len();
    assert_eq!(n, 7);
    let (_f, q) = quorum(n);
    // Check a window of heights starting from 2 (skip genesis), ensure at least 8 heights
    let hv = height_to_qc_signer_indices(&certs);
    let mut checked = 0usize;
    for ((h, idxs), cert) in hv.into_iter().filter(|(h, _)| *h >= 2).zip(&certs).take(12) {
        assert_eq!(cert.commit_qc.phase, GlobalPhase::Commit);
        assert_eq!(cert.height_context.roster.len(), n);
        // The wire certificate carries the canonical exact threshold subset.
        assert!(
            idxs.len() == q,
            "height {h}: signatures {} != exact quorum {q}",
            idxs.len()
        );
        // Signer indices resolve directly through the frozen validator roster.
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                iu < n,
                "height {h}: index {iu} out of validator-set bounds {n}"
            );
        }
        assert!(
            idxs.windows(2).all(|pair| pair[0] < pair[1]),
            "height {h}: signer indices are not strictly increasing"
        );
        checked += 1;
    }
    assert!(
        checked >= 8,
        "should check at least 8 heights; got {checked}"
    );
    Ok(())
}
#[test]
fn finality_context_identical_across_peers() -> Result<()> {
    init_instruction_registry();
    let Some((network, rt)) = start_network(
        || NetworkBuilder::new().with_peers(4),
        stringify!(finality_context_identical_across_peers),
    )?
    else {
        return Ok(());
    };
    // Ensure we have several blocks
    let client = network.client();
    drive_network_to_total_height(&network, &rt, &client, 5, "set a cert tick")?;
    let expected_height = client.get_status()?.blocks;
    // For each peer, fetch commit certificate for the same height and ensure
    // quorum is available for a consistent validator roster.
    let mut height_context_ids = Vec::new();
    let (_, required_quorum) = quorum(network.peers().len());
    for p in network.peers() {
        let peer_client = p.client();
        let cert = rt.block_on(async {
            wait_for_commit_certificate_height(&peer_client, network.network_id(), expected_height)
                .await
        })?;
        let idxs = &cert.commit_qc.signers;
        assert_eq!(cert.commit_qc.phase, GlobalPhase::Commit);
        assert!(
            idxs.len() == required_quorum,
            "height {}: signatures {} != exact quorum {}",
            cert.height,
            idxs.len(),
            required_quorum
        );
        for idx in idxs {
            let idx = usize::try_from(*idx).expect("signer index fits usize");
            assert!(
                idx < cert.height_context.roster.len(),
                "height {}: signer index {} out of validator-set bounds {}",
                cert.height,
                idx,
                cert.height_context.roster.len()
            );
        }
        assert!(
            idxs.windows(2).all(|pair| pair[0] < pair[1]),
            "height {}: signer indices are not strictly increasing",
            cert.height
        );
        height_context_ids.push(cert.height_context.id());
    }
    for pair in height_context_ids.windows(2) {
        assert_eq!(
            pair[0], pair[1],
            "frozen height context differs across peers"
        );
    }
    Ok(())
}
#[test]
fn rotation_startup_retry_filter_matches_startup_failures_only() {
    assert!(is_retryable_rotation_startup_error(&eyre!(
        "peer startup failed; startup snapshot: peer#0 stopped"
    )));
    assert!(is_retryable_rotation_startup_error(&eyre!(
        "expected peers to start within timeout"
    )));
    assert!(!is_retryable_rotation_startup_error(&eyre!(
        "commit certificate signatures below quorum"
    )));
}
