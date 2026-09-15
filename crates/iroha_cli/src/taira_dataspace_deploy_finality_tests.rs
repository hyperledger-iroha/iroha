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
