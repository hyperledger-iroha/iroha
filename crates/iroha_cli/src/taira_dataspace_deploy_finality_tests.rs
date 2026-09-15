//! Native deployment trust-input checks; no HTTP or ledger mutation.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};

#[test]
fn deployment_trust_derives_exact_genesis_roster_and_network() {
    let trust = test_trust();
    let network = test_network_id();
    let authority = trust
        .authority(network)
        .expect("exact public genesis trust");
    assert_eq!(authority.network, network);
    assert_eq!(NetworkId::from_genesis_hash(authority.genesis), network);
    assert_eq!(authority.roster.len(), 4);
    assert_eq!(authority.pops.len(), 4);
    let mut expected = trust
        .peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        authority
            .roster
            .iter()
            .map(|member| member.validator.clone())
            .collect::<Vec<_>>(),
        expected
    );
    for (member, pop) in authority.roster.iter().zip(&authority.pops) {
        assert_eq!(member.power, 1);
        iroha_crypto::bls_normal_pop_verify(member.validator.public_key(), pop)
            .expect("native PoP");
    }
    assert_eq!(
        trust,
        test_trust(),
        "public fixture identity must be stable across calls"
    );
    let mut reordered = trust;
    reordered.peers.reverse();
    let reordered = reordered
        .authority(network)
        .expect("explicit peer observation order");
    assert_eq!(reordered.roster, authority.roster);
    assert_eq!(reordered.pops, authority.pops);
}

#[test]
fn deployment_trust_rejects_wrong_network_key_and_changed_genesis_wire() {
    let trust = test_trust();
    let network = test_network_id();
    let wrong_network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"other genesis")));
    assert!(trust.authority(wrong_network).is_err());
    let mut changed = trust.clone();
    changed.genesis_public_key = KeyPair::try_from_seed(vec![91; 32], Algorithm::Ed25519)
        .expect("other genesis key")
        .public_key()
        .clone();
    assert!(changed.authority(network).is_err());
    for wire in [
        String::new(),
        trust.genesis_signed_wire_hex.to_uppercase(),
        format!("{}00", trust.genesis_signed_wire_hex),
        "00".repeat(MAX_BYTES / 2 + 1),
    ] {
        let mut changed = trust.clone();
        changed.genesis_signed_wire_hex = wire;
        assert!(changed.authority(network).is_err());
    }
    let mut changed = trust;
    let mut wire = hex::decode(&changed.genesis_signed_wire_hex).expect("wire");
    let last = wire.last_mut().expect("nonempty genesis");
    *last ^= 1;
    changed.genesis_signed_wire_hex = hex::encode(wire);
    assert!(changed.authority(network).is_err());
}

#[test]
fn deployment_trust_requires_four_distinct_genesis_peers_and_public_endpoints() {
    let trust = test_trust();
    let network = test_network_id();
    let mut changed = trust.clone();
    changed.peers.pop();
    assert!(changed.authority(network).is_err());
    let mut changed = trust.clone();
    changed.peers.push(trust.peers[0].clone());
    assert!(changed.authority(network).is_err());
    let mut changed = trust.clone();
    changed.peers[1].peer_id = trust.peers[0].peer_id.clone();
    changed.peers[1].node_fingerprint = trust.peers[0].node_fingerprint;
    assert!(changed.authority(network).is_err());
    let mut changed = trust.clone();
    changed.peers[1].torii_origin = trust.peers[0].torii_origin.clone();
    assert!(changed.authority(network).is_err());
    let mut changed = trust.clone();
    let unknown =
        KeyPair::try_from_seed(vec![92; 32], Algorithm::BlsNormal).expect("foreign validator");
    changed.peers[0].peer_id = PeerId::new(unknown.public_key().clone());
    changed.peers[0].node_fingerprint = Hash::new(changed.peers[0].peer_id.encode());
    assert!(
        changed.authority(network).is_err(),
        "a self-consistent foreign peer cannot bootstrap trust"
    );
    let mut changed = trust.clone();
    changed.peers[0].node_fingerprint = Hash::new(b"foreign node fingerprint");
    assert!(changed.authority(network).is_err());
    for origin in [
        "file:///tmp/",
        "http://user:secret@127.0.0.1:8080/",
        "http://127.0.0.1:8080/?a=1",
        "http://127.0.0.1:8080/#fragment",
        "http://127.0.0.1:8080",
    ] {
        let mut changed = trust.clone();
        changed.peers[0].torii_origin = origin.to_owned();
        assert!(changed.authority(network).is_err(), "{origin}");
    }
}

#[test]
fn deployment_peer_reads_overlap_and_preserve_input_order() {
    use std::sync::{Arc, Condvar, Mutex, mpsc};

    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (started, arrivals) = mpsc::channel();
    let worker_gate = Arc::clone(&gate);
    let worker = std::thread::spawn(move || {
        read_four_peers(&[40, 30, 20, 10], 369, |index, input| {
            started.send(index).expect("observe each started peer");
            let (lock, ready) = &*worker_gate;
            let released = lock.lock().expect("release lock");
            let _released = ready
                .wait_while(released, |released| !*released)
                .expect("release wait");
            Ok(*input)
        })
    });
    let mut observed = Vec::new();
    for _ in 0..VERIFICATION_PEERS {
        match arrivals.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(index) => observed.push(index),
            Err(_) => break,
        }
    }
    // Release even on a failed overlap assertion, so serial regressions cannot deadlock.
    let (lock, ready) = &*gate;
    *lock.lock().expect("release lock") = true;
    ready.notify_all();
    let result = worker
        .join()
        .expect("peer coordinator")
        .expect("peer reads");
    observed.sort_unstable();
    assert_eq!(observed, vec![0, 1, 2, 3], "all four reads must overlap");
    assert_eq!(result, vec![40, 30, 20, 10]);
}

#[test]
fn deployment_peer_reads_reject_non_four_cardinality_before_dispatch() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let dispatched = AtomicUsize::new(0);
    for inputs in [vec![], vec![0; 3], vec![0; 5]] {
        let result = read_four_peers(&inputs, 369, |_, _| {
            dispatched.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert!(result.is_err(), "peer count {}", inputs.len());
    }
    assert_eq!(dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn deployment_peer_reads_join_all_workers_and_report_first_error() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let finished = AtomicUsize::new(0);
    let result = read_four_peers(&[(); 4], 369, |index, _| -> Result<()> {
        finished.fetch_add(1, Ordering::SeqCst);
        Err(eyre!("peer error {index}"))
    });
    assert_eq!(finished.load(Ordering::SeqCst), 4);
    assert_eq!(
        format!("{:#}", result.expect_err("all peers failed")),
        "validator 1 verification failed: peer error 0"
    );
}

#[test]
fn deployment_peer_reads_inherit_configured_address_profile() {
    use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};

    let _caller_profile = ChainDiscriminantGuard::enter(0);
    let profiles = read_four_peers(&[(); 4], 369, |_, _| Ok(chain_discriminant()))
        .expect("worker address profiles");
    assert_eq!(profiles, vec![369; 4]);
    assert_eq!(chain_discriminant(), 0, "caller override must remain local");
}

#[test]
fn deployment_peer_reads_recover_worker_panic_after_joining_all() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let finished = AtomicUsize::new(0);
    let result = read_four_peers(&[(); 4], 369, |index, _| {
        assert_ne!(index, 1, "injected peer worker panic");
        finished.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    assert_eq!(finished.load(Ordering::SeqCst), 3);
    assert_eq!(
        format!("{:#}", result.expect_err("second worker panicked")),
        "validator 2 verification failed: validator read worker panicked"
    );
}

#[test]
fn deployment_carrier_results_require_exact_bytes_before_publication() {
    let first = BTreeMap::from([(4, vec![1, 2]), (7, vec![3, 4])]);
    let same = BTreeMap::from([(4, vec![1, 2]), (10, vec![5, 6])]);
    let carriers = consistent_carriers([&first, &same]).expect("identical shared carrier");
    assert_eq!(carriers.len(), 3);
    assert_eq!(carriers[&4], &[1, 2]);
    assert_eq!(carriers[&7], &[3, 4]);
    assert_eq!(carriers[&10], &[5, 6]);
    let changed = BTreeMap::from([(4, vec![1, 9])]);
    let error = consistent_carriers([&first, &same, &changed])
        .expect_err("conflicting authenticated bytes cannot be published");
    assert_eq!(
        error.to_string(),
        "validators returned different canonical carrier bytes"
    );
}

fn peer_progress_status(scope: &str, kind: &str) -> Option<PipelineTransactionStatusResponse> {
    Some(PipelineTransactionStatusResponse {
        hash: "ab".repeat(32),
        scope: scope.into(),
        resolved_from: "state".into(),
        status: iroha_torii_shared::PipelineTransactionStatus {
            kind: kind.into(),
            block_height: (kind == "Applied").then_some(10),
        },
    })
}

#[test]
fn deployment_peer_progress_requires_valid_complete_status_pair() {
    let hash = "ab".repeat(32);
    let queued = peer_progress_status("global", "Queued");
    let local = peer_progress_status("local", "Applied");
    let mut invalid = Vec::new();
    invalid.push(peer_progress_status("local", "FutureStatus"));
    invalid.push(peer_progress_status("global", "Applied"));
    let mut wrong_hash = local.clone();
    wrong_hash.as_mut().unwrap().hash = "cd".repeat(32);
    invalid.push(wrong_hash);
    let mut wrong_source = local.clone();
    wrong_source.as_mut().unwrap().resolved_from = "other".into();
    invalid.push(wrong_source);
    for height in [None, Some(0)] {
        let mut wrong_height = local.clone();
        wrong_height.as_mut().unwrap().status.block_height = height;
        invalid.push(wrong_height);
    }
    for local in invalid {
        for global in [None, queued.clone()] {
            assert!(
                peer_carrier_progress("pending", &hash, &global, &local, 9).is_err(),
                "a pending or absent global response must not hide malformed local status"
            );
        }
    }
}

#[test]
fn deployment_peer_progress_retries_only_pending_or_newer_carrier() {
    let hash = "ab".repeat(32);
    let global = peer_progress_status("global", "Applied");
    let local = peer_progress_status("local", "Applied");
    assert!(matches!(
        peer_carrier_progress("pending", &hash, &None, &local, 9).unwrap(),
        PeerRead::Pending
    ));
    assert!(matches!(
        peer_carrier_progress("applied_verification_pending", &hash, &global, &local, 9).unwrap(),
        PeerRead::Pending
    ));
    for tip in [10, 11] {
        assert!(matches!(
            peer_carrier_progress("applied_verification_pending", &hash, &global, &local, tip)
                .unwrap(),
            PeerRead::Verified(10)
        ));
    }
    for kind in ["Rejected", "Expired"] {
        assert!(
            peer_carrier_progress(
                "failed",
                &hash,
                &None,
                &peer_progress_status("local", kind),
                9
            )
            .is_err()
        );
    }
    for state in ["failed", "unknown", "pending"] {
        assert!(peer_carrier_progress(state, &hash, &global, &local, 9).is_err());
    }
}

#[test]
fn deployment_peer_progress_never_masks_fixed_worker_errors() {
    for fixed_peer in [0, 1, 3] {
        let result = read_four_peers(&[(); 4], 369, |index, _| -> Result<PeerRead<()>> {
            if index == fixed_peer {
                Err(eyre!("fixed invalid proof"))
            } else {
                Ok(PeerRead::Pending)
            }
        });
        let error = match result {
            Ok(_) => panic!("peer progress hid a fixed error"),
            Err(error) => error,
        };
        assert_eq!(
            format!("{error:#}"),
            format!(
                "validator {} verification failed: fixed invalid proof",
                fixed_peer + 1
            )
        );
    }
}

fn typed_attestation_progress_error() -> eyre::Report {
    let key = KeyPair::try_from_seed(vec![96; 32], Algorithm::Ed25519).expect("reporter key");
    let node = PeerId::new(key.public_key().clone());
    let network = test_network_id();
    let response = iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
        requested_height: 10,
        applied_height: 9,
        status_height: 10,
        challenge: [17; 32],
        node_id: node.clone(),
        network_id: network,
    };
    iroha::client::BridgeFinalityAttestationTipMismatch::from_response(
        response,
        NonZeroU64::new(10).unwrap(),
        [17; 32],
        &node,
        network,
    )
    .expect("exact request-bound progress")
    .into()
}

#[test]
fn deployment_attestation_progress_retries_only_exact_sdk_type() {
    assert!(matches!(
        peer_attestation_progress(Ok(42_u8)).unwrap(),
        PeerRead::Verified(42)
    ));
    assert!(matches!(
        peer_attestation_progress::<()>(
            Err(typed_attestation_progress_error()).wrap_err("fresh validator attestation")
        )
        .unwrap(),
        PeerRead::Pending
    ));
    for message in [
        "404 query_validation_failed: Query not found in the live query store",
        "409 bridge_finality_attestation_tip_mismatch",
        "finality tip is changing: requested 10, applied 9, consensus status 10",
        "missing durable finality proof",
        "invalid finality attestation signature",
        "operation deadline exhausted",
    ] {
        let error = match peer_attestation_progress::<()>(Err(eyre!(message))) {
            Ok(_) => panic!("untyped error was retried: {message}"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn deployment_attestation_progress_joins_all_peers_and_preserves_fixed_errors() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for fixed_peer in [0, 1, 2, 3] {
        let reads = AtomicUsize::new(0);
        let result = read_four_peers(&[(); 4], 369, |index, _| {
            reads.fetch_add(1, Ordering::SeqCst);
            peer_attestation_progress::<()>(if index == fixed_peer {
                Err(eyre!("fixed malformed attestation"))
            } else {
                Err(typed_attestation_progress_error())
            })
        });
        assert_eq!(reads.load(Ordering::SeqCst), 4);
        let error = match result {
            Ok(_) => panic!("tip mismatch hid a fixed failure"),
            Err(error) => error,
        };
        assert_eq!(
            format!("{error:#}"),
            format!(
                "validator {} verification failed: fixed malformed attestation",
                fixed_peer + 1
            )
        );
    }
}
