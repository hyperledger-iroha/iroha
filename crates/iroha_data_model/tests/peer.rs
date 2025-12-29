//! Tests for peer serialization
#![cfg(feature = "json")]

use iroha_crypto::KeyPair;
use iroha_data_model::prelude::Peer;
use iroha_primitives::addr::SocketAddr;

#[test]
fn peer_serialization_roundtrip() {
    let key_pair = KeyPair::random();
    let address: SocketAddr = "127.0.0.1:8080".parse().expect("valid address");
    let peer = Peer::new(address, key_pair.public_key().clone());

    let json = norito::json::to_json(&peer).expect("serialize peer");
    let deserialized: Peer = norito::json::from_str(&json).expect("deserialize peer");

    assert_eq!(peer.address(), deserialized.address());
    assert_eq!(peer.id(), deserialized.id());
}
