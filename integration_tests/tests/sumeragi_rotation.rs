#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Sumeragi Phase 3 integration: Set A window invariants and canonical certificates.
//!
//! These tests validate that:
//! - The signer indices in committed blocks stay within Set A (the first `q` indices)
//!   for the view-rotated topology, with leader at index 0 and proxy tail at `q - 1`.
//! - All peers converge on the same canonical certificate (identical signer index set).

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    Level, block::SignedBlock, isi::Log, prelude::*, query::block::prelude::FindBlocks,
};
use iroha_primitives::unique_vec::UniqueVec;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use tokio::runtime::Runtime;

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

/// Set A indices for the view-rotated topology: leader at index 0 and proxy tail at `q - 1`.
fn expected_set_a_indices(n: usize) -> (usize, usize) {
    let (_f, q) = quorum(n);
    let leader_idx = 0;
    let tail_idx = q.saturating_sub(1);
    (leader_idx, tail_idx)
}

/// Extract a map of block height -> signer index set (ascending block order).
fn height_to_signer_indices(blocks: &[SignedBlock]) -> Vec<(u64, Vec<u64>)> {
    let mut v = Vec::new();
    for b in blocks {
        let h = b.header().height().get();
        let mut idxs: Vec<u64> = b
            .signatures()
            .map(iroha::data_model::block::BlockSignature::index)
            .collect();
        idxs.sort_unstable();
        idxs.dedup();
        v.push((h, idxs));
    }
    v
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

    // Let the network produce a few blocks
    let status = client.get_status()?;
    for idx in status.blocks..6 {
        client.submit_blocking(Log::new(Level::INFO, format!("set a tick {idx}")))?;
    }
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 6).await })?;

    // Fetch all blocks from a single peer (descending) and reverse to ascending
    let mut blocks = client.query(FindBlocks).execute_all()?;
    blocks.reverse();

    // Build initial ordered peer list from the network view
    let peers: UniqueVec<Peer> = network
        .peers()
        .iter()
        .map(|p| Peer::new(p.p2p_address(), p.id()))
        .collect();
    let n = peers.len();
    let (_f, q) = quorum(n);

    // Check heights >= 2 (skip genesis at height=1)
    let hv = height_to_signer_indices(&blocks);
    for (h, idxs) in hv.into_iter().filter(|(h, _)| *h >= 2) {
        let (leader_abs, tail_abs) = expected_set_a_indices(n);
        // Expect at least quorum signatures; certificate may include more than threshold
        assert!(
            idxs.len() >= q,
            "height {h}: expected >= quorum signatures, got {} < {q}",
            idxs.len()
        );
        // Expect all indices lie within Set A window [0..q)
        let set_a: Vec<usize> = (0..q).collect();
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                set_a.contains(&iu),
                "height {h}: signer index {iu} not in Set A {set_a:?}"
            );
        }
        // Leader and proxy tail must be in the signer set
        assert!(
            idxs.contains(&(leader_abs as u64)),
            "height {h}: leader missing"
        );
        assert!(
            idxs.contains(&(tail_abs as u64)),
            "height {h}: proxy tail missing"
        );
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

    // Let the network produce a number of blocks (>= 10 total)
    let status = client.get_status()?;
    for idx in status.blocks..10 {
        client.submit_blocking(Log::new(Level::INFO, format!("set a n7 tick {idx}")))?;
    }
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 10).await })?;

    // Fetch all blocks (descending) and reverse to ascending
    let mut blocks = client.query(FindBlocks).execute_all()?;
    blocks.reverse();

    let peers: UniqueVec<Peer> = network
        .peers()
        .iter()
        .map(|p| Peer::new(p.p2p_address(), p.id()))
        .collect();
    let n = peers.len();
    assert_eq!(n, 7);
    let (_f, q) = quorum(n);

    // Check a window of heights starting from 2 (skip genesis), ensure at least 8 heights
    let hv = height_to_signer_indices(&blocks);
    let mut checked = 0usize;
    for (h, idxs) in hv.into_iter().filter(|(h, _)| *h >= 2).take(12) {
        let (leader_abs, tail_abs) = expected_set_a_indices(n);
        // At least quorum signatures
        assert!(
            idxs.len() >= q,
            "height {h}: signatures {} < quorum {q}",
            idxs.len()
        );
        // All indices within Set A window [0..q)
        let set_a: Vec<usize> = (0..q).collect();
        for ix in &idxs {
            let iu = usize::try_from(*ix).unwrap();
            assert!(
                set_a.contains(&iu),
                "height {h}: index {iu} not in Set A {set_a:?}"
            );
        }
        // Must include leader and proxy tail for this height
        assert!(
            idxs.contains(&(leader_abs as u64)),
            "height {h}: leader missing"
        );
        assert!(
            idxs.contains(&(tail_abs as u64)),
            "height {h}: proxy tail missing"
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

    // For each peer, fetch latest block and collect signer index sets. All must match.
    let mut latest_sets: Vec<Vec<u64>> = Vec::new();
    for p in network.peers() {
        let mut blocks = p.client().query(FindBlocks).execute_all()?;
        // blocks[0] is latest due to descending order
        let latest = blocks.remove(0);
        let mut idxs: Vec<u64> = latest
            .signatures()
            .map(iroha::data_model::block::BlockSignature::index)
            .collect();
        idxs.sort_unstable();
        latest_sets.push(idxs);
    }
    for w in latest_sets.windows(2) {
        assert_eq!(w[0], w[1], "canonical certificate differs across peers");
    }

    Ok(())
}
